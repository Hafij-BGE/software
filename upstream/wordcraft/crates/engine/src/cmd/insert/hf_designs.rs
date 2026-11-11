//! Ready-made header and footer designs (Insert > Header / Footer gallery) and page-number styles.
//!
//! Every design fills the header or footer with one paragraph built from document fields (TITLE,
//! AUTHOR, DATE, PAGE, NUMPAGES, STYLEREF, FILENAME), so it follows the document as it changes.

use super::header_footer::text_width;
use super::items::format_date;
use super::{Align, Border, Borders, CharProps, CmdError, InlineObject, Paragraph, Rgb, Session, TabAlign, TabStop, TextColor, para_block};

/// Header and footer designs beyond Blank, Blank (Three Columns), Title and Page Number:
/// `(preset id, gallery label)`. Each works as a header and as a footer.
pub const HF_DESIGNS: &[(&str, &str)] = &[
    ("accentBar", "Accent Bar"),
    ("banded", "Banded"),
    ("dateTitle", "Date and Title"),
    ("authorPage", "Author | Page"),
    ("pageXofY", "Page X of Y"),
    ("chapter", "Chapter and Page"),
    ("rule", "Rule Line"),
    ("confidential", "Confidential"),
    ("twoTone", "Two-Tone"),
    ("minimalDot", "Minimal Dot"),
    ("sideline", "Sideline"),
    ("framed", "Framed"),
    ("fileDate", "File and Date"),
    ("dash", "Dashed Number"),
];

/// Page-number styles for `insert.pageNumber` `style`: `(id, menu label)`.
pub const PAGE_NUMBER_STYLES: &[(&str, &str)] = &[
    ("plain", "Plain Number"),
    ("pageX", "Page X"),
    ("xOfY", "Page X of Y"),
    ("slash", "X / Y"),
    ("dash", "- X -"),
    ("brackets", "[X]"),
    ("parens", "(X)"),
    ("bold", "Bold Number"),
    ("accent", "Accent Number"),
    ("boxed", "Boxed Number"),
    ("circle", "Circled Number"),
    ("pill", "Accent Tab"),
    ("roman", "Roman Numeral"),
    ("words", "Number in Words"),
];

pub(super) const ACCENT: Rgb = Rgb(0x15, 0x60, 0x82);
const GREY: Rgb = Rgb(0x59, 0x59, 0x59);
const ALERT: Rgb = Rgb(0xC0, 0x00, 0x00);
const WHITE: Rgb = Rgb(0xFF, 0xFF, 0xFF);

pub(super) fn colored(c: Rgb) -> CharProps {
    CharProps { color: Some(TextColor::Rgb(c)), ..Default::default() }
}

fn bold(c: Rgb) -> CharProps {
    CharProps { bold: Some(true), ..colored(c) }
}

pub(super) fn push_text(para: &mut Paragraph, text: &str, props: &CharProps) -> Result<(), CmdError> {
    let end = para.len();
    para.insert_text(end, text, props)?;
    Ok(())
}

pub(super) fn push_field(para: &mut Paragraph, instr: &str, result: &str, props: &CharProps) -> Result<(), CmdError> {
    let end = para.len();
    let field = InlineObject::Field { instr: instr.into(), result: result.into(), locked: false, dirty: false, code: false, form: String::new() };
    para.insert_object(end, field, props)?;
    Ok(())
}

/// The document title as a TITLE field, or a placeholder to type over when there is none.
pub(super) fn push_title(s: &Session, para: &mut Paragraph, props: &CharProps) -> Result<(), CmdError> {
    let title = s.doc.core.title.trim();
    if title.is_empty() { push_text(para, "[Document title]", props) } else { push_field(para, "TITLE", title, props) }
}

/// The author as an AUTHOR field, else the user's name, else a placeholder.
pub(super) fn push_author(s: &Session, para: &mut Paragraph, props: &CharProps) -> Result<(), CmdError> {
    let creator = s.doc.core.creator.trim();
    if !creator.is_empty() {
        push_field(para, "AUTHOR", creator, props)
    } else if !s.author.trim().is_empty() {
        push_text(para, s.author.trim(), props)
    } else {
        push_text(para, "[Author name]", props)
    }
}

pub(super) fn push_date(para: &mut Paragraph, props: &CharProps) -> Result<(), CmdError> {
    let fmt = "MMMM d, yyyy";
    push_field(para, &format!("DATE \\@ \"{fmt}\""), &format_date(fmt), props)
}

pub(super) fn push_page(para: &mut Paragraph, props: &CharProps) -> Result<(), CmdError> {
    push_field(para, "PAGE", "1", props)
}

fn push_pages(para: &mut Paragraph, props: &CharProps) -> Result<(), CmdError> {
    push_field(para, "NUMPAGES", "1", props)
}

fn push_page_of(para: &mut Paragraph, text: &CharProps, num: &CharProps) -> Result<(), CmdError> {
    push_text(para, "Page ", text)?;
    push_page(para, num)?;
    push_text(para, " of ", text)?;
    push_pages(para, num)
}

