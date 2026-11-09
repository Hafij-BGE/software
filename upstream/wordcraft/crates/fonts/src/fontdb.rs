//! Font database: bundled OFL fonts, user fonts, the installed system fonts (cataloged the first
//! time a lookup needs them, native only), outline cache.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use kurbo::BezPath;
use skrifa::instance::{Location, LocationRef, Size};
use skrifa::outline::{
    DrawSettings, Engine, GlyphStyles, Hinting, HintingInstance, HintingOptions, OutlineGlyphFormat, OutlinePen, SmoothMode, Target,
};
use skrifa::raw::FileRef;
use skrifa::string::StringId;
use skrifa::{GlyphId, MetadataProvider};

pub(crate) mod user;
pub use user::{FONT_FILE_EXTS, FontFormat, Installed, MAX_FONT_FILE_BYTES, UserFont, UserFontError, decode_font_file, is_font_file_name, sniff};
#[cfg(not(target_arch = "wasm32"))]
pub use user::{font_files_under, windows_font_dirs, windows_font_dirs_under};

mod catalog;
mod face;
mod fallback;
mod fallback_data;
mod lookup;
mod names;

use face::{BUNDLED, FontBytes};
pub use face::{FALLBACK_FAMILY, FaceRef, FontFace, bundled};
#[cfg(not(target_arch = "wasm32"))]
use fallback_data::{CoverageIndex, SysFallback};
use fallback_data::{FallbackCache, RunFallbackCache};
#[cfg(test)]
use fallback_data::{RUN_FALLBACK_CACHE_CHARS, coverage_ranges, ranges_hold};
#[cfg(not(target_arch = "wasm32"))]
use names::CatalogEntry;
pub(crate) use names::face_names;
pub use names::{LocalName, base_style, name_language_matches, system_font_dirs};
use names::{MAX_FILE_FACES, enumerate_faces, make_face};
#[cfg(test)]
use names::{MAX_NAME_CHARS, file_face_names, font_faces, mini_font, name};
#[cfg(test)]
pub(crate) use names::{last_resort_face, legacy_family};

static GLOBAL: std::sync::OnceLock<FontDb> = std::sync::OnceLock::new();

/// Process-wide font database.
pub struct FontDb {
    faces: RwLock<Vec<Arc<FontFace>>>,
    outlines: Mutex<HashMap<(u32, u32), Arc<BezPath>>>,
    /// Screen-hinted outlines per (face, glyph, ppem bits); `None` where the face can't be hinted.
    hinted: Mutex<HashMap<(u32, u32, u32), Option<Arc<BezPath>>>>,
    /// Light hinting instances per (face, ppem bits); `None` where the face can't be hinted.
    hinters: Mutex<HashMap<(u32, u32), Option<Arc<HintingInstance>>>>,
    /// Fallback faces found per (character, excluded face), valid while the face count is the
    /// one stored (faces are only ever added). Layout asks once per uncovered character, and the
    /// search sorts every loaded face, so without this text in a script the requested font lacks
    /// (Thai, #59) got slower with every font loaded.
    fallbacks: RwLock<FallbackCache>,
    run_fallbacks: RwLock<RunFallbackCache>,
    #[cfg(not(target_arch = "wasm32"))]
    catalog: RwLock<Vec<CatalogEntry>>,
    /// Fonts the user added at run time, and the folder they are stored in (#582).
    user: RwLock<user::UserFonts>,
    /// Bumped whenever user fonts are added or removed ([`FontDb::generation`]).
    generation: AtomicU64,
    /// The folders the system font scan reads (the platform's font folders for [`FontDb::global`]).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    font_dirs: Vec<std::path::PathBuf>,
    /// Set once the font folders have been scanned. Lookups by family name wait for the first
    /// scan, so what they find doesn't depend on what ran before them.
    #[cfg(not(target_arch = "wasm32"))]
    cataloged: std::sync::OnceLock<()>,
    /// Held while an installed family's files load, so two lookups don't load it twice.
    #[cfg(not(target_arch = "wasm32"))]
    loading: Mutex<()>,
    /// System fallback state: enabled, and characters no system font covers.
    #[cfg(not(target_arch = "wasm32"))]
    sys: Mutex<SysFallback>,
    /// Held through a system fallback search, so searches run one at a time. Without it a search
    /// racing another that was still loading a preferred family ("Padauk" for Myanmar) skipped
    /// that family as already tried and loaded whatever the coverage index found first (only
    /// Padauk Bold), so which face a character fell back to depended on thread timing.
    #[cfg(not(target_arch = "wasm32"))]
    searching: Mutex<()>,
    /// The characters each installed font file maps, read the first time a fallback search
    /// needs that file. Held through a search, so two searches never read a file twice.
    #[cfg(not(target_arch = "wasm32"))]
    coverage: Mutex<CoverageIndex>,
    /// Counts the font folder scans, so the coverage index notices a rescan.
    #[cfg(not(target_arch = "wasm32"))]
    catalog_gen: AtomicU32,
}

