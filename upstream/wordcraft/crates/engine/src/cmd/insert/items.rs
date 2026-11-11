use super::cover_pages::{CoverText, cover_blocks};
use super::header_footer::hf_part;
use super::hf_designs::{PAGE_NUMBER_STYLES, push_page_number};
use super::{
    Align, CharProps, CmdError, CmdResult, Float, InlineObject, Paragraph, PartKind, Selection, Session, ShapeKind, StoryRef, TabAlign, TabStop,
    Value, Wrap, delete_selection, json, p, para_block, sel_result, type_text,
};
use wordcraft_doc::para::{Anchor, FloatAlign};

pub(super) fn page_number(s: &mut Session, v: &Value) -> CmdResult {
    let position = p::str(v, "position").unwrap_or("bottom");
    let align = match p::str(v, "align").unwrap_or("center") {
        "left" => Align::Left,
        "right" => Align::Right,
        _ => Align::Center,
    };
    // `format: "x of y"` is the older spelling of the Page X of Y style.
    let legacy = if p::str(v, "format").is_some_and(|f| f.contains("of")) { "xOfY" } else { "plain" };
    let style = p::str(v, "style").unwrap_or(legacy);
    if !PAGE_NUMBER_STYLES.iter().any(|(id, _)| *id == style) {
        return Err(CmdError::Params(format!("unknown page number style {style:?}")));
    }
    if !matches!(position, "top" | "bottom" | "current" | "leftMargin" | "rightMargin") {
        return Err(CmdError::Params(format!("unknown page number position {position:?}")));
    }
    if style == "circle" {
        // Word draws circled page numbers with the section's number format.
        let block = s.sel.focus.path.0.first().copied().unwrap_or(0) as usize;
        s.doc.section_mut(block).page_num_format = wordcraft_doc::section::NumFormat::DecimalEnclosedCircle;
    }
    if position == "current" {
        let props = s.typing_props();
        let at = delete_selection(s)?;
        let mut para = Paragraph::new();
        push_page_number(&mut para, style)?;
        let mut end = at;
        for (text, obj, run) in para_pieces(&para, &props) {
            end = match obj {
                Some(obj) => s.doc.insert_object(&end, obj, &run)?,
                None => s.doc.insert_text(&end, &text, &run)?,
            };
        }
        s.sel = Selection::caret(end);
        return sel_result(s);
    }
    let margin = matches!(position, "leftMargin" | "rightMargin");
    let header = position == "top" || margin;
    let id = hf_part(s, header);
    let style_name = if header { "Header" } else { "Footer" };
    let mut para = Paragraph::new().styled(style_name);
    para.props.tabs = Some(vec![TabStop { pos: 0.0, align: TabAlign::Clear, leader: Default::default() }]);
    if margin {
        // In the page margin: a borderless text box the header anchors, centered on the page's
        // height in the left (or right) margin, as Word's Page Margins designs place it.
        let mut inner = Paragraph::new().styled(style_name);
        inner.props.align = Some(Align::Center);
        push_page_number(&mut inner, style)?;
        let story = s.doc.try_add_part(PartKind::TextBox, vec![para_block(inner)])?;
        let float = Float {
            wrap: Wrap::InFrontOfText,
            h_rel: if position == "leftMargin" { Anchor::LeftMargin } else { Anchor::RightMargin },
            v_rel: Anchor::Page,
            h_align: Some(FloatAlign::Center),
            v_align: Some(FloatAlign::Center),
            ..Default::default()
        };
        let obj = InlineObject::Shape {
            kind: ShapeKind::TextBox,
            w: 54.0,
            h: 36.0,
            fill: None,
            stroke: None,
            stroke_width: 0.75,
            float,
            story: Some(story),
            freeform: None,
            effects: Default::default(),
            extra: Default::default(),
        };
        // Kept with what the header already shows, like Word's margin designs.
        let mut blocks: Vec<_> = s.doc.parts.get(&id).map(|part| part.blocks.clone()).unwrap_or_default();
        para.insert_object(0, obj, &CharProps::default())?;
        blocks.push(para_block(para));
        s.doc.set_story(StoryRef::Part(id), blocks)?;
        return Ok(json!({"story": id, "box": story}));
    }
    para.props.align = Some(align);
    push_page_number(&mut para, style)?;
    s.doc.set_story(StoryRef::Part(id), vec![para_block(para)])?;
    Ok(json!({"story": id}))
}

