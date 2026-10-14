//! What the AI sees of a tab, what it is told, and the changes it may suggest.
//!
//! Private (the default) shares the headings, slide titles, the selected text and statistics the
//! suite computes; never the full text, speaker notes or data rows. With Private off the text,
//! notes and the first rows of the sheet go too, up to a size limit.

use serde_json::{Value, json};

use super::stats::{self, Summary, Table};
use crate::hosts::Doc;
use crate::kind::Kind;

/// The most text of a document sent with Private off.
const MAX_TEXT: usize = 30_000;
const MAX_SELECTION: usize = 6_000;
const MAX_ROWS: usize = 80;

pub struct Gathered {
    /// For the AI.
    pub prompt: String,
    /// For the user: what was shared.
    pub shared: String,
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}\n…(cut: {} more characters not sent)", s.chars().take(max).collect::<String>(), s.chars().count() - max) }
}

fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.get("text").and_then(Value::as_str).map(str::to_string).unwrap_or_default(),
    }
}

fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            if f.fract() == 0.0 && f.abs() < 1e15 { format!("{}", f as i64) } else { f.to_string() }
        }
        Value::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
        other => other.to_string(),
    }
}

/// The active sheet's used range as a table (header row first), from GridCraft.
pub fn sheet_table(doc: &mut Doc, range: Option<&str>) -> Option<(String, Table)> {
    let params = match range {
        Some(r) => json!({"range": r}),
        None => json!({}),
    };
    let read = doc.command("sheet.read", params).ok()?;
    let rows: Vec<Vec<String>> = read.get("values")?.as_array()?.iter().map(|r| r.as_array().map(|c| c.iter().map(cell_text).collect()).unwrap_or_default()).collect();
    let name = format!("{}!{}", read.get("sheet").and_then(Value::as_str).unwrap_or("Sheet"), read.get("range").and_then(Value::as_str).unwrap_or(""));
    Some((name, Table::from_rows(rows)))
}

pub fn gather(doc: &mut Doc, title: &str, private: bool) -> Gathered {
    match doc.kind() {
        Kind::Word => word(doc, title, private),
        Kind::Deck => deck(doc, title, private),
        Kind::Grid => grid(doc, title, private),
        _ => Gathered { prompt: String::new(), shared: String::new() },
    }
}

