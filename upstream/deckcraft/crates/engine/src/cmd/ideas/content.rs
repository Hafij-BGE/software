//! What a slide's text says, as far as Design Ideas care: the list items of its text, a
//! statistic to show large, a quotation, numbered steps or dates. Plain string checks, offline.

use deckcraft_model::text::{Bullet, Paragraph, TextBody};

/// One list item of a text body: a top-level paragraph and the deeper paragraphs under it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Item {
    /// First paragraph of the item, and how many paragraphs (empty ones included) it spans.
    pub start: usize,
    pub len: usize,
    /// The item's own (top-level) text.
    pub head: String,
    /// Non-empty paragraphs below the head.
    pub subs: usize,
}

/// The list items of `body`: each non-empty paragraph at the shallowest level starts one.
pub(crate) fn items(body: &TextBody) -> Vec<Item> {
    let ps = &body.paragraphs;
    let Some(top) = ps.iter().filter(|p| !p.text().trim().is_empty()).map(|p| p.level).min() else { return vec![] };
    let mut out: Vec<Item> = vec![];
    for (i, p) in ps.iter().enumerate() {
        let t = p.text();
        if t.trim().is_empty() {
            if let Some(last) = out.last_mut() {
                last.len = i + 1 - last.start;
            }
            continue;
        }
        if p.level <= top || out.is_empty() {
            out.push(Item { start: i, len: 1, head: t.trim().to_string(), subs: 0 });
        } else if let Some(last) = out.last_mut() {
            last.len = i + 1 - last.start;
            last.subs += 1;
        }
    }
    // Trailing empty paragraphs belong to no item's visible text but stay in its range, so a
    // card's paragraphs map back one to one.
    out
}

/// A figure worth showing large: `45%`, `$3.2M`, `1,200`, `3x`, `2026`. The first one found in
/// `text`, with its sign or currency.
pub(crate) fn statistic(text: &str) -> Option<String> {
    for raw in text.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | ':' | ';' | '"' | '“' | '”')) {
        let tok = raw.trim_end_matches(['.', ',', '!', '?']).trim_start_matches(['+']);
        let digits = tok.chars().filter(char::is_ascii_digit).count();
        if digits == 0 || tok.chars().count() > 12 {
            continue;
        }
        let first = tok.chars().next().unwrap_or(' ');
        let currency = matches!(first, '$' | '€' | '£' | '¥' | '₹' | '₺' | '-' | '−');
        let body: String = tok.trim_start_matches(['$', '€', '£', '¥', '₹', '₺', '-', '−']).to_string();
        let lower = body.to_ascii_lowercase();
        let num_end = body.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',')).unwrap_or(body.len());
        let (num, suffix) = body.split_at(num_end);
        if num.is_empty() || !num.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let suffix_ok = matches!(suffix, "" | "%" | "x" | "×" | "k" | "K" | "m" | "M" | "bn" | "B" | "+" | "pts" | "pt") || lower.ends_with("%");
        if !suffix_ok {
            continue;
        }
        // A list number ("1.", "2)") or a lone digit is not a statistic; a percentage,
        // currency, suffix or a figure of two digits or more is.
        if currency || !suffix.is_empty() || digits >= 2 {
            return Some(tok.to_string());
        }
    }
    None
}

/// Whether `text` reads as a quotation: it opens with a quotation mark, or closes with one and
/// an attribution line.
pub(crate) fn is_quote(text: &str) -> bool {
    let t = text.trim();
    let opens = t.starts_with(['"', '“', '«', '„', '\'', '‘', '「']);
    let closes = t.lines().next().is_some_and(|l| l.trim_end().ends_with(['"', '”', '»', '’']));
    opens || (closes && t.lines().any(|l| l.trim_start().starts_with(['—', '–', '-', '~'])))
}

