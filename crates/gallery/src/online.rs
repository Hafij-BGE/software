//! Free pictures from the web, through Openverse (openverse.org): Creative Commons and public
//! domain images from Flickr, Wikimedia Commons, museums and more. Every result carries its
//! creator and licence so the apps can credit it.

use std::io::Cursor;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use image::{DynamicImage, GenericImageView, ImageFormat, imageops::FilterType};
use serde_json::Value;

const API: &str = "https://api.openverse.org/v1/images/";
/// Pictures larger than this are refused (bytes).
#[cfg(not(target_arch = "wasm32"))]
const MAX_DOWNLOAD: u64 = 25 * 1024 * 1024;
/// Pictures wider or taller than this are scaled down before they go into a document (pixels).
pub const MAX_SIDE: u32 = 2400;

/// Which licences a search accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Licence {
    /// Licences that allow commercial use (CC0, public domain, CC BY, CC BY-SA).
    #[default]
    Commercial,
    /// Public domain and CC0 only: no credit legally required.
    PublicDomain,
    /// Any Creative Commons licence.
    Any,
}

impl Licence {
    pub const ALL: [Licence; 3] = [Licence::Commercial, Licence::PublicDomain, Licence::Any];

    pub fn label(self) -> &'static str {
        match self {
            Licence::Commercial => "Free for commercial use",
            Licence::PublicDomain => "Public domain only",
            Licence::Any => "All Creative Commons",
        }
    }
}

/// What sort of picture a search wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Any,
    Photos,
    Illustrations,
    Artwork,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::Any, Sort::Photos, Sort::Illustrations, Sort::Artwork];

    pub fn label(self) -> &'static str {
        match self {
            Sort::Any => "Any kind",
            Sort::Photos => "Photos",
            Sort::Illustrations => "Illustrations",
            Sort::Artwork => "Artwork",
        }
    }

    fn param(self) -> Option<&'static str> {
        match self {
            Sort::Any => None,
            Sort::Photos => Some("photograph"),
            Sort::Illustrations => Some("illustration"),
            Sort::Artwork => Some("digitized_artwork"),
        }
    }
}

/// One picture found online.
#[derive(Clone, Debug, Default)]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub creator: String,
    /// `by`, `by-sa`, `cc0`, `pdm`…
    pub licence: String,
    pub licence_version: String,
    pub licence_url: String,
    /// The picture's page at its source.
    pub page: String,
    pub source: String,
    pub url: String,
    pub thumbnail: String,
    pub width: u32,
    pub height: u32,
}

impl Hit {
    /// "“Title” by Creator, CC BY 2.0, via Wikimedia Commons" — for alt text or a caption.
    pub fn credit(&self) -> String {
        let title = if self.title.trim().is_empty() { "Picture".to_string() } else { format!("“{}”", self.title.trim()) };
        let by = if self.creator.trim().is_empty() { String::new() } else { format!(" by {}", self.creator.trim()) };
        let licence = match self.licence.as_str() {
            "cc0" => "CC0".to_string(),
            "pdm" => "public domain".to_string(),
            l => format!("CC {} {}", l.to_uppercase(), self.licence_version).trim().to_string(),
        };
        let source = if self.source.is_empty() { String::new() } else { format!(", via {}", pretty_source(&self.source)) };
        format!("{title}{by}, {licence}{source}")
    }

    /// Whether the licence asks for a credit (everything but CC0 and public domain).
    pub fn needs_credit(&self) -> bool {
        !matches!(self.licence.as_str(), "cc0" | "pdm")
    }
}

fn pretty_source(s: &str) -> String {
    match s {
        "wikimedia" => "Wikimedia Commons".into(),
        "flickr" => "Flickr".into(),
        "stocksnap" => "StockSnap".into(),
        "rawpixel" => "rawpixel".into(),
        other => {
            let mut c = other.replace('_', " ");
            if let Some(first) = c.get(..1) {
                c = first.to_uppercase() + &c[1..];
            }
            c
        }
    }
}

/// One page of results and how many pages there are.
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub page: u32,
    pub pages: u32,
    pub total: u64,
}

#[cfg(not(target_arch = "wasm32"))]
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).user_agent("CraftSuite (desktop office suite)").build().into()
}

/// Searches Openverse, 20 pictures a page (the most it gives without an API key). `page` counts from 1.
pub fn search(query: &str, page: u32, licence: Licence, sort: Sort) -> Result<Page, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Page::default());
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (page, licence, sort.param());
        Err(DESKTOP_ONLY.into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let page_s = page.max(1).to_string();
        let mut req = agent().get(API).query("q", query).query("page", &page_s).query("page_size", "20").query("mature", "false");
        req = match licence {
            Licence::Commercial => req.query("license_type", "commercial"),
            Licence::PublicDomain => req.query("license", "cc0,pdm"),
            Licence::Any => req,
        };
        if let Some(c) = sort.param() {
            req = req.query("category", c);
        }
        let mut resp = req.call().map_err(|e| format!("Couldn't reach Openverse: {e}"))?;
        let body = resp.body_mut().read_to_string().map_err(|e| format!("Openverse answered oddly: {e}"))?;
        parse(&body, page.max(1))
    }
}

#[cfg(target_arch = "wasm32")]
const DESKTOP_ONLY: &str = "Online pictures need the desktop app.";

