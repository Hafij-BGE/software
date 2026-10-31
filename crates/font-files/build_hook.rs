//! Build-script half of craft-font-files, shared by the WordCraft, DeckCraft and GridCraft font
//! build scripts through `#[path]` (`mod shared_fonts;`).
//!
//! [`rewrite`] takes the generated `craft_fonts.rs` source. With `CRAFT_FONTS_SHARED` set (the
//! folder the package installs the craft-fonts checkout `CRAFT_FONTS_DIR` to), each
//! `include_bytes!("<CRAFT_FONTS_DIR>/<file>")` becomes
//! `::craft_font_files::map("<CRAFT_FONTS_SHARED>/<file>")` and `CRAFT_FONTS` becomes a
//! `LazyLock` over the faces whose file could be mapped. Unset, or on wasm, the source is returned
//! as it is and the fonts stay embedded.

/// Rewrites the generated craft-fonts table to map files from `CRAFT_FONTS_SHARED`; see the module docs.
pub fn rewrite(src: String) -> String {
    println!("cargo::rerun-if-env-changed=CRAFT_FONTS_SHARED");
    let Some(shared) = std::env::var("CRAFT_FONTS_SHARED").ok().filter(|s| !s.is_empty()) else {
        return src;
    };
    if std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|a| a == "wasm32") {
        return src;
    }
    let Some(dir) = std::env::var_os("CRAFT_FONTS_DIR").and_then(|d| std::path::PathBuf::from(d).canonicalize().ok()) else {
        return src;
    };
    let head = "pub static CRAFT_FONTS: &[CraftFont] = &[\n";
    let Some(start) = src.find(head) else {
        println!("cargo::error=CRAFT_FONTS_SHARED: generated craft_fonts.rs has no CRAFT_FONTS table");
        return src;
    };
    let body_start = start + head.len();
    let Some(body_len) = src[body_start..].find("];\n") else {
        println!("cargo::error=CRAFT_FONTS_SHARED: unterminated CRAFT_FONTS table");
        return src;
    };
    let body = &src[body_start..body_start + body_len];
    let mut entries = String::new();
    let mut rest = body;
    while let Some(at) = rest.find("include_bytes!(\"") {
        entries.push_str(&rest[..at]);
        let after = &rest[at + "include_bytes!(\"".len()..];
        let Some((path, len)) = unescape(after) else {
            println!("cargo::error=CRAFT_FONTS_SHARED: cannot read a font path in craft_fonts.rs");
            return src;
        };
        let Some(rel) = std::path::Path::new(&path).strip_prefix(&dir).ok().and_then(|r| r.to_str()) else {
            println!("cargo::error=CRAFT_FONTS_SHARED: {path} is outside CRAFT_FONTS_DIR={}", dir.display());
            return src;
        };
        let shared_path = format!("{}/{rel}", shared.trim_end_matches('/'));
        entries.push_str(&format!("::craft_font_files::map({shared_path:?})"));
        // Skip the closing `")`, the end of the `include_bytes!` call.
        rest = &after[len + 2..];
    }
    entries.push_str(rest);
    format!(
        "{}pub static CRAFT_FONTS: ::std::sync::LazyLock<&'static [CraftFont]> = ::std::sync::LazyLock::new(|| {{\n\
         let all: ::std::vec::Vec<CraftFont> = ::std::vec![\n{entries}];\n\
         all.into_iter().filter(|f| !f.bytes.is_empty()).collect::<::std::vec::Vec<_>>().leak()\n}});\n{}",
        &src[..start],
        &src[body_start + body_len + "];\n".len()..],
    )
}

/// Reads a Rust string literal body up to its closing quote, undoing the `{:?}` escapes a path can
/// carry. Returns the text and the number of source bytes before the closing quote.
fn unescape(s: &str) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, i)),
            '\\' => match chars.next()?.1 {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                '\'' => out.push('\''),
                _ => return None,
            },
            c => out.push(c),
        }
    }
    None
}