<<<<<<< HEAD
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct SysFallback {
    enabled: bool,
    misses: std::collections::HashSet<char>,
    /// Preferred fallback families already loaded (or tried) by a fallback search.
    tried: std::collections::HashSet<&'static str>,
}

/// The longest name a document's font table gives an embedded font that faces take on.
const MAX_FAMILY_NAME: usize = 128;

/// Characters remembered as covered by no installed font (a cap on memory: once full the set
/// starts over, and the coverage index answers again quickly).
#[cfg(not(target_arch = "wasm32"))]
const SYS_MISSES_MAX: usize = 65_536;

/// Installed font files larger than this are not searched for fallback characters.
#[cfg(not(target_arch = "wasm32"))]
const SYS_FALLBACK_MAX_FILE: u64 = 40 << 20;

/// Which characters each cataloged font file maps (#605). Before it, every distinct character no
/// loaded font covered read and parsed every installed font file again: 500 distinct Tangut
/// characters froze the app for most of a minute. Now each file is read once, when a search
/// first gets to it, and later searches answer from its ranges, reading only a file that matches.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct CoverageIndex {
    /// The catalog scan the file list was taken from (`None` until the first search).
    generation: Option<u32>,
    /// The cataloged font files, sorted.
    files: Vec<IndexedFile>,
    /// Font files read by fallback searches (each at most once to index it, once more to load it).
    reads: usize,
    /// Files loaded by family name ([`FontDb::load_cataloged`]), whether or not they are indexed
    /// yet: a search never loads them a second time.
    loaded: std::collections::HashSet<std::path::PathBuf>,
}

#[cfg(not(target_arch = "wasm32"))]
struct IndexedFile {
    path: std::path::PathBuf,
    /// The code points the file's faces map, as sorted, disjoint inclusive ranges (`None` until
    /// the file is read; empty when it can't be read or is too large).
    ranges: Option<Box<[(u32, u32)]>>,
    /// Loaded for a character it covers: its faces are searched as loaded fonts from then on.
    used: bool,
}

/// Does `ranges` (sorted, disjoint, inclusive) hold `cp`?
#[cfg(not(target_arch = "wasm32"))]
fn ranges_hold(ranges: &[(u32, u32)], cp: u32) -> bool {
    let i = ranges.partition_point(|r| r.1 < cp);
    ranges.get(i).is_some_and(|r| r.0 <= cp)
}

/// The code points mapped by the faces of `data` (a font file or collection), as sorted,
/// disjoint, inclusive ranges. Bounded however damaged the file: at most 256 faces, a budget of
/// mappings read across them, and a cap on the ranges kept.
#[cfg(not(target_arch = "wasm32"))]
fn coverage_ranges(data: &[u8]) -> Box<[(u32, u32)]> {
    const MAX_FACES: u32 = 256;
    const MAX_MAPPINGS: usize = 4 * 0x11_0000;
    const MAX_RANGES: usize = 1 << 17;
    fn compact(r: &mut Vec<(u32, u32)>) {
        r.sort_unstable();
        let mut out: Vec<(u32, u32)> = Vec::with_capacity(r.len());
        for &(s, e) in r.iter() {
            match out.last_mut() {
                Some(last) if s <= last.1.saturating_add(1) => last.1 = last.1.max(e),
                _ => out.push((s, e)),
            }
        }
        *r = out;
    }
    let count = match FileRef::new(data) {
        Ok(FileRef::Font(_)) => 1,
        Ok(FileRef::Collection(c)) => c.len().min(MAX_FACES),
        Err(_) => 0,
    };
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let mut budget = MAX_MAPPINGS;
    'faces: for i in 0..count {
        let Ok(f) = skrifa::FontRef::from_index(data, i) else { continue };
        for (cp, _) in f.charmap().mappings() {
            budget = match budget.checked_sub(1) {
                Some(b) => b,
                None => break 'faces,
            };
            match ranges.last_mut() {
                Some(last) if last.1.checked_add(1) == Some(cp) => last.1 = cp,
                Some(last) if last.0 <= cp && cp <= last.1 => {}
                _ => {
                    if ranges.len() >= MAX_RANGES {
                        compact(&mut ranges);
                        if ranges.len() >= MAX_RANGES {
                            break 'faces;
                        }
                    }
                    ranges.push((cp, cp));
                }
            }
        }
    }
    compact(&mut ranges);
    ranges.into_boxed_slice()
}

/// An installed font file's bytes, `None` when it can't be read or is larger than
/// [`SYS_FALLBACK_MAX_FILE`] (also when it grew past that since it was cataloged).
#[cfg(not(target_arch = "wasm32"))]
fn read_fallback_file(path: &std::path::Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > SYS_FALLBACK_MAX_FILE {
        return None;
    }
    let mut data = Vec::new();
    file.take(SYS_FALLBACK_MAX_FILE + 1).read_to_end(&mut data).ok()?;
    (data.len() as u64 <= SYS_FALLBACK_MAX_FILE).then_some(data)
}

