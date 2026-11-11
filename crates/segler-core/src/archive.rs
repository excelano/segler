//! Opening a DocLang document from disk, whether it is bare markup or an
//! archive.
//!
//! A DocLang archive (`.dclx`) is a ZIP using the Open Packaging Conventions:
//! `[Content_Types].xml` and `_rels/.rels` at the root, the document at
//! `document.xml`, optional page images under `pages/` named `N.png` (or
//! `jpg`, `jpeg`, `webp`) with `N` counted from one, optional payload
//! under `assets/`, and optionally the original document the markup was made
//! from at `source`. Bare markup (`.dclg`) is the document alone.
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

/// The part holding the original document, which has no extension, so only
/// an `<Override>` in [`CONTENT_TYPES_PART`] can give its type.
pub const SOURCE_PART: &str = "source";

pub const CONTENT_TYPES_PART: &str = "[Content_Types].xml";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modality {
    Audio,
    Video,
}

impl Modality {
    pub fn folder(self) -> &'static str {
        match self {
            Modality::Audio => "audio",
            Modality::Video => "video",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording {
    pub modality: Modality,
    pub track: u32,
    pub name: String,
}

/// The original document an archive carries at [`SOURCE_PART`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The type its `<Override>` declares, or `None` when there is none.
    pub content_type: Option<String>,
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
    pub recordings: Vec<Recording>,
    pub source: Option<Source>,
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
            recordings: Vec::new(),
            source: None,
        })
    }
}

fn load_archive(bytes: &[u8]) -> Result<Loaded, Error> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let names: Vec<String> = zip
        .file_names()
        .map(|n| n.map(|n| n.into_owned()))
        .collect::<Result<_, _>>()?;

    let markup = String::from_utf8(part_bytes(&mut zip, DOCUMENT_PART)?.ok_or(Error::NoDocument)?)
        .map_err(|_| Error::NotUtf8)?;

    let mut pages: Vec<PagePart> = names.iter().filter_map(|n| page_part(n)).collect();
    pages.sort_by_key(|p| p.number);

    let mut assets: Vec<String> = names
        .iter()
        .filter(|n| n.starts_with("assets/") && !n.ends_with('/'))
        .cloned()
        .collect();
    assets.sort();

    let mut recordings: Vec<Recording> = names.iter().filter_map(|n| recording(n)).collect();
    recordings.sort_by(|a, b| (a.track, &a.name).cmp(&(b.track, &b.name)));

    let source = if names.iter().any(|n| n == SOURCE_PART) {
        let types = part_bytes(&mut zip, CONTENT_TYPES_PART)?.unwrap_or_default();
        Some(Source {
            content_type: source_content_type(&String::from_utf8_lossy(&types)),
        })
    } else {
        None
    };

    Ok(Loaded {
        kind: Kind::Archive,
        markup,
        pages,
        assets,
        recordings,
        source,
    })
}

/// One part of an archive, by name, or `None` if the archive has no such
/// part.
pub fn read_part(bytes: &[u8], name: &str) -> Result<Option<Vec<u8>>, Error> {
    part_bytes(&mut ZipArchive::new(Cursor::new(bytes))?, name)
}

