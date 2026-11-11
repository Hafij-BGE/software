//! Cover page designs (Insert > Cover Page): each is a run of paragraphs ending in a page break.

use super::hf_designs::ACCENT;
use super::items::format_date;
use super::{Align, Block, Border, Borders, CharProps, CmdError, Paragraph, Rgb, TextColor};

/// Cover page designs: `(design id, gallery label)`. `studio` is the default.
pub const COVER_DESIGNS: &[(&str, &str)] = &[
    ("studio", "Studio"),
    ("banded", "Banded"),
    ("centered", "Centered"),
    ("sideline", "Sideline"),
    ("framed", "Framed"),
    ("minimal", "Minimal"),
    ("boldBlock", "Bold Block"),
    ("report", "Report"),
    ("academic", "Academic"),
    ("twoTone", "Two-Tone"),
];

const GREY: Rgb = Rgb(0x59, 0x59, 0x59);
const PALE: Rgb = Rgb(0xDC, 0xEA, 0xF2);

/// What a cover page shows.
pub(super) struct CoverText {
    pub title: String,
    pub subtitle: String,
    pub author: String,
}

fn color(c: Rgb) -> Option<TextColor> {
    Some(TextColor::Rgb(c))
}

fn text(t: &str, props: CharProps) -> Paragraph {
    Paragraph::with_text(t, props)
}

fn aligned(mut para: Paragraph, align: Align) -> Paragraph {
    para.props.align = Some(align);
    para
}

fn shaded(mut para: Paragraph, fill: Rgb) -> Paragraph {
    para.props.shading = Some(fill);
    para
}

fn gap(blocks: &mut Vec<Block>, n: usize) {
    for _ in 0..n {
        blocks.push(Block::Para(Paragraph::new()));
    }
}

fn date_text() -> String {
    format_date("MMMM d, yyyy")
}