/// Families preferred for right-to-left scripts (Persian, Arabic, Urdu, Hebrew) when the
/// document's font lacks them: open fonts with full Persian coverage first (the Persian-only
/// letters پ چ ژ گ, the Persian digits), then the platforms' own.
/// Right-to-left script fonts (Hebrew, Arabic, Persian, Syriac…) in fallback order.
/// Windows prioritizes David/Miriam (Office) and Segoe UI (standard).
/// macOS prioritizes Helvetica, Geeza Pro (system), and SF fonts.
/// Linux uses Noto fonts.
#[cfg(windows)]
pub const RTL_FALLBACKS: &[&str] = &[
    "Vazirmatn",
    "Noto Naskh Arabic",
    "Noto Sans Arabic",
    "Noto Sans Arabic UI",
    "Sahel",
    "Shabnam",
    "Samim",
    "Noto Sans Hebrew",
    "David",
    "Miriam",
    "Segoe UI",
    "Tahoma",
    "Arial",
    "DejaVu Sans",
];

#[cfg(target_os = "macos")]
pub const RTL_FALLBACKS: &[&str] = &[
    "Helvetica",
    "Vazirmatn",
    "Noto Naskh Arabic",
    "Noto Sans Arabic",
    "Noto Sans Hebrew",
    "Geeza Pro",
    "SF Arabic",
    "Arial",
    "Tahoma",
    "DejaVu Sans",
];

#[cfg(not(any(windows, target_os = "macos")))]
pub const RTL_FALLBACKS: &[&str] =
    &["Noto Naskh Arabic", "Noto Sans Arabic", "Noto Sans Hebrew", "Vazirmatn", "Segoe UI", "Arial", "Tahoma", "DejaVu Sans"];

/// Families tried (when installed) for characters the loaded fonts lack: CJK, symbols, emoji.
#[cfg(not(target_arch = "wasm32"))]
const SYSTEM_FALLBACKS: &[&str] = &[
    "Helvetica Neue",
    "Arial",
    "Segoe UI",
    "Noto Sans",
    "DejaVu Sans",
    "PingFang SC",
    "Hiragino Sans",
    "Hiragino Kaku Gothic ProN",
    "Apple SD Gothic Neo",
    "Heiti SC",
    "STHeiti",
    "Microsoft YaHei",
    "Yu Gothic",
    "Malgun Gothic",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Arial Unicode MS",
    "Apple Symbols",
    "Segoe UI Symbol",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
    "Noto Emoji",
    "Segoe UI Emoji",
    "Apple Color Emoji",
    "Noto Color Emoji",
];

/// Arabic-script marks and digits (bidi classes NSM / AN): they belong with the Arabic letters
/// around them when choosing a fallback font.
fn arabic_block(c: char) -> bool {
    matches!(u32::from(c), 0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF)
}

#[derive(Default)]
struct FallbackCache {
    faces: usize,
    map: HashMap<(char, u32), Option<Arc<FontFace>>>,
}

/// [`FontDb::fallback_for_run`] results: (sorted, deduplicated chars, excluded face, bold,
/// italic) → face, valid while the loaded face count is `faces`.
#[derive(Default)]
struct RunFallbackCache {
    faces: usize,
    /// Chars held in the keys (bounds memory: hostile runs can be long).
    chars: usize,
    #[allow(clippy::type_complexity)]
    map: HashMap<(Vec<char>, u32, bool, bool), Option<Arc<FontFace>>>,
}

/// Most cached run-level fallbacks, and most chars in their keys, before the cache is reset.
const RUN_FALLBACK_CACHE_MAX: usize = 4096;
const RUN_FALLBACK_CACHE_CHARS: usize = 1 << 20;

/// Loaded faces other than `exclude`, in fallback order: for right-to-left text the fonts made
/// for the script first (so a word isn't split across fonts, which breaks Arabic joining); for a
/// complex-script character `complex` (Thai, Myanmar…) faces with OpenType layout tables, the
/// script's own families first, as the per-character fallback ranks them; then the fallback
/// family, upright, nearest to regular weight.
fn fallback_order(faces: &[Arc<FontFace>], exclude: u32, rtl: bool, complex: Option<char>) -> Vec<&Arc<FontFace>> {
    let mut order: Vec<&Arc<FontFace>> = faces.iter().filter(|f| f.id != exclude).collect();
    let rank =
        |f: &FontFace| if rtl { RTL_FALLBACKS.iter().position(|n| f.family.eq_ignore_ascii_case(n)).unwrap_or(RTL_FALLBACKS.len()) } else { 0 };
    let shaping = |f: &FontFace| match complex {
        Some(c) => (!f.has_layout_tables(), crate::fallback_rank(c, &f.family)),
        None => (false, 0),
    };
    order.sort_by_key(|f| (rank(f), shaping(f), !f.family.eq_ignore_ascii_case(FALLBACK_FAMILY), f.italic, (f.weight - 400.0).abs() as i32));
    order
}