fn parse(body: &str, page: u32) -> Result<Page, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("Openverse answered oddly: {e}"))?;
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let n = |o: &Value, k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0);
    let hits = v
        .get("results")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(|r| {
                    let url = s(r, "url");
                    if url.is_empty() {
                        return None;
                    }
                    let id = s(r, "id");
                    let thumbnail = match s(r, "thumbnail") {
                        t if !t.is_empty() => t,
                        _ => format!("{API}{id}/thumb/"),
                    };
                    Some(Hit {
                        title: s(r, "title"),
                        creator: s(r, "creator"),
                        licence: s(r, "license"),
                        licence_version: s(r, "license_version"),
                        licence_url: s(r, "license_url"),
                        page: s(r, "foreign_landing_url"),
                        source: s(r, "source").chars().take(40).collect(),
                        width: n(r, "width") as u32,
                        height: n(r, "height") as u32,
                        id,
                        url,
                        thumbnail,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Page { hits, page, pages: n(&v, "page_count") as u32, total: n(&v, "result_count") })
}

/// Downloads `url` (up to 25 MB).
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Not a web address".into());
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err(DESKTOP_ONLY.into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut resp = agent().get(url).call().map_err(|e| format!("Download failed: {e}"))?;
        resp.body_mut().with_config().limit(MAX_DOWNLOAD).read_to_vec().map_err(|e| format!("Download failed: {e}"))
    }
}

/// A downloaded picture made ready for a document: JPEG or PNG, at most [`MAX_SIDE`] pixels a side.
/// Returns the bytes and the file extension.
pub fn prepare(bytes: &[u8]) -> Result<(Vec<u8>, &'static str), String> {
    let format = image::guess_format(bytes).map_err(|_| "That file isn't a picture the apps can read".to_string())?;
    let img = image::load_from_memory_with_format(bytes, format).map_err(|e| format!("That picture can't be read: {e}"))?;
    let (w, h) = img.dimensions();
    let small = w <= MAX_SIDE && h <= MAX_SIDE;
    match format {
        ImageFormat::Jpeg if small => return Ok((bytes.to_vec(), "jpg")),
        ImageFormat::Png if small => return Ok((bytes.to_vec(), "png")),
        _ => {}
    }
    let img = if small { img } else { img.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle) };
    encode(&img)
}

fn encode(img: &DynamicImage) -> Result<(Vec<u8>, &'static str), String> {
    let mut out = Cursor::new(Vec::new());
    if img.color().has_alpha() {
        img.write_to(&mut out, ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok((out.into_inner(), "png"))
    } else {
        DynamicImage::ImageRgb8(img.to_rgb8()).write_to(&mut out, ImageFormat::Jpeg).map_err(|e| e.to_string())?;
        Ok((out.into_inner(), "jpg"))
    }
}

/// A thumbnail decoded to straight RGBA, at most `side` pixels a side.
pub fn thumbnail(bytes: &[u8], side: u32) -> Option<([usize; 2], Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > side || img.height() > side { img.thumbnail(side, side) } else { img };
    let rgba = img.to_rgba8();
    Some(([rgba.width() as usize, rgba.height() as usize], rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_openverse_page() {
        let body = r#"{"result_count":2,"page_count":1,"results":[
            {"id":"a1","title":"Cell","creator":"Ann","license":"by","license_version":"2.0","url":"https://x/a.jpg","source":"wikimedia","width":800,"height":600},
            {"id":"b2","title":"","creator":"","license":"cc0","url":"https://x/b.jpg","thumbnail":"https://x/b_t.jpg"},
            {"id":"c3","title":"no url"}]}"#;
        let p = parse(body, 1).unwrap_or_default();
        assert_eq!(p.hits.len(), 2);
        assert_eq!(p.total, 2);
        assert_eq!(p.hits[0].thumbnail, "https://api.openverse.org/v1/images/a1/thumb/");
        assert_eq!(p.hits[0].credit(), "“Cell” by Ann, CC BY 2.0, via Wikimedia Commons");
        assert!(p.hits[0].needs_credit());
        assert_eq!(p.hits[1].credit(), "Picture, CC0");
        assert!(!p.hits[1].needs_credit());
    }

    #[test]
    fn big_pictures_are_scaled_down_and_small_ones_kept() {
        let mut png = Cursor::new(Vec::new());
        DynamicImage::new_rgb8(3000, 100).write_to(&mut png, ImageFormat::Png).ok();
        let (out, ext) = prepare(png.get_ref()).unwrap_or_default();
        assert_eq!(ext, "jpg");
        let back = image::load_from_memory(&out).map(|i| i.dimensions()).unwrap_or_default();
        assert_eq!(back.0, MAX_SIDE);
        let mut small = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(10, 10).write_to(&mut small, ImageFormat::Png).ok();
        assert_eq!(prepare(small.get_ref()).map(|(b, e)| (b.len(), e)), Ok((small.get_ref().len(), "png")));
        assert!(prepare(b"not a picture").is_err());
    }

    #[test]
    fn empty_queries_ask_nothing() {
        assert!(search("  ", 1, Licence::Any, Sort::Any).is_ok_and(|p| p.hits.is_empty()));
        assert!(fetch("file:///etc/passwd").is_err());
    }
}
