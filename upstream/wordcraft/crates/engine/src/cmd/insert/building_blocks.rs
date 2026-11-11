//! Built-in Quick Parts (Insert > Quick Parts > Building Blocks): ready-made blocks of text with
//! placeholders to type over, inserted at the caret.

use super::hf_designs::ACCENT;
use super::items::format_date;
use super::{Block, CharProps, CmdError, CmdResult, Paragraph, Rgb, Selection, Session, TextColor, Value, delete_selection, p, sel_result};

/// Built-in building blocks: `(name, menu label)`.
pub const BUILT_IN_BLOCKS: &[(&str, &str)] = &[
    ("signature", "Signature Block"),
    ("address", "Address Block"),
    ("contact", "Contact Details"),
    ("approval", "Approval Lines"),
    ("disclaimer", "Disclaimer"),
    ("confidential", "Confidentiality Notice"),
];

const GREY: Rgb = Rgb(0x59, 0x59, 0x59);

fn para(text: &str, props: CharProps) -> Block {
    Block::Para(Paragraph::with_text(text, props))
}

fn plain(text: &str) -> Block {
    para(text, CharProps::default())
}

fn author_name(s: &Session) -> String {
    let creator = s.doc.core.creator.trim();
    let name = if creator.is_empty() { s.author.trim() } else { creator };
    if name.is_empty() { "[Your name]".into() } else { name.to_string() }
}

/// The blocks of built-in building block `name`; `None` for a name that isn't one.
fn blocks(s: &Session, name: &str) -> Option<Vec<Block>> {
    let bold = CharProps { bold: Some(true), ..Default::default() };
    let note = CharProps { size: Some(9.0), italic: Some(true), color: Some(TextColor::Rgb(GREY)), ..Default::default() };
    Some(match name {
        "signature" => vec![
            plain("Sincerely,"),
            plain(""),
            plain(""),
            plain("______________________________"),
            para(&author_name(s), bold),
            plain("[Job title]"),
            plain(&format_date("MMMM d, yyyy")),
        ],
        "address" => vec![para(&author_name(s), bold), plain("[Street address]"), plain("[City, State ZIP]"), plain("[Country]")],
        "contact" => vec![
            para("Contact", CharProps { color: Some(TextColor::Rgb(ACCENT)), ..bold.clone() }),
            plain(&author_name(s)),
            plain("Phone: [Phone number]"),
            plain("Email: [Email address]"),
            plain("Web: [Website]"),
        ],
        "approval" => vec![
            plain("Prepared by: ______________________    Date: ____________"),
            plain("Reviewed by: ______________________    Date: ____________"),
            plain("Approved by: ______________________    Date: ____________"),
        ],
        "disclaimer" => vec![para(
            "This document is provided for information only. While care has been taken to make it accurate, no guarantee is given, and the \
             authors accept no liability for decisions made on the strength of it.",
            note,
        )],
        "confidential" => vec![
            para("CONFIDENTIAL", CharProps { color: Some(TextColor::Rgb(Rgb(0xC0, 0x00, 0x00))), spacing: Some(1.5), ..bold }),
            para(
                "This document is meant only for the people it was sent to. If you received it by mistake, please tell the sender and delete \
                 it; do not copy, share or act on its contents.",
                note,
            ),
        ],
        _ => return None,
    })
}

/// `insert.buildingBlock`: puts built-in building block `name` at the caret (replacing the
/// selection).
pub(super) fn building_block(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?;
    let blocks = blocks(s, name).ok_or_else(|| CmdError::Params(format!("no built-in building block `{name}`")))?;
    let frag = wordcraft_doc::edit::Fragment { blocks, ..Default::default() };
    let at = delete_selection(s)?;
    let end = s.doc.insert_fragment(&at, &frag)?;
    s.sel = Selection::caret(end);
    sel_result(s)
}