const FALLBACK_CACHE_MAX: usize = 20_000;

static NEXT_ID: AtomicU32 = AtomicU32::new(1);
const OUTLINE_CACHE_MAX: usize = 50_000;

fn name(font: &skrifa::FontRef<'_>, ids: &[StringId]) -> Option<String> {
    ids.iter().find_map(|id| font.localized_strings(*id).english_or_first().map(|s| s.to_string()).filter(|s| !s.is_empty()))
}

/// The family's names in other languages: localized typographic family names (name ID 16) stand
/// for `family`, localized legacy family names (name ID 1) for `id1` (or for `family` when the
/// font has no typographic names). Bounded: a damaged font can list any number of records.
fn local_names(f: &skrifa::FontRef<'_>, family: &str, id1: Option<&str>) -> Vec<LocalName> {
    const MAX_NAMES: usize = 64;
    const MAX_CHARS: usize = 128;
    let typographic = f.localized_strings(StringId::TYPOGRAPHIC_FAMILY_NAME).next().is_some();
    let legacy = if typographic { id1 } else { Some(family) };
    let mut out: Vec<LocalName> = Vec::new();
    for (id, english) in [(StringId::TYPOGRAPHIC_FAMILY_NAME, Some(family)), (StringId::FAMILY_NAME, legacy)] {
        let Some(english) = english else { continue };
        for s in f.localized_strings(id) {
            let Some(lang) = s.language().map(str::to_ascii_lowercase) else { continue };
            if lang == "en" || lang.starts_with("en-") {
                continue;
            }
            let name: String = s.chars().take(MAX_CHARS).collect::<String>().trim().to_string();
            if name.is_empty() || name.eq_ignore_ascii_case(english) || out.iter().any(|n| n.lang == lang && n.name == name) {
                continue;
            }
            if out.len() >= MAX_NAMES {
                return out;
            }
            out.push(LocalName { lang, name, english: english.to_string() });
        }
    }
    out
}

/// A face's family and style names (the default instance's style for a variable font), and its
/// legacy family name (name ID 1) when it has one.
pub(crate) fn face_names(f: &skrifa::FontRef<'_>) -> Option<(String, String, Option<String>)> {
    let family = name(f, &[StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME])?;
    let style = name(f, &[StringId::TYPOGRAPHIC_SUBFAMILY_NAME, StringId::SUBFAMILY_NAME]).unwrap_or_else(|| "Regular".into());
    Some((family, style, name(f, &[StringId::FAMILY_NAME])))
}

/// The family name Word uses for a face that has no legacy name (name ID 1) of its own, such as a
/// variable font's named instance: the family plus the style's words other than Regular, Bold and
/// Italic, which stay styles within it (OpenType `name` table, IDs 1/2 vs 16/17). "Roboto" +
/// "Light Italic" → "Roboto Light"; "Roboto" + "Bold Italic" → "Roboto".
pub(crate) fn legacy_family(family: &str, style: &str) -> String {
    const RIBBI: &[&str] = &["regular", "normal", "bold", "italic", "oblique"];
    let extra: Vec<&str> = base_style(style).split_whitespace().filter(|w| !RIBBI.iter().any(|r| w.eq_ignore_ascii_case(r))).collect();
    if extra.is_empty() { family.to_string() } else { format!("{family} {}", extra.join(" ")) }
}

/// The color bitmap tables the renderer draws glyphs from ([`FontFace::color_glyph`]).
#[cfg(not(target_arch = "wasm32"))]
const COLOR_BITMAP_TABLES: [&[u8; 4]; 2] = [b"sbix", b"CBDT"];

/// The family names (typographic and legacy) of the faces in the font file at `path`, read from
/// its name tables only (the rest of the file isn't loaded). Empty for a file that isn't a font.
#[cfg(not(target_arch = "wasm32"))]
pub fn font_file_families(path: &std::path::Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in file_face_names(path) {
        for name in [f.family, f.legacy] {
            if !name.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&name)) {
                out.push(name);
            }
        }
    }
    out
}