/// The blocks of cover design `design` (see [`COVER_DESIGNS`]), the last ending in a page break;
/// `None` for an id that isn't one of them.
pub(super) fn cover_blocks(design: &str, c: &CoverText) -> Result<Option<Vec<Block>>, CmdError> {
    let accent = color(ACCENT);
    let white = color(Rgb::WHITE);
    let mut blocks = Vec::new();
    match design {
        "studio" => {
            gap(&mut blocks, 8);
            let mut t = text(&c.title, CharProps { size: Some(44.0), color: accent, ..Default::default() }).styled("Title");
            t.props.borders = Some(Borders { bottom: Some(Border { color: Some(ACCENT), space: 6.0, ..Border::single(2.0) }), ..Default::default() });
            blocks.push(Block::Para(t));
            blocks.push(Block::Para(text(&c.subtitle, CharProps::default()).styled("Subtitle")));
            gap(&mut blocks, 14);
            blocks.push(Block::Para(text(&c.author, CharProps { bold: Some(true), color: accent, ..Default::default() })));
            blocks.push(Block::Para(text(&date_text(), CharProps::default())));
        }
        "banded" => {
            gap(&mut blocks, 6);
            let band = |t: &str, size: f32, bold: bool| {
                let mut p = shaded(text(t, CharProps { size: Some(size), bold: Some(bold), color: white, ..Default::default() }), ACCENT);
                p.props.indent_left = Some(12.0);
                p.props.indent_right = Some(12.0);
                p.props.space_after = Some(0.0);
                p
            };
            blocks.push(Block::Para(band("", 12.0, false)));
            blocks.push(Block::Para(band(&c.title, 40.0, true)));
            blocks.push(Block::Para(band(&c.subtitle, 16.0, false)));
            blocks.push(Block::Para(band("", 12.0, false)));
            gap(&mut blocks, 16);
            blocks.push(Block::Para(text(&c.author, CharProps { bold: Some(true), ..Default::default() })));
            blocks.push(Block::Para(text(&date_text(), CharProps { color: color(GREY), ..Default::default() })));
        }
        "centered" => {
            gap(&mut blocks, 10);
            blocks.push(Block::Para(aligned(text(&c.title, CharProps { size: Some(40.0), bold: Some(true), ..Default::default() }), Align::Center)));
            let mut rule = aligned(text("\u{2014}\u{2014}\u{2014}", CharProps { color: accent, ..Default::default() }), Align::Center);
            rule.props.space_after = Some(12.0);
            blocks.push(Block::Para(rule));
            blocks.push(Block::Para(aligned(
                text(&c.subtitle, CharProps { size: Some(16.0), italic: Some(true), color: color(GREY), ..Default::default() }),
                Align::Center,
            )));
            gap(&mut blocks, 14);
            blocks
                .push(Block::Para(aligned(text(&c.author, CharProps { caps: Some(true), spacing: Some(1.0), ..Default::default() }), Align::Center)));
            blocks.push(Block::Para(aligned(text(&date_text(), CharProps::default()), Align::Center)));
        }
        "sideline" => {
            gap(&mut blocks, 8);
            let line = Borders { left: Some(Border { color: Some(ACCENT), space: 12.0, ..Border::single(6.0) }), ..Default::default() };
            let mut t = text(&c.title, CharProps { size: Some(40.0), color: accent, ..Default::default() });
            t.props.borders = Some(line);
            t.props.space_after = Some(0.0);
            let mut sub = text(&c.subtitle, CharProps { size: Some(16.0), color: color(GREY), ..Default::default() });
            sub.props.borders = Some(line);
            blocks.push(Block::Para(t));
            blocks.push(Block::Para(sub));
            gap(&mut blocks, 16);
            blocks.push(Block::Para(text(&c.author, CharProps { bold: Some(true), color: accent, ..Default::default() })));
            blocks.push(Block::Para(text(&date_text(), CharProps::default())));
        }
        "framed" => {
            gap(&mut blocks, 9);
            let mut t = aligned(text(&c.title, CharProps { size: Some(36.0), caps: Some(true), color: accent, ..Default::default() }), Align::Center);
            t.props.borders = Some(Borders::box_(Border { color: Some(ACCENT), space: 12.0, ..Border::single(1.5) }));
            t.props.space_after = Some(18.0);
            blocks.push(Block::Para(t));
            blocks.push(Block::Para(aligned(text(&c.subtitle, CharProps { size: Some(14.0), ..Default::default() }), Align::Center)));
            gap(&mut blocks, 15);
            blocks.push(Block::Para(aligned(text(&c.author, CharProps { bold: Some(true), ..Default::default() }), Align::Center)));
            blocks.push(Block::Para(aligned(text(&date_text(), CharProps { color: color(GREY), ..Default::default() }), Align::Center)));
        }
        "minimal" => {
            blocks.push(Block::Para(aligned(
                text(&date_text(), CharProps { size: Some(10.0), color: color(GREY), ..Default::default() }),
                Align::Right,
            )));
            gap(&mut blocks, 14);
            blocks.push(Block::Para(text(&c.title, CharProps { size: Some(36.0), ..Default::default() })));
            blocks.push(Block::Para(text(&c.subtitle, CharProps { size: Some(14.0), color: color(GREY), ..Default::default() })));
            gap(&mut blocks, 12);
            blocks.push(Block::Para(text(&c.author, CharProps { size: Some(10.0), caps: Some(true), spacing: Some(1.5), ..Default::default() })));
        }
        "boldBlock" => {
            let block = |t: &str, size: f32, bold: bool| {
                let mut p = shaded(text(t, CharProps { size: Some(size), bold: Some(bold), color: white, ..Default::default() }), ACCENT);
                p.props.indent_left = Some(18.0);
                p.props.space_after = Some(0.0);
                p
            };
            for _ in 0..9 {
                blocks.push(Block::Para(block("", 14.0, false)));
            }
            blocks.push(Block::Para(block(&c.title, 48.0, true)));
            blocks.push(Block::Para(block(&c.subtitle, 18.0, false)));
            for _ in 0..3 {
                blocks.push(Block::Para(block("", 14.0, false)));
            }
            gap(&mut blocks, 6);
            blocks.push(Block::Para(text(&c.author, CharProps { size: Some(14.0), bold: Some(true), color: accent, ..Default::default() })));
            blocks.push(Block::Para(text(&date_text(), CharProps::default())));
        }
        "report" => {
            let mut top =
                text(&c.author, CharProps { size: Some(10.0), caps: Some(true), spacing: Some(1.0), color: color(GREY), ..Default::default() });
            top.props.borders =
                Some(Borders { bottom: Some(Border { color: Some(GREY), space: 4.0, ..Border::single(0.75) }), ..Default::default() });
            blocks.push(Block::Para(top));
            gap(&mut blocks, 12);
            blocks.push(Block::Para(text(&c.title, CharProps { size: Some(40.0), bold: Some(true), ..Default::default() })));
            blocks.push(Block::Para(text(&c.subtitle, CharProps { size: Some(16.0), color: accent, ..Default::default() })));
            gap(&mut blocks, 16);
            let mut by = text("Prepared by ", CharProps { color: color(GREY), ..Default::default() });
            by.insert_text(by.len(), &c.author, &CharProps { bold: Some(true), ..Default::default() })?;
            by.props.borders = Some(Borders { top: Some(Border { color: Some(ACCENT), space: 4.0, ..Border::single(1.5) }), ..Default::default() });
            blocks.push(Block::Para(by));
            blocks.push(Block::Para(text(&date_text(), CharProps { color: color(GREY), ..Default::default() })));
        }
        "academic" => {
            let center = |t: &str, props: CharProps| aligned(text(t, props), Align::Center);
            gap(&mut blocks, 2);
            blocks.push(Block::Para(center(
                "[Institution name]",
                CharProps { size: Some(16.0), caps: Some(true), bold: Some(true), ..Default::default() },
            )));
            blocks.push(Block::Para(center("[Department or course]", CharProps { size: Some(12.0), ..Default::default() })));
            gap(&mut blocks, 8);
            blocks.push(Block::Para(center(&c.title, CharProps { size: Some(28.0), bold: Some(true), ..Default::default() })));
            blocks.push(Block::Para(center(&c.subtitle, CharProps { size: Some(14.0), italic: Some(true), ..Default::default() })));
            gap(&mut blocks, 8);
            blocks.push(Block::Para(center("Submitted by", CharProps { color: color(GREY), ..Default::default() })));
            blocks.push(Block::Para(center(&c.author, CharProps { size: Some(14.0), bold: Some(true), ..Default::default() })));
            gap(&mut blocks, 4);
            blocks.push(Block::Para(center(&date_text(), CharProps::default())));
        }
        "twoTone" => {
            gap(&mut blocks, 8);
            let mut t = shaded(text(&c.title, CharProps { size: Some(40.0), bold: Some(true), color: white, ..Default::default() }), ACCENT);
            t.props.indent_left = Some(12.0);
            t.props.space_after = Some(0.0);
            let mut sub = shaded(text(&c.subtitle, CharProps { size: Some(16.0), color: accent, ..Default::default() }), PALE);
            sub.props.indent_left = Some(12.0);
            blocks.push(Block::Para(t));
            blocks.push(Block::Para(sub));
            gap(&mut blocks, 16);
            blocks.push(Block::Para(text(&c.author, CharProps { bold: Some(true), color: accent, ..Default::default() })));
            blocks.push(Block::Para(text(&date_text(), CharProps::default())));
        }
        _ => return Ok(None),
    }
    // The cover ends with a page break, so the document starts on the next page.
    let mut last = Paragraph::new();
    if let Some(Block::Para(p)) = blocks.pop() {
        last = p;
    }
    last.insert_text(last.len(), "\u{000C}", &CharProps::default())?;
    blocks.push(Block::Para(last));
    Ok(Some(blocks))
}
