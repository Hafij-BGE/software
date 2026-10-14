//! Shows an AI answer's Markdown safely: everything is plain text drawn by the suite (nothing is
//! run or loaded), and only http, https and mailto links become clickable.

use egui::{Color32, RichText, Ui};

#[derive(Debug, PartialEq)]
enum Span {
    Text { text: String, bold: bool, italic: bool },
    Code(String),
    Link { label: String, url: String },
}

pub fn safe_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    (u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:")) && !u.contains(char::is_whitespace)
}

/// Inline Markdown: **bold**, *italic* / _italic_, `code`, [label](url) and bare https links.
fn spans(line: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let (mut bold, mut italic) = (false, false);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let flush = |buf: &mut String, out: &mut Vec<Span>, bold: bool, italic: bool| {
        if !buf.is_empty() {
            out.push(Span::Text { text: std::mem::take(buf), bold, italic });
        }
    };
    while i < chars.len() {
        let c = chars[i];
        let rest: String = chars[i..].iter().collect();
        if c == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == '`') {
                flush(&mut buf, &mut out, bold, italic);
                out.push(Span::Code(chars[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
                continue;
            }
        } else if rest.starts_with("**") || rest.starts_with("__") {
            flush(&mut buf, &mut out, bold, italic);
            bold = !bold;
            i += 2;
            continue;
        } else if (c == '*' || c == '_') && (italic || chars.get(i + 1).is_some_and(|n| !n.is_whitespace())) && !(c == '_' && i > 0 && chars[i - 1].is_alphanumeric()) {
            flush(&mut buf, &mut out, bold, italic);
            italic = !italic;
            i += 1;
            continue;
        } else if c == '['
            && let Some(close) = rest.find("](")
            && let Some(end) = rest[close + 2..].find(')')
        {
            let label = rest[1..close].to_string();
            let url = rest[close + 2..close + 2 + end].to_string();
            flush(&mut buf, &mut out, bold, italic);
            if safe_url(&url) {
                out.push(Span::Link { label, url });
            } else {
                out.push(Span::Text { text: format!("{label} ({url})"), bold, italic });
            }
            i += rest[..close + 2 + end + 1].chars().count();
            continue;
        } else if rest.starts_with("https://") || rest.starts_with("http://") {
            let url: String = rest.chars().take_while(|c| !c.is_whitespace() && *c != ')' && *c != '>').collect();
            let url = url.trim_end_matches(['.', ',', ';', ':']).to_string();
            flush(&mut buf, &mut out, bold, italic);
            i += url.chars().count();
            out.push(Span::Link { label: url.clone(), url });
            continue;
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut out, bold, italic);
    out
}

fn inline(ui: &mut Ui, line: &str, size: f32, strong: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for s in spans(line) {
            match s {
                Span::Text { text, bold, italic } => {
                    let mut t = RichText::new(text).size(size);
                    if bold || strong {
                        t = t.strong();
                    }
                    if italic {
                        t = t.italics();
                    }
                    ui.label(t);
                }
                Span::Code(code) => {
                    ui.label(RichText::new(code).monospace().size(size - 1.0).background_color(ui.visuals().code_bg_color));
                }
                Span::Link { label, url } => {
                    ui.hyperlink_to(RichText::new(label).size(size), url);
                }
            }
        }
    });
}

fn table_row(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

pub fn show(ui: &mut Ui, text: &str, size: f32) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    ui.spacing_mut().item_spacing.y = 4.0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_start();
        if t.starts_with("```") {
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            i += 1;
            egui::Frame::NONE.fill(ui.visuals().code_bg_color).corner_radius(6).inner_margin(8).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(code.join("\n")).monospace().size(size - 1.0)).wrap());
            });
            continue;
        }
        if t.starts_with('|') && i + 1 < lines.len() && is_rule(lines[i + 1]) {
            let header = table_row(t);
            let mut rows = Vec::new();
            i += 2;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                rows.push(table_row(lines[i]));
                i += 1;
            }
            egui::ScrollArea::horizontal().id_salt(("md_table", i)).show(ui, |ui| {
                egui::Grid::new(("md_table_grid", i)).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                    for h in &header {
                        inline(ui, h, size, true);
                    }
                    ui.end_row();
                    for r in &rows {
                        for c in r {
                            inline(ui, c, size, false);
                        }
                        ui.end_row();
                    }
                });
            });
            continue;
        }
        if t.is_empty() {
            ui.add_space(2.0);
        } else if let Some(h) = t.strip_prefix("### ").or_else(|| t.strip_prefix("#### ")) {
            inline(ui, h, size + 1.0, true);
        } else if let Some(h) = t.strip_prefix("## ") {
            inline(ui, h, size + 2.5, true);
        } else if let Some(h) = t.strip_prefix("# ") {
            inline(ui, h, size + 4.0, true);
        } else if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
            let indent = (line.len() - t.len()) as f32 * 4.0;
            ui.horizontal_top(|ui| {
                ui.add_space(4.0 + indent);
                ui.label(RichText::new("•").size(size));
                inline(ui, &t[2..], size, false);
            });
        } else if let Some((n, rest)) = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit())) {
            let indent = (line.len() - t.len()) as f32 * 4.0;
            ui.horizontal_top(|ui| {
                ui.add_space(4.0 + indent);
                ui.label(RichText::new(format!("{n}.")).size(size));
                inline(ui, rest, size, false);
            });
        } else if let Some(q) = t.strip_prefix("> ") {
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(3.0, size + 4.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 1.0, Color32::from_gray(140));
                inline(ui, q, size, false);
            });
        } else if t.chars().all(|c| matches!(c, '-' | '*' | '_')) && t.len() >= 3 {
            ui.separator();
        } else {
            inline(ui, t, size, false);
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_safe_links() {
        assert!(safe_url("https://example.org/a") && safe_url("mailto:a@b.c"));
        assert!(!safe_url("javascript:alert(1)") && !safe_url("file:///etc/passwd") && !safe_url("data:text/html,x"));
        let s = spans("see [bad](javascript:alert(1)) and [ok](https://x.org)");
        assert!(s.iter().any(|x| matches!(x, Span::Text { text, .. } if text.contains("bad (javascript:alert(1"))));
        assert!(s.contains(&Span::Link { label: "ok".into(), url: "https://x.org".into() }));
    }

    #[test]
    fn inline_styles() {
        let s = spans("a **b** *c* `d` snake_case_name");
        assert_eq!(s[0], Span::Text { text: "a ".into(), bold: false, italic: false });
        assert_eq!(s[1], Span::Text { text: "b".into(), bold: true, italic: false });
        assert!(s.contains(&Span::Text { text: "c".into(), bold: false, italic: true }));
        assert!(s.contains(&Span::Code("d".into())));
        assert!(s.iter().any(|x| matches!(x, Span::Text { text, italic: false, .. } if text.contains("snake_case_name"))));
    }
}