/// The names of every face in the font file at `path`, reading only its table directories and
/// `name` and `fvar` tables: a scan opens hundreds of font files, many of them megabytes long
/// (CJK collections run to 20 MB and more). A variable font yields one entry per named instance,
/// as it does loaded. Faces with none of the [`OUTLINE_TABLES`](crate::ui_fallback::OUTLINE_TABLES)
/// and [`COLOR_BITMAP_TABLES`] are left out.
#[cfg(not(target_arch = "wasm32"))]
fn file_face_names(path: &std::path::Path) -> Vec<Found> {
    use crate::ui_fallback::OUTLINE_TABLES;
    use std::io::{Read, Seek, SeekFrom};
    /// Caps on what a (possibly damaged) file can make the scan read.
    const MAX_FACES: u32 = 256;
    const MAX_TABLE: u32 = 1 << 20;
    let Ok(mut file) = std::fs::File::open(path) else { return vec![] };
    let mut read_at = |offset: u64, len: usize| -> Option<Vec<u8>> {
        let mut buf = vec![0; len];
        file.seek(SeekFrom::Start(offset)).ok()?;
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    };
    let be32 = |b: &[u8], at: usize| b.get(at..at + 4).and_then(|s| s.try_into().ok()).map(u32::from_be_bytes);
    let Some(head) = read_at(0, 12) else { return vec![] };
    // A collection lists where each face's table directory starts.
    let starts: Vec<u32> = if head.starts_with(b"ttcf") {
        let n = be32(&head, 8).unwrap_or(0).min(MAX_FACES) as usize;
        read_at(12, n * 4).map(|b| b.as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect()).unwrap_or_default()
    } else {
        vec![0]
    };
    starts
        .into_iter()
        .filter_map(|start| {
            let dir = read_at(start.into(), 12)?;
            let tables = u16::from_be_bytes(dir.get(4..6)?.try_into().ok()?) as usize;
            let records = read_at(u64::from(start) + 12, tables * 16)?;
            if !records.as_chunks::<16>().0.iter().any(|r| OUTLINE_TABLES.iter().chain(&COLOR_BITMAP_TABLES).any(|t| r.starts_with(*t))) {
                return None;
            }
            let mut table = |tag: &[u8; 4]| -> Option<Vec<u8>> {
                let rec = records.as_chunks::<16>().0.iter().find(|r| r.starts_with(tag))?;
                let (offset, len) = (be32(rec, 8)?, be32(rec, 12)?);
                if len > MAX_TABLE {
                    return None;
                }
                read_at(offset.into(), len as usize)
            };
            let name = table(b"name")?;
            // Variable fonts: the named instances; OS/2: the code pages (East Asian fonts).
            let mut tables: Vec<([u8; 4], Vec<u8>)> = table(b"OS/2").map(|t| (*b"OS/2", t)).into_iter().collect();
            tables.extend(table(b"fvar").map(|t| (*b"fvar", t)));
            tables.push((*b"name", name));
            // A font holding just those tables, to read them as the font itself would.
            let font = mini_font(&tables);
            Some(font_faces(&skrifa::FontRef::new(&font).ok()?, 0))
        })
        .flatten()
        .collect()
}

/// A font file holding just `tables` (sorted by tag), each padded to four bytes.
#[cfg(not(target_arch = "wasm32"))]
fn mini_font(tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut font = Vec::new();
    font.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
    // numTables, then searchRange, entrySelector and rangeShift (which readers don't need).
    font.extend_from_slice(&u16::try_from(tables.len()).unwrap_or(0).to_be_bytes());
    font.extend_from_slice(&[0; 6]);
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in tables {
        font.extend_from_slice(tag);
        font.extend_from_slice(&[0; 4]);
        for v in [offset, data.len()] {
            font.extend_from_slice(&u32::try_from(v).unwrap_or(0).to_be_bytes());
        }
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in tables {
        font.extend_from_slice(data);
        font.resize(font.len().next_multiple_of(4), 0);
    }
    font
}

/// The platform's font folders (the system's and the user's), scanned by [`FontDb::global`].
/// Empty on wasm.
pub fn system_font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if cfg!(target_arch = "wasm32") {
        return dirs;
    }
    // Extra folders first (a path list, like PATH): fonts that aren't installed, such as a
    // Windows partition's fonts when comparing pages with Word on another OS.
    if let Some(extra) = std::env::var_os("CRAFT_EXTRA_FONT_DIRS") {
        dirs.extend(std::env::split_paths(&extra).filter(|p| !p.as_os_str().is_empty()));
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if cfg!(target_os = "macos") {
        dirs.extend(["/System/Library/Fonts", "/Library/Fonts"].map(Into::into));
        if let Some(h) = &home {
            dirs.push(h.join("Library/Fonts"));
        }
        // Fonts macOS ships as assets (PingFang, Yu Mincho, …): com_apple_MobileAsset_Font<N>
        // (`Assets` before macOS 11, `AssetsV2` since).
        for parent in ["/System/Library/AssetsV2", "/System/Library/AssetsV2/PreinstalledAssetsV2/InstallWithOs", "/System/Library/Assets"] {
            let Ok(entries) = std::fs::read_dir(parent) else { continue };
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("com_apple_MobileAsset_Font") {
                    dirs.push(e.path());
                }
            }
        }
    } else if cfg!(windows) {
        let root = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
        dirs.push(root.join("Fonts"));
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            let local = std::path::PathBuf::from(l);
            dirs.push(local.join("Microsoft\\Windows\\Fonts"));
            // Microsoft 365's cloud fonts (Aptos, Grandview, …), downloaded on demand into
            // one folder per family; Word documents since 2024 default to Aptos.
            dirs.push(local.join("Microsoft\\FontCache\\4\\CloudFonts"));
        }
    } else {
        dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(Into::into));
        if let Some(h) = &home {
            dirs.push(h.join(".fonts"));
            dirs.push(h.join(".local/share/fonts"));
        }
        // Flatpak (#1937): the folders above are the runtime's, and the host's system, local and
        // user fonts are mounted here instead (no filesystem permission needed). Flatpak after
        // 1.19.2 also mounts ~/.fonts, at user-fonts-legacy when the XDG user font folder exists too.
        dirs.extend(["/run/host/fonts", "/run/host/local-fonts", "/run/host/user-fonts", "/run/host/user-fonts-legacy"].map(Into::into));
    }
    dirs
}

