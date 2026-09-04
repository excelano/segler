//! Opening a DocLang document from disk, whether it is bare markup or an
//! archive.
//!
//! A DocLang archive (`.dclx`) is a ZIP using the Open Packaging Conventions:
//! `[Content_Types].xml` and `_rels/.rels` at the root, the document at
//! `document.xml`, optional page images under `pages/` named `N.png` (or
//! `jpg`, `jpeg`, `webp`) with `N` counted from one, and optional payload
//! under `assets/`. Bare markup (`.dclg`) is the document alone.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use zip::ZipArchive;

/// The part an archive's `_rels/.rels` must point at.
pub const DOCUMENT_PART: &str = "document.xml";

/// The relationship type that names the main document part.
pub const DOCUMENT_RELATIONSHIP: &str = "http://doclang.ai/ns/package/2026/relationships/document";

/// The content type `[Content_Types].xml` must declare for the document part.
pub const DOCUMENT_CONTENT_TYPE: &str = "application/vnd.doclang.document+xml";

/// Image extensions the spec allows under `pages/`.
const PAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

/// What a file on disk turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Bare DocLang markup.
    Markup,
    /// A DocLang archive.
    Archive,
}

/// A page image found under `pages/` in an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagePart {
    /// The one-based page number, from the file name.
    pub number: u32,
    /// The part name inside the archive, e.g. `pages/3.png`.
    pub name: String,
}

/// A document loaded from disk: the markup and, for an archive, what came
/// with it. Page images are named rather than decoded, because decoding is
/// the renderer's business and most of them will never be looked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub kind: Kind,
    /// The DocLang markup, as the bytes on disk read as UTF-8.
    pub markup: String,
    /// Page images, in page order. Gaps are allowed by the spec.
    pub pages: Vec<PagePart>,
    /// Part names under `assets/`.
    pub assets: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("archive has no {DOCUMENT_PART}")]
    NoDocument,
    #[error("markup is not UTF-8")]
    NotUtf8,
}

/// Load a document from `path`, deciding by content rather than extension:
/// a file that begins with the ZIP local-header signature is an archive,
/// anything else is bare markup.
pub fn load(path: &Path) -> Result<Loaded, Error> {
    let bytes = fs::read(path)?;
    load_bytes(&bytes)
}

/// [`load`] over bytes already in memory.
pub fn load_bytes(bytes: &[u8]) -> Result<Loaded, Error> {
    if bytes.starts_with(b"PK\x03\x04") {
        load_archive(bytes)
    } else {
        Ok(Loaded {
            kind: Kind::Markup,
            markup: String::from_utf8(bytes.to_vec()).map_err(|_| Error::NotUtf8)?,
            pages: Vec::new(),
            assets: Vec::new(),
        })
    }
}

fn load_archive(bytes: &[u8]) -> Result<Loaded, Error> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();

    let markup = match zip.by_name(DOCUMENT_PART) {
        Ok(mut part) => {
            let mut raw = Vec::new();
            part.read_to_end(&mut raw)?;
            String::from_utf8(raw).map_err(|_| Error::NotUtf8)?
        }
        Err(zip::result::ZipError::FileNotFound) => return Err(Error::NoDocument),
        Err(e) => return Err(e.into()),
    };

    let mut pages: Vec<PagePart> = names.iter().filter_map(|n| page_part(n)).collect();
    pages.sort_by_key(|p| p.number);

    let mut assets: Vec<String> = names
        .iter()
        .filter(|n| n.starts_with("assets/") && !n.ends_with('/'))
        .cloned()
        .collect();
    assets.sort();

    Ok(Loaded {
        kind: Kind::Archive,
        markup,
        pages,
        assets,
    })
}

/// `pages/12.png` → page 12. Anything else under `pages/`, or anywhere else,
/// is not a page part.
fn page_part(name: &str) -> Option<PagePart> {
    let file = name.strip_prefix("pages/")?;
    let (stem, ext) = file.rsplit_once('.')?;
    if !PAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
        return None;
    }
    let number: u32 = stem.parse().ok()?;
    (number >= 1).then(|| PagePart {
        number,
        name: name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    const MARKUP: &str = "<doclang><text>Hello</text></doclang>";

    fn archive(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            for (name, data) in parts {
                w.start_file(*name, SimpleFileOptions::default()).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn bare_markup_loads_as_markup() {
        let loaded = load_bytes(MARKUP.as_bytes()).unwrap();
        assert_eq!(loaded.kind, Kind::Markup);
        assert_eq!(loaded.markup, MARKUP);
        assert!(loaded.pages.is_empty());
    }

    #[test]
    fn archive_yields_document_pages_and_assets() {
        let bytes = archive(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("_rels/.rels", b"<Relationships/>"),
            (DOCUMENT_PART, MARKUP.as_bytes()),
            ("pages/2.png", b"png"),
            ("pages/1.png", b"png"),
            ("pages/10.webp", b"webp"),
            ("pages/notes.txt", b"not a page"),
            ("assets/chart.svg", b"<svg/>"),
        ]);
        let loaded = load_bytes(&bytes).unwrap();
        assert_eq!(loaded.kind, Kind::Archive);
        assert_eq!(loaded.markup, MARKUP);
        let numbers: Vec<u32> = loaded.pages.iter().map(|p| p.number).collect();
        assert_eq!(numbers, [1, 2, 10]);
        assert_eq!(loaded.assets, ["assets/chart.svg"]);
    }

    #[test]
    fn archive_without_document_is_an_error() {
        let bytes = archive(&[("pages/1.png", b"png")]);
        assert!(matches!(load_bytes(&bytes), Err(Error::NoDocument)));
    }

    #[test]
    fn page_zero_is_not_a_page() {
        assert!(page_part("pages/0.png").is_none());
        assert!(page_part("pages/3.PNG").is_some());
        assert!(page_part("assets/3.png").is_none());
    }
}
