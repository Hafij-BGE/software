//! Font files shared by every Craft Suite binary.
//!
//! The Linux package installs craft-fonts once, under `/usr/share/craft-suite/fonts`, instead of
//! embedding a copy in each binary. When a font crate is built with `CRAFT_FONTS_SHARED` set,
//! `build_hook.rs` turns its `include_bytes!` table into calls to [`map`], so the font tables keep
//! their `&'static [u8]` type. The files are memory-mapped: like embedded data, pages are read
//! on first use and shared between processes.

/// Maps the font file at `path` read-only for the rest of the process. A missing or unreadable
/// file is logged and gives an empty slice; the generated tables drop such faces.
pub fn map(path: &str) -> &'static [u8] {
    match open(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::warn!("craft-fonts: {path}: {e}");
            &[]
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open(path: &str) -> std::io::Result<&'static [u8]> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() == 0 {
        return Ok(&[]);
    }
    // SAFETY: the mapping is read-only and the files are root-owned package files. dpkg and the
    // updater replace them by rename, so a running process keeps the old inode mapped; nothing
    // truncates or rewrites them in place.
    let map = unsafe { memmap2::Mmap::map(&file)? };
    Ok(&Box::leak(Box::new(map))[..])
}

#[cfg(target_arch = "wasm32")]
fn open(_path: &str) -> std::io::Result<&'static [u8]> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn maps_file_and_tolerates_missing() {
        let dir = std::env::temp_dir().join(format!("craft-font-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.ttf");
        std::fs::write(&path, b"font").unwrap();
        assert_eq!(super::map(path.to_str().unwrap()), b"font");
        assert!(super::map(dir.join("missing.ttf").to_str().unwrap()).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