/// One face found in a font file.
struct Found {
    /// Face index within the file.
    index: u32,
    family: String,
    style: String,
    /// The legacy family name ([`FontFace::legacy_family`]).
    legacy: String,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    full_name: Option<String>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    postscript: Option<String>,
    /// The family's names in other languages ([`FontFace::local_names`]).
    local: Vec<LocalName>,
    /// A Chinese, Japanese or Korean font ([`FontFace::is_east_asian`]); the scan's catalog keeps it.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    east_asian: bool,
    /// Variable fonts: the named instance's axis settings.
    coords: Vec<([u8; 4], f32)>,
}

/// The faces of `f` (face `index` of its file): one per named instance of a variable font
/// (InDesign lists them as styles), otherwise the font itself.
fn font_faces(f: &skrifa::FontRef<'_>, index: u32) -> Vec<Found> {
    let Some((family, style, id1)) = face_names(f) else { return vec![] };
    let local = local_names(f, &family, id1.as_deref());
    let east_asian = east_asian_font(f);
    let axes = f.axes();
    let mut named: Vec<Found> = Vec::new();
    // At most one more than a file may add, so a caller can tell there are more (#3716).
    for ni in f.named_instances().iter().take(MAX_FILE_FACES + 1) {
        let Some(style) = name(f, &[ni.subfamily_name_id()]) else { continue };
        if named.iter().any(|n| n.style.eq_ignore_ascii_case(&style)) {
            continue;
        }
        let coords: Vec<([u8; 4], f32)> = axes.iter().zip(ni.user_coords()).map(|(a, v)| (a.tag().to_be_bytes(), v)).collect();
        // Name ID 1 names the default instance only.
        let legacy = legacy_family(&family, &style);
        // Name ID 1 names the default instance only: keep the names of the whole family.
        let local = local.iter().filter(|n| n.english.eq_ignore_ascii_case(&family)).cloned().collect();
        named.push(Found { index, family: family.clone(), style, legacy, full_name: None, postscript: None, local, east_asian, coords });
    }
    if !named.is_empty() {
        return named;
    }
    let legacy = id1.filter(|n| !n.eq_ignore_ascii_case(&family)).unwrap_or_else(|| legacy_family(&family, &style));
    vec![Found {
        index,
        family,
        style,
        legacy,
        full_name: name(f, &[StringId::FULL_NAME]),
        postscript: name(f, &[StringId::POSTSCRIPT_NAME]),
        local,
        east_asian,
        coords: Vec::new(),
    }]
}

/// Most faces one font file adds (collection members, variable fonts' named instances).
const MAX_FILE_FACES: usize = 256;

/// Parse the faces in `data` (a font file or collection), at most `limit` of them; a variable
/// font yields one face per named instance. A face with the family and style of an earlier one
/// is left out (a collection can list one font many times, #3716).
fn enumerate_faces(data: &[u8], limit: usize) -> Vec<Found> {
    let count = match FileRef::new(data) {
        Ok(FileRef::Font(_)) => 1,
        Ok(FileRef::Collection(c)) => c.len().min(u32::try_from(limit).unwrap_or(u32::MAX)),
        Err(_) => 0,
    };
    let mut seen = std::collections::HashSet::new();
    (0..count)
        .filter_map(|i| skrifa::FontRef::from_index(data, i).ok().map(|f| font_faces(&f, i)))
        .flatten()
        .filter(|f| seen.insert((f.family.to_ascii_lowercase(), f.style.to_ascii_lowercase())))
        .take(limit)
        .collect()
}

/// The compiled-in Source Sans 3 Regular, for a database that somehow has no faces at all.
pub(crate) fn last_resort_face() -> Arc<FontFace> {
    static FACE: std::sync::OnceLock<Arc<FontFace>> = std::sync::OnceLock::new();
    FACE.get_or_init(|| {
        // The font is compiled in (`include_bytes!`), so parsing it can't depend on input; the
        // `last_resort_face_parses` test proves it on every run.
        #[allow(clippy::expect_used)]
        let face = make_face(FontBytes::Static(BUNDLED[0]), 0, FALLBACK_FAMILY.into(), "Regular".into(), FALLBACK_FAMILY.into(), Vec::new())
            .expect("the compiled-in Source Sans 3 Regular parses");
        Arc::new(face)
    })
    .clone()
}