/// Left text, a center tab stop and a right tab stop spanning the section's text width.
fn tabbed(s: &Session, style: &str) -> Paragraph {
    let width = text_width(s);
    let mut para = Paragraph::new().styled(style);
    para.props.tabs = Some(vec![
        TabStop { pos: width / 2.0, align: TabAlign::Center, leader: Default::default() },
        TabStop { pos: width, align: TabAlign::Right, leader: Default::default() },
    ]);
    para
}

fn centered(style: &str) -> Paragraph {
    let mut para = Paragraph::new().styled(style);
    para.props.align = Some(Align::Center);
    para
}

/// The rule between the header (or footer) and the body: below a header, above a footer.
fn rule(para: &mut Paragraph, header: bool, border: Border) {
    let borders = if header { Borders { bottom: Some(border), ..Default::default() } } else { Borders { top: Some(border), ..Default::default() } };
    para.props.borders = Some(borders);
}

/// The blocks of design `id` (see [`HF_DESIGNS`]); `None` for an id that isn't one of them.
pub(super) fn design(s: &Session, id: &str, header: bool) -> Result<Option<wordcraft_doc::Blocks>, CmdError> {
    let style = if header { "Header" } else { "Footer" };
    let plain = CharProps::default();
    let grey = colored(GREY);
    let para = match id {
        "accentBar" => {
            let mut para = tabbed(s, style);
            if header {
                push_title(s, &mut para, &bold(ACCENT))?;
                push_text(&mut para, "\t\t", &plain)?;
                push_date(&mut para, &colored(ACCENT))?;
            } else {
                push_title(s, &mut para, &colored(ACCENT))?;
                push_text(&mut para, "\t\t", &plain)?;
                push_page(&mut para, &bold(ACCENT))?;
            }
            rule(&mut para, header, Border { width: 2.25, color: Some(ACCENT), space: 4.0, ..Border::single(2.25) });
            para
        }
        "banded" => {
            let mut para = tabbed(s, style);
            para.props.shading = Some(ACCENT);
            let white = CharProps { caps: Some(true), ..bold(WHITE) };
            if header {
                push_title(s, &mut para, &white)?;
                push_text(&mut para, "\t\t", &plain)?;
                push_date(&mut para, &colored(WHITE))?;
            } else {
                push_author(s, &mut para, &colored(WHITE))?;
                push_text(&mut para, "\t\t", &plain)?;
                push_text(&mut para, "Page ", &colored(WHITE))?;
                push_page(&mut para, &bold(WHITE))?;
            }
            para
        }
        "dateTitle" => {
            let mut para = tabbed(s, style);
            push_date(&mut para, &grey)?;
            push_text(&mut para, "\t", &plain)?;
            push_title(s, &mut para, &CharProps { bold: Some(true), ..Default::default() })?;
            if !header {
                push_text(&mut para, "\t", &plain)?;
                push_page(&mut para, &plain)?;
            }
            para
        }
        "authorPage" => {
            let mut para = Paragraph::new().styled(style);
            para.props.align = Some(Align::Right);
            push_author(s, &mut para, &grey)?;
            push_text(&mut para, "  |  ", &colored(ACCENT))?;
            push_text(&mut para, "Page ", &plain)?;
            push_page(&mut para, &CharProps { bold: Some(true), ..Default::default() })?;
            para
        }
        "pageXofY" => {
            let mut para = centered(style);
            push_page_of(&mut para, &plain, &CharProps { bold: Some(true), ..Default::default() })?;
            para
        }
        "chapter" => {
            let mut para = tabbed(s, style);
            push_field(&mut para, "STYLEREF \"Heading 1\"", "", &CharProps { italic: Some(true), ..grey.clone() })?;
            push_text(&mut para, "\t\t", &plain)?;
            push_page(&mut para, &bold(ACCENT))?;
            para
        }
        "rule" => {
            let mut para = centered(style);
            if header {
                push_title(s, &mut para, &CharProps { small_caps: Some(true), spacing: Some(1.0), ..Default::default() })?;
            } else {
                push_page(&mut para, &plain)?;
            }
            rule(&mut para, header, Border { color: Some(GREY), space: 1.0, ..Border::single(0.75) });
            para
        }
        "confidential" => {
            let mut para = tabbed(s, style);
            push_text(&mut para, "CONFIDENTIAL", &CharProps { spacing: Some(1.5), ..bold(ALERT) })?;
            push_text(&mut para, "\t", &plain)?;
            if header {
                push_text(&mut para, "\t", &plain)?;
                push_date(&mut para, &grey)?;
            } else {
                push_date(&mut para, &grey)?;
                push_text(&mut para, "\t", &plain)?;
                push_page_of(&mut para, &plain, &plain)?;
            }
            para
        }
        "twoTone" => {
            let mut para = tabbed(s, style);
            let band = CharProps { shading: Some(ACCENT), ..bold(WHITE) };
            if header {
                push_text(&mut para, " ", &band)?;
                push_title(s, &mut para, &band)?;
                push_text(&mut para, " \t\t", &band)?;
                push_date(&mut para, &colored(ACCENT))?;
            } else {
                push_text(&mut para, " ", &band)?;
                push_page(&mut para, &band)?;
                push_text(&mut para, " ", &band)?;
                push_text(&mut para, "\t\t", &plain)?;
                push_title(s, &mut para, &colored(ACCENT))?;
            }
            para
        }
        "minimalDot" => {
            let mut para = centered(style);
            let small = CharProps { size: Some(9.0), ..grey.clone() };
            if header {
                push_title(s, &mut para, &small)?;
                push_text(&mut para, "  \u{00B7}  ", &colored(ACCENT))?;
                push_author(s, &mut para, &small)?;
            } else {
                push_text(&mut para, "\u{00B7}  ", &colored(ACCENT))?;
                push_page(&mut para, &small)?;
                push_text(&mut para, "  \u{00B7}", &colored(ACCENT))?;
            }
            para
        }
        "sideline" => {
            let mut para = Paragraph::new().styled(style);
            para.props.borders =
                Some(Borders { left: Some(Border { color: Some(ACCENT), space: 6.0, ..Border::single(4.5) }), ..Default::default() });
            if header {
                push_title(s, &mut para, &bold(ACCENT))?;
                push_text(&mut para, "  |  ", &grey)?;
                push_author(s, &mut para, &grey)?;
            } else {
                push_page(&mut para, &bold(ACCENT))?;
                push_text(&mut para, "  |  ", &grey)?;
                push_title(s, &mut para, &grey)?;
            }
            para
        }
        "framed" => {
            let mut para = centered(style);
            para.props.borders = Some(Borders::box_(Border { color: Some(ACCENT), space: 2.0, ..Border::single(0.75) }));
            if header {
                push_title(s, &mut para, &CharProps { caps: Some(true), ..colored(ACCENT) })?;
            } else {
                push_text(&mut para, "Page ", &colored(ACCENT))?;
                push_page(&mut para, &bold(ACCENT))?;
            }
            para
        }
        "fileDate" => {
            let mut para = tabbed(s, style);
            let small = CharProps { size: Some(9.0), ..grey.clone() };
            push_field(&mut para, "FILENAME", "", &small)?;
            push_text(&mut para, "\t\t", &plain)?;
            push_date(&mut para, &small)?;
            para
        }
        "dash" => {
            let mut para = centered(style);
            push_field(&mut para, "PAGE \\* ArabicDash", "- 1 -", &colored(ACCENT))?;
            para
        }
        _ => return Ok(None),
    };
    Ok(Some(vec![para_block(para)]))
}