const ORDINALS: [&str; 12] = ["first", "second", "third", "fourth", "fifth", "sixth", "then", "next", "finally", "last", "after", "before"];
const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// Whether the item starts like a step: `1.`, `2)`, `Step 3`, `Phase 2`, `First`, `Then`.
fn step_like(head: &str) -> bool {
    let l = head.trim().to_lowercase();
    let lead = l.split(|c: char| c.is_whitespace() || c == ':' || c == ',').next().unwrap_or("");
    // A list number has one or two digits; `2019 Founded` is a date, not step 2019.
    let digits = l.chars().take_while(char::is_ascii_digit).count();
    let numbered = (1..=2).contains(&digits) && l.chars().nth(digits).is_some_and(|c| matches!(c, '.' | ')' | ':' | ' ' | '-'));
    numbered || ["step", "phase", "stage", "day", "week"].contains(&lead) || ORDINALS.contains(&lead)
}

/// Whether the item starts with a date: a year, a month, a quarter.
fn dated(head: &str) -> bool {
    let l = head.trim().to_lowercase();
    let lead = l.split(|c: char| c.is_whitespace() || matches!(c, ':' | ',' | '-' | '–' | '—')).next().unwrap_or("");
    let year = lead.len() == 4 && lead.chars().all(|c| c.is_ascii_digit()) && matches!(lead.parse::<u32>(), Ok(1000..=2999));
    let quarter = lead.len() == 2 && lead.starts_with('q') && lead[1..].chars().all(|c| matches!(c, '1'..='4'));
    year || quarter || MONTHS.iter().any(|m| lead.starts_with(m))
}

/// Whether the paragraph is numbered by its own bullet (`1.`, `a)`).
fn auto_numbered(p: &Paragraph) -> bool {
    matches!(p.props.bullet, Some(Bullet::AutoNum { .. }))
}

/// Whether the items read as steps in order: numbered (by bullet or by text) or led by step and
/// ordinal words, all but one of them at most.
pub(crate) fn steps(body: &TextBody, items: &[Item]) -> bool {
    if items.len() < 3 {
        return false;
    }
    let numbered = items.iter().filter(|i| body.paragraphs.get(i.start).is_some_and(auto_numbered)).count();
    let led = items.iter().filter(|i| step_like(&i.head)).count();
    numbered == items.len() || led + 1 >= items.len()
}

/// Whether the items are dated (a timeline): all but one start with a year, month or quarter.
pub(crate) fn timeline(items: &[Item]) -> bool {
    items.len() >= 3 && items.iter().filter(|i| dated(&i.head)).count() + 1 >= items.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_are_found_and_list_numbers_are_not() {
        assert_eq!(statistic("Revenue grew 45% this year").as_deref(), Some("45%"));
        assert_eq!(statistic("We raised $3.2M.").as_deref(), Some("$3.2M"));
        assert_eq!(statistic("1,200 customers").as_deref(), Some("1,200"));
        assert_eq!(statistic("Founded in 1998").as_deref(), Some("1998"));
        assert_eq!(statistic("1. Plan the work"), None);
        assert_eq!(statistic("Version 2 is out"), None);
        assert_eq!(statistic("no figures here"), None);
        assert_eq!(statistic("abc123def"), None);
    }

    #[test]
    fn quotes_steps_and_dates_are_recognised() {
        assert!(is_quote("“Simplicity is the ultimate sophistication.”\n— Leonardo"));
        assert!(is_quote("\"Stay hungry\""));
        assert!(!is_quote("Plain statement"));
        let body = TextBody::from_text("1. Plan\n2. Build\n3. Ship");
        let it = items(&body);
        assert_eq!(it.len(), 3);
        assert!(steps(&body, &it));
        assert!(!timeline(&it));
        let body = TextBody::from_text("2019 Founded\n2021 First customer\nMarch 2024 Series A");
        let it = items(&body);
        assert!(timeline(&it) && !steps(&body, &it));
        let body = TextBody::from_text("Apples\nPears\nPlums");
        assert!(!steps(&body, &items(&body)));
    }

    #[test]
    fn items_group_deeper_paragraphs_under_their_head() {
        let mut body = TextBody::from_text("Pros\nfast\ncheap\n\nCons\nloud");
        for i in [1, 2, 5] {
            body.paragraphs[i].level = 1;
        }
        let it = items(&body);
        assert_eq!(it.len(), 2);
        assert_eq!((it[0].start, it[0].len, it[0].subs), (0, 4, 2));
        assert_eq!((it[1].start, it[1].len, it[1].subs, it[1].head.as_str()), (4, 2, 1, "Cons"));
    }
}