fn make_face(bytes: FontBytes, index: u32, family: String, style: String, legacy_family: String, coords: Vec<([u8; 4], f32)>) -> Option<FontFace> {
    let data: &[u8] = match &bytes {
        FontBytes::Static(b) => b,
        FontBytes::Owned(v) => v.as_slice(),
    };
    let f = skrifa::FontRef::from_index(data, index).ok()?;
    let settings: Vec<(skrifa::Tag, f32)> = coords.iter().map(|(t, v)| (skrifa::Tag::new(t), *v)).collect();
    let location = if coords.is_empty() { Location::default() } else { f.axes().location(settings.iter().copied()) };
    let m = f.metrics(Size::unscaled(), &location);
    let a = f.attributes();
    let hb = harfrust::FontRef::from_index(data, index).ok()?;
    let shaper = harfrust::ShaperData::new(&hb);
    let instance = (!coords.is_empty())
        .then(|| harfrust::ShaperInstance::from_variations(&hb, settings.iter().map(|(t, v)| harfrust::Variation { tag: *t, value: *v })));
    let axis = |tag: &[u8; 4]| coords.iter().find(|(t, _)| t == tag).map(|(_, v)| *v);
    // Old releases of heavy faces (Arial Black in the core fonts) claim weight 400 where current
    // ones say 900, which Word doesn't embolden: a family named for its weight is that heavy.
    let weight = axis(b"wght").unwrap_or(if coords.is_empty() { static_weight(a.weight.value(), &legacy_family) } else { style_weight(&style) });
    let italic = axis(b"ital")
        .map(|v| v >= 0.5)
        .or(axis(b"slnt").map(|v| v.abs() > 0.1))
        .unwrap_or(!matches!(a.style, skrifa::attribute::Style::Normal) || style_italic(&style));
    Some(FontFace {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        family,
        style,
        legacy_family,
        local_names: Vec::new(),
        full_name: coords.is_empty().then(|| name(&f, &[StringId::FULL_NAME])).flatten(),
        postscript: coords.is_empty().then(|| name(&f, &[StringId::POSTSCRIPT_NAME])).flatten(),
        weight,
        italic,
        upem: m.units_per_em.max(1) as f64,
        ascent: m.ascent as f64,
        descent: -(m.descent as f64),
        cap_height: m.cap_height.map(|v| v as f64).filter(|v| *v > 0.0).unwrap_or(m.ascent as f64 * 0.72),
        x_height: m.x_height.map(|v| v as f64).filter(|v| *v > 0.0).unwrap_or(m.ascent as f64 * 0.5),
        shaper,
        coords,
        location,
        instance,
        bytes,
        index,
        bmp: std::sync::OnceLock::new(),
        outline_hinter: std::sync::OnceLock::new(),
        hint_draws: Default::default(),
        hint_slow: Default::default(),
        hinting_off: Default::default(),
        #[cfg(not(target_arch = "wasm32"))]
        hint_strike: HINT_STRIKE,
    })
}