/// Appends page-number style `style` (see [`PAGE_NUMBER_STYLES`]) to `para`; an error for an
/// unknown style.
pub(super) fn push_page_number(para: &mut Paragraph, style: &str) -> Result<(), CmdError> {
    let plain = CharProps::default();
    let strong = CharProps { bold: Some(true), ..Default::default() };
    match style {
        "plain" => push_page(para, &plain),
        "pageX" => {
            push_text(para, "Page ", &plain)?;
            push_page(para, &plain)
        }
        // "Page X of Y" as Word builds it: the numbers bold.
        "xOfY" => push_page_of(para, &plain, &strong),
        "slash" => {
            push_page(para, &plain)?;
            push_text(para, " / ", &plain)?;
            push_pages(para, &plain)
        }
        "dash" => push_field(para, "PAGE \\* ArabicDash", "- 1 -", &plain),
        "brackets" | "parens" => {
            let (open, close) = if style == "brackets" { ("[", "]") } else { ("(", ")") };
            push_text(para, open, &plain)?;
            push_page(para, &plain)?;
            push_text(para, close, &plain)
        }
        "bold" => push_page(para, &CharProps { size: Some(14.0), ..strong }),
        "accent" => push_page(para, &CharProps { size: Some(14.0), ..bold(ACCENT) }),
        "boxed" => {
            let props = CharProps { border: Some(Border { color: Some(ACCENT), space: 1.0, ..Border::single(0.75) }), ..colored(ACCENT) };
            push_text(para, "\u{00A0}", &props)?;
            push_page(para, &props)?;
            push_text(para, "\u{00A0}", &props)
        }
        // No field switch draws a circled number: the caller sets the section's page-number
        // format to decimalEnclosedCircle, as Word does.
        "circle" => push_page(para, &CharProps { size: Some(14.0), ..colored(ACCENT) }),
        "pill" => {
            let props = CharProps { shading: Some(ACCENT), ..bold(WHITE) };
            push_text(para, "\u{00A0}\u{00A0}", &props)?;
            push_page(para, &props)?;
            push_text(para, "\u{00A0}\u{00A0}", &props)
        }
        "roman" => push_field(para, "PAGE \\* ROMAN", "I", &plain),
        "words" => push_field(para, "PAGE \\* CardText \\* FirstCap", "One", &plain),
        other => Err(CmdError::Params(format!("unknown page number style {other:?}"))),
    }
}