/// `para`'s text runs and objects in order, each with `base` under its own character formatting:
/// how a styled page number goes in at the caret.
fn para_pieces(para: &Paragraph, base: &CharProps) -> Vec<(String, Option<InlineObject>, CharProps)> {
    let mut out = Vec::new();
    let mut objects = para.objects.iter();
    let mut off = 0;
    for run in &para.runs {
        let Some(run_text) = para.text.get(off..off + run.len) else { break };
        off += run.len;
        let mut props = base.clone();
        props.overlay(&run.props);
        let mut text = String::new();
        for c in run_text.chars() {
            if c == wordcraft_doc::para::OBJ {
                if !text.is_empty() {
                    out.push((std::mem::take(&mut text), None, props.clone()));
                }
                if let Some(obj) = objects.next() {
                    out.push((String::new(), Some(obj.clone()), props.clone()));
                }
            } else {
                text.push(c);
            }
        }
        if !text.is_empty() {
            out.push((text, None, props));
        }
    }
    out
}

pub(super) fn date_time(s: &mut Session, v: &Value) -> CmdResult {
    let fmt = p::str(v, "format").unwrap_or("M/d/yyyy");
    let text = format_date(fmt);
    if p::bool(v, "update").unwrap_or(false) {
        let props = s.typing_props();
        let at = delete_selection(s)?;
        let end = s.doc.insert_object(
            &at,
            InlineObject::Field { instr: format!("DATE \\@ \"{fmt}\""), result: text, locked: false, dirty: false, code: false, form: String::new() },
            &props,
        )?;
        s.sel = Selection::caret(end);
    } else {
        type_text(s, &text)?;
    }
    sel_result(s)
}

/// Format today's date with a Word picture (`M/d/yyyy`, `MMMM d, yyyy`, `yyyy-MM-dd`, `dddd`…).
pub fn format_date(fmt: &str) -> String {
    // Word shows the computer's local date and time (#1707), not UTC's.
    let now = crate::cmd::now_unix();
    format_date_at(fmt, now, crate::cmd::local_offset_secs(now))
}

/// Format the instant `unix` (seconds since the epoch, UTC) with a Word picture, in the zone
/// `offset_secs` east of UTC.
pub fn format_date_at(fmt: &str, unix: u64, offset_secs: i64) -> String {
    let (y, m, d, hms, days) = crate::cmd::local_civil(unix, offset_secs);
    format_date_parts(fmt, y, m as usize, d, hms, days)
}

/// Format a date and time with a Word picture; `days_since` (days since 1970-01-01) gives the weekday.
pub fn format_date_parts(fmt: &str, y: i64, m: usize, d: u32, (hh, mm, ss): (u32, u32, u32), days_since: i64) -> String {
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const DAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];
    let month = MONTHS.get(m.saturating_sub(1)).copied().unwrap_or("January");
    let day = DAYS.get(days_since.rem_euclid(7) as usize).copied().unwrap_or("Monday");
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars.get(i).copied().unwrap_or(' ');
        // Text in single quotes is shown as it is; two quotes in a row are one, inside quoted
        // text or out of it.
        if c == '\'' && chars.get(i + 1) == Some(&'\'') {
            out.push('\'');
            i += 2;
            continue;
        }
        if c == '\'' {
            i += 1;
            while let Some(&q) = chars.get(i) {
                i += 1;
                if q != '\'' {
                    out.push(q);
                } else if chars.get(i) == Some(&'\'') {
                    out.push('\'');
                    i += 1;
                } else {
                    break;
                }
            }
            continue;
        }
        // `am/pm` (any case) is one token: AM or PM.
        if chars.get(i..i + 5).is_some_and(|w| w.iter().collect::<String>().eq_ignore_ascii_case("am/pm")) {
            out.push_str(if hh < 12 { "AM" } else { "PM" });
            i += 5;
            continue;
        }
        let mut n = 1;
        while chars.get(i + n) == Some(&c) {
            n += 1;
        }
        match (c, n) {
            ('y' | 'Y', 4..) => out.push_str(&format!("{y:04}")),
            ('y' | 'Y', _) => out.push_str(&format!("{:02}", y % 100)),
            ('M', 4..) => out.push_str(month),
            ('M', 3) => out.push_str(month.get(..3).unwrap_or(month)),
            ('M', 2) => out.push_str(&format!("{m:02}")),
            ('M', 1) => out.push_str(&m.to_string()),
            ('d' | 'D', 4..) => out.push_str(day),
            ('d' | 'D', 3) => out.push_str(day.get(..3).unwrap_or(day)),
            ('d' | 'D', 2) => out.push_str(&format!("{d:02}")),
            ('d' | 'D', 1) => out.push_str(&d.to_string()),
            ('H', 2..) => out.push_str(&format!("{hh:02}")),
            ('H', _) => out.push_str(&hh.to_string()),
            ('h', 2..) => out.push_str(&format!("{:02}", if hh.is_multiple_of(12) { 12 } else { hh % 12 })),
            ('h', _) => out.push_str(&(if hh.is_multiple_of(12) { 12 } else { hh % 12 }).to_string()),
            ('m', 2..) => out.push_str(&format!("{mm:02}")),
            ('m', _) => out.push_str(&mm.to_string()),
            ('s', 2..) => out.push_str(&format!("{ss:02}")),
            ('s', _) => out.push_str(&ss.to_string()),
            ('a' | 'A', _) => out.push_str(if hh < 12 { "AM" } else { "PM" }),
            _ => {
                for _ in 0..n {
                    out.push(c);
                }
            }
        }
        i += n;
    }
    out
}