/// The named style part of a style with axis settings (`Bold {wght:650}` → `Bold`).
pub fn base_style(style: &str) -> &str {
    style.split('{').next().unwrap_or(style).trim()
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Weight implied by a style name.
fn style_weight(style: &str) -> f32 {
    let s = norm(style);
    const TABLE: &[(&str, f32)] = &[
        ("extralight", 200.0),
        ("ultralight", 200.0),
        ("semibold", 600.0),
        ("demibold", 600.0),
        ("extrabold", 800.0),
        ("ultrabold", 800.0),
        ("hairline", 100.0),
        ("thin", 100.0),
        ("light", 300.0),
        ("medium", 500.0),
        ("bold", 700.0),
        ("black", 900.0),
        ("heavy", 900.0),
    ];
    TABLE.iter().find(|(k, _)| s.contains(k)).map(|(_, w)| *w).unwrap_or(400.0)
}

/// A static face's weight: the file's, or the weight the last word of its family name stands
/// for (`Arial Black`, `Franklin Gothic Heavy`) when the file says less than semibold.
fn static_weight(file: f32, family: &str) -> f32 {
    if file >= 600.0 {
        return file;
    }
    match family.split_whitespace().next_back().unwrap_or("").to_ascii_lowercase().as_str() {
        "black" | "heavy" => 900.0,
        "extrabold" | "ultrabold" => 800.0,
        _ => file,
    }
}

fn style_italic(style: &str) -> bool {
    let s = norm(style);
    s.contains("italic") || s.contains("oblique") || s == "it"
}

=======
>>>>>>> bcf2d018432b2b64af710c0c37fc15c621d664d9
impl FontDb {
    /// A database holding the bundled fonts, then the craft-fonts Japanese faces (Mincho first,
    /// so they are the fallback for Japanese text after the requested and bundled fonts), then the
    /// other craft-fonts faces: Simplified Chinese, for the ideographs the Japanese fonts lack, and
    /// Arabic, so Arabic and Persian text draws without system fonts (#1652) (all empty without
    /// `CRAFT_FONTS_DIR`), whose system font scan reads `font_dirs`.
    pub fn with_font_dirs(font_dirs: Vec<std::path::PathBuf>) -> Self {
        let others = crate::CRAFT_FONTS.iter().filter(|f| !f.scripts.contains(&"Jpan"));
        let craft = crate::japanese_document_fonts().into_iter().chain(others).map(|f| f.bytes);
        Self::with_static_fonts(font_dirs, BUNDLED.iter().copied().chain(craft))
    }

    fn with_static_fonts(font_dirs: Vec<std::path::PathBuf>, fonts: impl Iterator<Item = &'static [u8]>) -> Self {
        let mut faces = Vec::new();
        for data in fonts {
            for f in enumerate_faces(data, MAX_FILE_FACES) {
                if let Some(mut face) = make_face(FontBytes::Static(data), f.index, f.family, f.style, f.legacy, f.coords) {
                    face.local_names = f.local;
                    faces.push(Arc::new(face));
                }
            }
        }
        Self {
            faces: RwLock::new(faces),
            outlines: Mutex::new(HashMap::new()),
            hinted: Mutex::new(HashMap::new()),
            hinters: Mutex::new(HashMap::new()),
            fallbacks: RwLock::new(FallbackCache::default()),
            run_fallbacks: RwLock::new(RunFallbackCache::default()),
            #[cfg(not(target_arch = "wasm32"))]
            catalog: RwLock::new(Vec::new()),
            user: RwLock::new(user::UserFonts::default()),
            generation: AtomicU64::new(0),
            font_dirs,
            #[cfg(not(target_arch = "wasm32"))]
            cataloged: std::sync::OnceLock::new(),
            #[cfg(not(target_arch = "wasm32"))]
            loading: Mutex::new(()),
            #[cfg(not(target_arch = "wasm32"))]
            sys: Mutex::new(SysFallback { enabled: true, ..Default::default() }),
            #[cfg(not(target_arch = "wasm32"))]
            searching: Mutex::new(()),
            #[cfg(not(target_arch = "wasm32"))]
            coverage: Mutex::new(CoverageIndex::default()),
            #[cfg(not(target_arch = "wasm32"))]
            catalog_gen: AtomicU32::new(0),
        }
    }

    /// Enable or disable the lazy system-font fallback for characters the loaded fonts lack
    /// (native only; on by default).
    pub fn set_system_fallback(&self, on: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.sys.lock().unwrap_or_else(std::sync::PoisonError::into_inner).enabled = on;
        }
        // Misses remembered while it was off may be found now.
        self.fallbacks.write().unwrap_or_else(std::sync::PoisonError::into_inner).map.clear();
        *self.run_fallbacks.write().unwrap_or_else(std::sync::PoisonError::into_inner) = RunFallbackCache::default();
        #[cfg(target_arch = "wasm32")]
        let _ = on;
    }

    /// Process-wide database: the bundled fonts, then the installed system fonts, cataloged the
    /// first time a lookup by family name needs them (or ahead of time by
    /// [`FontDb::scan_in_background`]).
    pub fn global() -> &'static FontDb {
        GLOBAL.get_or_init(|| FontDb::with_font_dirs(system_font_dirs()))
    }

    /// Initialize a test process with only the repository's pinned bundled fonts: no optional
    /// craft-fonts, user fonts or system font directories. Call before any global font lookup.
    /// Returns false if a normal global database was already initialized; never replaces it.
    /// Repeated/concurrent calls are safe. Merely enabling `test-support` changes nothing.
    #[cfg(feature = "test-support")]
    pub fn init_test_fonts() -> bool {
        static INIT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *INIT.get_or_init(|| {
            let mut installed = false;
            GLOBAL.get_or_init(|| {
                installed = true;
                let db = Self::with_static_fonts(Vec::new(), BUNDLED.iter().copied());
                db.set_system_fallback(false);
                db
            });
            installed
        })
    }

    fn read_faces(&self) -> std::sync::RwLockReadGuard<'_, Vec<Arc<FontFace>>> {
        self.faces.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The system font catalog, scanned first if it hasn't been yet.
    #[cfg(not(target_arch = "wasm32"))]
    fn read_catalog(&self) -> std::sync::RwLockReadGuard<'_, Vec<CatalogEntry>> {
        self.ensure_catalog();
        self.catalog.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
pub(crate) mod tests;