fn word(doc: &mut Doc, title: &str, private: bool) -> Gathered {
    let mut p = format!("Document: {title} (WordCraft)\n");
    let inspect = doc.command("document.inspect", json!({})).unwrap_or(Value::Null);
    let text = as_text(&doc.command("document.text", json!({})).unwrap_or(Value::Null));
    let words = text.split_whitespace().count();
    let pages = inspect.get("pages").and_then(Value::as_u64).unwrap_or(0);
    let blocks = inspect.get("blocks").and_then(Value::as_array).cloned().unwrap_or_default();
    p.push_str(&format!("Pages: {pages}; paragraphs: {}; words: {words}\n", blocks.len()));
    let headings: Vec<String> = blocks
        .iter()
        .filter_map(|b| {
            let style = b.get("style").and_then(Value::as_str).unwrap_or("");
            let lower = style.to_ascii_lowercase();
            (lower.starts_with("heading") || lower == "title" || lower == "subtitle").then(|| format!("- [{style}] {}", b.get("text").and_then(Value::as_str).unwrap_or("").trim()))
        })
        .take(80)
        .collect();
    if headings.is_empty() {
        p.push_str("Headings: none\n");
    } else {
        p.push_str(&format!("Headings:\n{}\n", headings.join("\n")));
    }
    let selection = doc.command("document.selection", json!({})).ok().and_then(|v| v.get("text").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
    let mut shared = vec!["headings", "word count"];
    if !selection.trim().is_empty() {
        p.push_str(&format!("Selected text:\n\"\"\"\n{}\n\"\"\"\n", cut(&selection, MAX_SELECTION)));
        shared.push("the selected text");
    } else {
        p.push_str("Selected text: none\n");
    }
    if private {
        p.push_str("Full text: not shared (Private is on).\n");
    } else {
        p.push_str(&format!("Full text:\n\"\"\"\n{}\n\"\"\"\n", cut(&text, MAX_TEXT)));
        shared.push("the full text");
    }
    Gathered { prompt: p, shared: shared.join(", ") }
}

fn deck(doc: &mut Doc, title: &str, private: bool) -> Gathered {
    let mut p = format!("Presentation: {title} (DeckCraft)\n");
    let inspect = doc.command("document.inspect", json!({})).unwrap_or(Value::Null);
    let slides = inspect.get("slides").and_then(Value::as_array).cloned().unwrap_or_default();
    let current = inspect.pointer("/selection/slide").and_then(Value::as_u64);
    p.push_str(&format!("Slides: {} (current: {})\n", slides.len(), current.map(|c| (c + 1).to_string()).unwrap_or("–".into())));
    let mut total = 0;
    for s in &slides {
        let i = s.get("index").and_then(Value::as_u64).unwrap_or(0);
        let t = s.get("title").and_then(Value::as_str).unwrap_or("").trim();
        let layout = s.get("layout").and_then(Value::as_str).unwrap_or("");
        let hidden = if s.get("hidden") == Some(&Value::Bool(true)) { ", hidden" } else { "" };
        p.push_str(&format!("- Slide {} (index {i}): {} [{layout}{hidden}]\n", i + 1, if t.is_empty() { "(no title)" } else { t }));
        if !private && total < MAX_TEXT {
            let detail = doc.command("slide.inspect", json!({"index": i})).unwrap_or(Value::Null);
            for sh in detail.get("shapes").and_then(Value::as_array).cloned().unwrap_or_default() {
                let text = sh.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string();
                if !text.is_empty() && text != t {
                    let id = sh.get("id").and_then(Value::as_u64).unwrap_or(0);
                    let line = format!("    shape {id}: {}\n", cut(&text.replace('\n', " / "), 1500));
                    total += line.len();
                    p.push_str(&line);
                }
            }
            let notes = detail.get("notes").and_then(Value::as_str).unwrap_or("").trim();
            if !notes.is_empty() {
                let line = format!("    speaker notes: {}\n", cut(&notes.replace('\n', " / "), 1500));
                total += line.len();
                p.push_str(&line);
            }
        }
    }
    let sel = inspect.pointer("/selection/text").map(as_text).unwrap_or_default();
    let mut shared = vec!["slide titles"];
    if !sel.trim().is_empty() {
        p.push_str(&format!("Selected text:\n\"\"\"\n{}\n\"\"\"\n", cut(&sel, MAX_SELECTION)));
        shared.push("the selected text");
    }
    if private {
        p.push_str("Slide text and speaker notes: not shared (Private is on).\n");
    } else {
        shared.push("slide text and speaker notes");
    }
    Gathered { prompt: p, shared: shared.join(", ") }
}

fn grid(doc: &mut Doc, title: &str, private: bool) -> Gathered {
    let mut p = format!("Workbook: {title} (GridCraft)\n");
    let inspect = doc.command("document.inspect", json!({})).unwrap_or(Value::Null);
    for s in inspect.get("sheets").and_then(Value::as_array).cloned().unwrap_or_default() {
        let tables: Vec<String> = s.get("tables").and_then(Value::as_array).map(|t| t.iter().filter_map(|x| x.get("name").and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
        p.push_str(&format!(
            "- Sheet \"{}\": used range {}, {} charts{}\n",
            s.get("name").and_then(Value::as_str).unwrap_or(""),
            s.get("usedRange").and_then(Value::as_str).unwrap_or("none"),
            s.get("charts").and_then(Value::as_array).map(|c| c.len()).unwrap_or(0),
            if tables.is_empty() { String::new() } else { format!(", tables: {}", tables.join(", ")) }
        ));
    }
    let selection = inspect.get("selection").and_then(Value::as_str).unwrap_or("");
    p.push_str(&format!("Active sheet index: {}; selection: {selection}\n", inspect.get("activeSheet").and_then(Value::as_u64).unwrap_or(0)));
    let mut shared = vec!["sheet names", "column names", "computed statistics"];
    if let Some((range, table)) = sheet_table(doc, None)
        && !table.headers.is_empty()
    {
        p.push_str(&format!("Computed statistics of the active sheet ({range}; the first row is taken as the header):\n{}", summarize_for_prompt(&stats::summarize(&table))));
        if !private {
            let mut rows = vec![table.headers.join("\t")];
            rows.extend(table.rows.iter().take(MAX_ROWS).map(|r| r.join("\t")));
            p.push_str(&format!("First {} data rows (tab-separated):\n{}\n", table.rows.len().min(MAX_ROWS), cut(&rows.join("\n"), 20_000)));
            shared.push("the first rows");
        } else {
            p.push_str("Data rows: not shared (Private is on).\n");
        }
    }
    Gathered { prompt: p, shared: shared.join(", ") }
}

pub fn summarize_for_prompt(s: &Summary) -> String {
    s.to_prompt()
}

// ---- the AI's instructions ----------------------------------------------------------------

/// Changes the AI may suggest, per app: (command, parameters, what it does). Nothing that opens,
/// saves, prints, deletes files or reads paths.
pub fn allowed(kind: Kind) -> &'static [(&'static str, &'static str, &'static str)] {
    match kind {
        Kind::Word => &[
            ("text.insert", r#"{"text": string}"#, "types text at the cursor, replacing the selection; \\n starts a new paragraph"),
            ("text.newParagraph", "{}", "starts a new paragraph"),
            ("select.text", r#"{"text": string, "occurrence"?: n}"#, "selects text found in the document (first occurrence by default)"),
            ("select.all", "{}", "selects the whole document"),
            ("select.paragraph", "{}", "selects the paragraph at the cursor"),
            ("caret.docEnd", "{}", "moves the cursor to the end of the document"),
            ("caret.docStart", "{}", "moves the cursor to the start of the document"),
            ("caret.end", "{}", "moves the cursor to the end of its line"),
            ("edit.replaceAll", r#"{"text": string, "with": string}"#, "replaces every occurrence"),
            ("styles.apply", r#"{"style": "Title"|"Heading 1"|"Heading 2"|"Heading 3"|"Normal"|"List Bullet"|"List Number"|"Quote"}"#, "applies a paragraph style to the selected paragraphs"),
            ("format.bold", r#"{"value"?: bool}"#, "bold for the selection"),
            ("format.italic", r#"{"value"?: bool}"#, "italic for the selection"),
            ("format.underline", r#"{"value"?: bool}"#, "underline for the selection"),
            ("format.highlight", r#"{"color": "yellow"|"brightGreen"|"turquoise"|"pink"|null}"#, "highlights the selection"),
            ("format.changeCase", r#"{"mode": "sentence"|"lower"|"upper"|"title"}"#, "changes the case of the selection"),
            ("insert.table", r#"{"rows": n, "cols": n}"#, "inserts an empty table at the cursor"),
        ],
        Kind::Deck => &[
            ("slide.fromOutline", r#"{"text": string}"#, "adds slides from an outline: each unindented line is a slide title, tab-indented lines are its bullets"),
            ("slide.new", r#"{"layout"?: "title"|"titleAndContent"|"sectionHeader"|"twoContent"|"titleOnly"|"blank", "title"?: string, "body"?: string, "at"?: index}"#, "adds one slide"),
            ("slide.notes", r#"{"text": string, "index"?: n}"#, "sets a slide's speaker notes"),
            ("slide.layout", r#"{"layout": string, "index"?: n}"#, "changes a slide's layout"),
            ("slide.go", r#"{"index": n}"#, "shows a slide (0-based index)"),
            ("text.set", r#"{"id": shape id, "text": string}"#, "replaces a shape's text (shape ids are listed in the context when shared)"),
            ("insert.textBox", r#"{"text": string, "rect"?: [x, y, w, h]}"#, "adds a text box to the current slide (points; the slide is 960 × 540)"),
            ("insert.chart", r#"{"type"?: "column"|"bar"|"line"|"pie", "categories": [..], "series": [{"name": string, "values": [numbers]}]}"#, "adds a chart to the current slide"),
            ("insert.table", r#"{"rows": n, "cols": n}"#, "adds a table to the current slide"),
            ("insert.smartArt", r#"{"kind": "list"|"process"|"cycle"|"hierarchy"|"pyramid"|"matrix", "items": [string]}"#, "adds a SmartArt graphic"),
        ],
        Kind::Grid => &[
            ("cell.set", r#"{"cell": "B2", "input": "text, number or =formula"}"#, "enters one cell"),
            ("range.setValues", r#"{"range": "A1", "values": [[...], ...]}"#, "enters rows of values from the top-left cell; strings starting with = are formulas"),
            ("selection.set", r#"{"range": "A1:B2"}"#, "selects cells"),
            ("insert.table", r#"{"range": "A1:D10", "header"?: true}"#, "formats a range as a table"),
            ("insert.chart", r#"{"range": "A1:B10", "type"?: "column"|"bar"|"line"|"pie"|"scatter"}"#, "adds a chart of a range"),
            ("home.bold", r#"{"range": "A1:D1", "on"?: bool}"#, "bold cells"),
            ("home.fillColor", r##"{"range": "A1:D1", "color": "#FFFF00"|"none"}"##, "fills cells with a colour"),
            ("home.cellStyle", r#"{"range": "A1", "name": "Good"|"Bad"|"Neutral"|"Heading 1"|"Title"|"Total"|"Normal"}"#, "applies a cell style"),
            ("home.numberFormat", r#"{"range": "B2:B10", "format": "General"|"Number"|"Currency"|"Percent"|"Short Date"}"#, "sets a number format"),
            ("data.sort", r#"{"range"?: "A1:D20", "header"?: true, "keys": [{"column": "B", "order": "asc"|"desc"}]}"#, "sorts a range"),
            ("data.removeDuplicates", r#"{"range"?: "A1:D20", "header"?: true}"#, "removes duplicate rows"),
            ("sheet.rename", r#"{"name": string}"#, "renames the active sheet"),
        ],
        _ => &[],
    }
}

pub fn system_prompt(kind: Kind, context: &str, data: &str) -> String {
    let what = match kind {
        Kind::Word => "a WordCraft document",
        Kind::Deck => "a DeckCraft presentation",
        Kind::Grid => "a GridCraft workbook",
        _ => "no document (the Home page)",
    };
    let mut s = format!(
        "You are Ask AI, the assistant inside Craft Suite (an office suite). The user is working on {what}.\n\
Rules:\n\
- Answer only from the context below and the conversation. If something isn't there, say it is missing or not shared; don't guess.\n\
- Never invent numbers, dates, citations, quotes, names or file contents.\n\
- Numbers under \"Computed statistics\" were calculated by the app. Explain them; never recalculate, round differently or add new numbers.\n\
- Be brief and clear. Use Markdown (headings, lists, **bold**, tables) when it helps.\n"
    );
    let commands = allowed(kind);
    if !commands.is_empty() {
        s.push_str(
            "- When the user asks you to change the document, add ONE fenced code block whose first line is ```craft and whose content is a JSON array of steps, e.g.\n\
```craft\n[{\"command\": \"text.insert\", \"params\": {\"text\": \"Hello\"}}]\n```\n\
  The user reviews the steps and clicks Apply; say what they will do, never that you already did it. Only these commands exist:\n",
        );
        for (id, params, what) in commands {
            s.push_str(&format!("  - {id} {params}: {what}\n"));
        }
    }
    s.push_str("\nContext (the current state, gathered just now):\n");
    s.push_str(if context.trim().is_empty() { "(no document)\n" } else { context });
    if !data.trim().is_empty() {
        s.push_str("\nAttached data — computed statistics only (no data rows were sent):\n");
        s.push_str(data);
    }
    s
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub command: String,
    pub params: Value,
}

impl Step {
    /// For the list the user reviews.
    pub fn describe(&self) -> String {
        let p = |k: &str| self.params.get(k).map(|v| match v {
            Value::String(s) => {
                let s = s.replace('\n', " ⏎ ");
                if s.chars().count() > 60 { format!("\"{}…\"", s.chars().take(60).collect::<String>()) } else { format!("\"{s}\"") }
            }
            other => {
                let s = other.to_string();
                if s.chars().count() > 60 { format!("{}…", s.chars().take(60).collect::<String>()) } else { s }
            }
        });
        let args: Vec<String> = self.params.as_object().map(|o| o.keys().filter_map(|k| p(k).map(|v| format!("{k}: {v}"))).collect()).unwrap_or_default();
        if args.is_empty() { self.command.clone() } else { format!("{}  ({})", self.command, args.join(", ")) }
    }
}

/// The reply without its ```craft block, and the block's steps (only allowed commands, without
/// file paths); a step that isn't allowed is reported, not kept.
pub fn split_steps(kind: Kind, reply: &str) -> (String, Vec<Step>, Vec<String>) {
    let mut text = String::new();
    let mut block = String::new();
    let mut in_block = false;
    let mut found = false;
    for line in reply.lines() {
        let t = line.trim();
        if !in_block && !found && (t == "```craft" || t == "``` craft") {
            in_block = true;
            continue;
        }
        if in_block {
            if t.starts_with("```") {
                in_block = false;
                found = true;
            } else {
                block.push_str(line);
                block.push('\n');
            }
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }
    let mut steps = Vec::new();
    let mut refused = Vec::new();
    if found || in_block {
        match serde_json::from_str::<Value>(block.trim()) {
            Ok(Value::Array(items)) => {
                let ok = allowed(kind);
                for item in items {
                    let command = item.get("command").and_then(Value::as_str).unwrap_or("").to_string();
                    let params = item.get("params").cloned().filter(|v| v.is_object()).unwrap_or(json!({}));
                    let has_path = params.as_object().is_some_and(|o| o.keys().any(|k| matches!(k.as_str(), "path" | "data" | "base64" | "url" | "file")));
                    if command.is_empty() {
                        continue;
                    }
                    if !ok.iter().any(|(id, _, _)| *id == command) || has_path {
                        refused.push(command);
                    } else {
                        steps.push(Step { command, params });
                    }
                }
            }
            _ => refused.push("(the suggested changes couldn't be read)".into()),
        }
    }
    (text.trim().to_string(), steps, refused)
}

/// Runs the steps on the tab, in order, stopping at the first that fails. Returns how many ran.
pub fn apply(doc: &mut Doc, steps: &[Step]) -> Result<usize, (usize, String)> {
    let kind = doc.kind();
    for (i, step) in steps.iter().enumerate() {
        let r = if kind == Kind::Word && step.command == "text.insert" {
            // A line break in the text: a new paragraph, as typing Enter.
            let text = step.params.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            let mut r = Ok(Value::Null);
            for (n, line) in text.split('\n').enumerate() {
                if n > 0 {
                    r = doc.command("text.newParagraph", json!({}));
                }
                if r.is_ok() && !line.is_empty() {
                    r = doc.command("text.insert", json!({"text": line, "raw": true}));
                }
                if r.is_err() {
                    break;
                }
            }
            r
        } else {
            doc.command(&step.command, step.params.clone())
        };
        if let Err(e) = r {
            return Err((i, format!("{}: {e}", step.command)));
        }
    }
    Ok(steps.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_steps_and_refuses_others() {
        let reply = "Here you go.\n```craft\n[{\"command\":\"text.insert\",\"params\":{\"text\":\"Hi\"}},{\"command\":\"file.save\",\"params\":{}},{\"command\":\"select.text\",\"params\":{\"text\":\"x\",\"path\":\"/etc/passwd\"}}]\n```\nDone.";
        let (text, steps, refused) = split_steps(Kind::Word, reply);
        assert_eq!(text, "Here you go.\nDone.");
        assert_eq!(steps, vec![Step { command: "text.insert".into(), params: json!({"text": "Hi"}) }]);
        assert_eq!(refused, vec!["file.save", "select.text"]);
        let (_, none, _) = split_steps(Kind::Grid, "no block here");
        assert!(none.is_empty());
    }

    #[test]
    fn prompt_has_rules_and_commands() {
        let s = system_prompt(Kind::Grid, "Workbook: x\n", "Rows: 3\n");
        assert!(s.contains("Never invent numbers"));
        assert!(s.contains("range.setValues"));
        assert!(s.contains("no data rows were sent"));
        assert!(!system_prompt(Kind::Home, "", "").contains("```craft"));
    }
}