/// Insert › Symbol: the character at the caret, in its own font when `font` names one (the rest of
/// the text keeps the caret's font).
pub(super) fn symbol(s: &mut Session, v: &Value) -> CmdResult {
    let c = p::req_str(v, "char")?;
    if c.is_empty() || c.chars().count() > 64 {
        return Err(CmdError::Params("char: 1–64 characters".into()));
    }
    let font = p::str(v, "font").map(str::trim).filter(|f| !f.is_empty());
    let Some(font) = font else {
        type_text(s, c)?;
        return sel_result(s);
    };
    if font.len() > 256 {
        return Err(CmdError::Params("font: a family name".into()));
    }
    let base = s.typing_props();
    // A symbol font's character remembers it as its own font (`w:sym`), so changing the text's
    // font later keeps the symbol; other fonts are ordinary run formatting. `w:sym` holds a 16-bit
    // code, so a character beyond the Basic Multilingual Plane stays ordinary text in that font.
    // Each character decides for itself, so a mixed string keeps its symbols and its text apart.
    let font_is_symbol = wordcraft_doc::symbol_fonts::is_symbol_font(font);
    let mut r = Ok(());
    let mut buf = [0u8; 4];
    for ch in c.chars() {
        let mut props = base.clone();
        let private_use = ('\u{F020}'..='\u{F0FF}').contains(&ch);
        if u32::from(ch) <= 0xFFFF && (font_is_symbol || private_use) {
            props.sym_font = Some(font.to_string());
        } else {
            props.font = Some(font.to_string());
        }
        s.pending = Some(props);
        r = type_text(s, ch.encode_utf8(&mut buf));
        if r.is_err() {
            break;
        }
    }
    // Typing on after the symbol goes on in the text's own font.
    s.pending = Some(base);
    r?;
    sel_result(s)
}

pub(super) fn field(s: &mut Session, v: &Value) -> CmdResult {
    let instr = p::req_str(v, "instr")?.trim().to_string();
    if instr.is_empty() || instr.len() > 4096 {
        return Err(CmdError::Params("bad field code".into()));
    }
    let result = p::str(v, "result").unwrap_or("").to_string();
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let end =
        s.doc.insert_object(&at, InlineObject::Field { instr, result, locked: false, dirty: false, code: false, form: String::new() }, &props)?;
    s.sel = Selection::caret(end);
    let _ = crate::cmd::references::update_fields(s);
    sel_result(s)
}

pub(super) fn cover_page(s: &mut Session, v: &Value) -> CmdResult {
    let fallback = |given: Option<&str>, own: &str, default: &str| {
        given.map(str::to_string).or_else(|| (!own.trim().is_empty()).then(|| own.trim().to_string())).unwrap_or_else(|| default.to_string())
    };
    let cover = CoverText {
        title: fallback(p::str(v, "title"), &s.doc.core.title, "Document Title"),
        subtitle: fallback(p::str(v, "subtitle"), &s.doc.core.subject, "Document subtitle"),
        author: fallback(p::str(v, "author"), &s.doc.core.creator, &s.author),
    };
    let design = p::str(v, "design").unwrap_or("studio");
    let blocks = cover_blocks(design, &cover)?.ok_or_else(|| CmdError::Params(format!("unknown cover page design {design:?}")))?;
    let frag = wordcraft_doc::edit::Fragment { blocks, ..Default::default() };
    let start = s.doc.start_of(StoryRef::Body);
    let at = s.doc.split_paragraph(&start)?;
    let _ = at;
    s.doc.insert_fragment(&start, &frag)?;
    s.sel = Selection::caret(s.doc.start_of(StoryRef::Body));
    sel_result(s)
}