fn part_bytes(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Option<Vec<u8>>, Error> {
    match zip.by_name(name) {
        Ok(mut part) => {
            let mut raw = Vec::new();
            part.read_to_end(&mut raw)?;
            Ok(Some(raw))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The `ContentType` of the `<Override>` whose `PartName` is `/source`, part
/// names comparing without regard to ASCII case as OPC compares them.
fn source_content_type(types: &str) -> Option<String> {
    let mut in_override = false;
    let mut part_name = None;
    let mut content_type = None;
    for token in xmlparser::Tokenizer::from(types) {
        match token.ok()? {
            xmlparser::Token::ElementStart { local, .. } => {
                in_override = local.as_str() == "Override";
                part_name = None;
                content_type = None;
            }
            xmlparser::Token::Attribute { local, value, .. } if in_override => {
                match local.as_str() {
                    "PartName" => part_name = Some(value.as_str()),
                    "ContentType" => content_type = Some(value.as_str()),
                    _ => {}
                }
            }
            xmlparser::Token::ElementEnd { .. } if in_override => {
                in_override = false;
                let is_source = part_name.is_some_and(|p| {
                    p.strip_prefix('/')
                        .is_some_and(|p| p.eq_ignore_ascii_case(SOURCE_PART))
                });
                if is_source {
                    return content_type.filter(|t| !t.is_empty()).map(str::to_owned);
                }
            }
            _ => {}
        }
    }
    None
}

/// A new archive with `markup` as its document and every other part of
/// `original` carried over byte for byte, compression included. The
/// document is deflated, as the reference toolkit packs it.
pub fn repack(original: &[u8], markup: &str) -> Result<Vec<u8>, Error> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let mut zip = ZipArchive::new(Cursor::new(original))?;
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..zip.len() {
        let part = zip.by_index_raw(i)?;
        if part.name()? == DOCUMENT_PART {
            continue;
        }
        out.raw_copy_file(part)?;
    }
    out.start_file(
        DOCUMENT_PART,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )?;
    out.write_all(markup.as_bytes())?;
    Ok(out.finish()?.into_inner())
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

fn recording(name: &str) -> Option<Recording> {
    let (modality, file) = [Modality::Audio, Modality::Video]
        .into_iter()
        .find_map(|m| Some((m, name.strip_prefix(m.folder())?.strip_prefix('/')?)))?;
    let (stem, ext) = file.rsplit_once('.')?;
    let track: u32 = stem.parse().ok()?;
    (track >= 1 && !ext.is_empty()).then(|| Recording {
        modality,
        track,
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
        assert!(loaded.recordings.is_empty());
        assert!(loaded.source.is_none());
    }

    #[test]
    fn the_source_part_takes_its_type_from_its_override() {
        let types = |overrides: &str| {
            format!(
                "<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/>{overrides}</Types>"
            )
        };
        let source_type = |content_types: &str| {
            load_bytes(&archive(&[
                (CONTENT_TYPES_PART, content_types.as_bytes()),
                (DOCUMENT_PART, MARKUP.as_bytes()),
                (SOURCE_PART, b"%PDF"),
            ]))
            .unwrap()
            .source
            .expect("the archive has a source part")
            .content_type
        };
        assert_eq!(
            source_type(&types("<Override PartName=\"/document.xml\" ContentType=\"application/vnd.doclang.document+xml\"/><Override PartName=\"/Source\" ContentType=\"application/pdf\"/>")).as_deref(),
            Some("application/pdf")
        );
        assert_eq!(source_type(&types("")), None);
        assert_eq!(
            source_type(&types("<Override PartName=\"/source\" ContentType=\"\"/>")),
            None
        );
        assert_eq!(
            source_type(&types(
                "<Override PartName=\"/sources\" ContentType=\"application/pdf\"/>"
            )),
            None
        );
        assert_eq!(source_type("not xml <"), None);
    }

    #[test]
    fn recordings_are_numbered_by_track() {
        let bytes = archive(&[
            (DOCUMENT_PART, MARKUP.as_bytes()),
            ("video/3.mp4", b"v"),
            ("audio/1.wav", b"a"),
            ("audio/0.wav", b"a"),
            ("audio/notes.txt", b"x"),
            ("audiobook/1.mp3", b"x"),
        ]);
        let found: Vec<(Modality, u32)> = load_bytes(&bytes)
            .unwrap()
            .recordings
            .iter()
            .map(|r| (r.modality, r.track))
            .collect();
        assert_eq!(found, [(Modality::Audio, 1), (Modality::Video, 3)]);
    }

    #[test]
    fn repack_replaces_the_document_and_keeps_the_rest() {
        let bytes = archive(&[
            ("[Content_Types].xml", b"<Types/>"),
            (DOCUMENT_PART, MARKUP.as_bytes()),
            ("pages/1.png", b"png"),
            (SOURCE_PART, b"%PDF"),
        ]);
        let again = repack(&bytes, "<doclang/>").unwrap();
        let loaded = load_bytes(&again).unwrap();
        assert_eq!(loaded.markup, "<doclang/>");
        assert_eq!(read_part(&again, "pages/1.png").unwrap().unwrap(), b"png");
        assert_eq!(read_part(&again, SOURCE_PART).unwrap().unwrap(), b"%PDF");
        assert_eq!(
            read_part(&again, "[Content_Types].xml").unwrap().unwrap(),
            b"<Types/>"
        );
        assert!(read_part(&again, "missing").unwrap().is_none());
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