/// Why Insert › Drop Cap is off at the caret, as in Word: in a table cell, in an empty paragraph
/// and in one that starts with a space. A selection must be fit throughout (what Dropped and In
/// margin need), unless a paragraph in it has a drop cap for None to take away.
pub(super) fn drop_cap_off(s: &Session) -> Option<&'static str> {
    let why = drop_cap_selection_off(s)?;
    let story = s.sel.focus.story;
    let has_cap = crate::cmd::selected_paths(s).iter().any(|path| s.doc.para(story, path).is_some_and(|p| p.props.drop_cap.is_some()));
    (!has_cap).then_some(why)
}

fn drop_cap_first_letter(first: Option<char>) -> Option<&'static str> {
    match first {
        None => Some("the paragraph is empty"),
        Some(c) if c.is_whitespace() => Some("the paragraph starts with a space"),
        Some(_) => None,
    }
}

/// The same check for every selected paragraph, as the command formats them all.
fn drop_cap_selection_off(s: &Session) -> Option<&'static str> {
    let story = s.sel.focus.story;
    for path in crate::cmd::selected_paths(s) {
        if path.cell().is_some() {
            return Some("drop caps can't go in a table");
        }
        let first = s.doc.para(story, &path).and_then(|p| p.text.chars().next());
        if let Some(why) = drop_cap_first_letter(first) {
            return Some(why);
        }
    }
    None
}

/// Insert › Drop Cap and Drop Cap Options: none, or the letter dropped into the text or hung in
/// the margin, `lines` tall, `distance` points from the text, in `font` (unset: the text's).
/// None takes all of it away, the letter's font with it.
pub(super) fn drop_cap(s: &mut Session, v: &Value) -> CmdResult {
    let lines = v.get("lines").and_then(Value::as_f64).filter(|l| l.is_finite());
    let none = p::str(v, "position") == Some("none") || lines == Some(0.0);
    let margin = p::str(v, "position") == Some("margin");
    let lines_given = lines.is_some();
    let lines = lines.map_or(3.0, |l| l.round().clamp(1.0, 10.0)) as u8;
    let distance = p::f32(v, "distance").map_or(0.0, |d| d.clamp(0.0, wordcraft_doc::resolve::MAX_DROP_CAP_DISTANCE));
    // A family name, not a paragraph of text.
    let font = p::str(v, "font").map(str::trim).filter(|f| !f.is_empty()).map(|f| f.chars().take(64).collect::<String>());
    if !none && let Some(why) = drop_cap_selection_off(s) {
        return Err(CmdError::Failed(why.into()));
    }
    // `keep` (the menu's Dropped / In margin): a paragraph that has a drop cap keeps its own
    // lines, distance and font unless given here.
    let keep = !none && v.get("keep").and_then(Value::as_bool) == Some(true);
    let (distance_given, font_given) = (p::f32(v, "distance").is_some(), v.get("font").is_some());
    crate::cmd::para::fmt(s, &|p| {
        let own = keep && p.drop_cap.is_some();
        p.drop_cap = (!none).then(|| if own && !lines_given { p.drop_cap.unwrap_or(lines) } else { lines });
        p.drop_cap_margin = (!none && margin).then_some(true);
        let d = if own && !distance_given { p.drop_cap_distance.unwrap_or(0.0) } else { distance };
        p.drop_cap_distance = (!none && d > 0.0).then_some(d);
        if !(own && !font_given) {
            p.drop_cap_font = font.clone().filter(|_| !none);
        }
    })
}

/// Insert › Drop Cap at the caret: the menu's highlight and the Drop Cap dialog's values.
#[derive(Clone, Debug, PartialEq)]
pub struct DropCapState {
    /// `none`, `dropped` or `margin`.
    pub position: &'static str,
    /// Lines to drop (3 when there is no drop cap).
    pub lines: u8,
    /// Distance from the text, points.
    pub distance: f32,
    /// The letter's own font, if it has one.
    pub font: Option<String>,
    /// The font of the paragraph's first character.
    pub text_font: String,
}

pub fn drop_cap_state(s: &Session) -> DropCapState {
    let para = s.doc.para_at(&s.sel.focus);
    let props = para.map(|p| p.props.clone()).unwrap_or_default();
    let rp = s.doc.styles.resolve_para(&props);
    let first = para.map(|p| p.props_at(0).clone()).unwrap_or_default();
    let text_font = s.doc.styles.resolve_char_in(props.style.as_deref(), None, &first).font;
    DropCapState {
        position: match (rp.drop_cap, rp.drop_cap_margin) {
            (0, _) => "none",
            (_, true) => "margin",
            _ => "dropped",
        },
        lines: if rp.drop_cap == 0 { 3 } else { rp.drop_cap },
        distance: if rp.drop_cap == 0 { 0.0 } else { rp.drop_cap_distance },
        font: rp.drop_cap_font.filter(|_| rp.drop_cap > 0),
        text_font,
    }
}
