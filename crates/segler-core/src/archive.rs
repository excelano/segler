//! Opening a DocLang document from disk, whether it is bare markup or an
//! archive.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use zip::ZipArchive;

use crate::tree::{Document, Element};

/// The part an archive's `_rels/.rels` must point at.
pub const DOCUMENT_PART: &str = "document.xml";

/// The relationship type that names the main document part.
pub const DOCUMENT_RELATIONSHIP: &str = "http://doclang.ai/ns/package/2026/relationships/document";

pub const SOURCE_RELATIONSHIP: &str = "http://doclang.ai/ns/package/2026/relationships/source";

pub const SOURCE_METADATA_RELATIONSHIP: &str =
    "http://doclang.ai/ns/package/2026/relationships/source-metadata";

pub const SOURCE_METADATA_NAMESPACE: &str = "http://doclang.ai/ns/package/2026/source-metadata";

/// The content type `[Content_Types].xml` must declare for the document part.
pub const DOCUMENT_CONTENT_TYPE: &str = "application/vnd.doclang.document+xml";

pub const CONTENT_TYPES_PART: &str = "[Content_Types].xml";

pub const RELATIONSHIPS_PART: &str = "_rels/.rels";

const CONTENT_TYPES_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/content-types";
const RELATIONSHIPS_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Positional {
    Page(u32),
    Recording(Modality, u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Metadata<T> {
    Missing,
    Malformed(String),
    Read(T),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentTypes {
    pub defaults: HashMap<String, String>,
    pub overrides: HashMap<String, String>,
}

impl ContentTypes {
    pub fn of(&self, part: &str) -> Option<&str> {
        self.overrides
            .get(&part.to_lowercase())
            .or_else(|| extension(part).and_then(|e| self.defaults.get(&e)))
            .map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub kind: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Descriptor {
    Malformed(String),
    Read {
        namespace: String,
        local_name: String,
        name: Option<String>,
    },
}

impl Descriptor {
    pub fn is_source_descriptor(&self) -> bool {
        matches!(self, Descriptor::Read { namespace, local_name, .. }
            if namespace == SOURCE_METADATA_NAMESPACE && local_name == "source")
    }

    pub fn file_name(&self) -> Option<&str> {
        match self {
            Descriptor::Read { name, .. } if self.is_source_descriptor() => name.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub parts: Vec<String>,
    pub content_types: Metadata<ContentTypes>,
    pub relationships: Metadata<Vec<Relationship>>,
    pub relationship_targets: HashSet<String>,
    pub descriptors: HashMap<String, Descriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub part: String,
    pub content_type: Option<String>,
    pub file_name: Option<String>,
}

impl Package {
    pub fn relationships_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Relationship> {
        let all: &[Relationship] = match &self.relationships {
            Metadata::Read(all) => all,
            _ => &[],
        };
        all.iter().filter(move |r| r.kind == kind)
    }

    pub fn has_part(&self, name: &str) -> bool {
        self.parts.iter().any(|p| p == name)
    }

    pub fn source(&self) -> Option<Source> {
        let part = self
            .relationships_of(SOURCE_RELATIONSHIP)
            .find_map(|r| r.target.as_ref().filter(|t| self.has_part(t)))?;
        let content_type = match &self.content_types {
            Metadata::Read(types) => types.of(part).map(str::to_owned),
            _ => None,
        };
        let file_name = self
            .relationships_of(SOURCE_METADATA_RELATIONSHIP)
            .find_map(|r| self.descriptors.get(r.target.as_ref()?))
            .and_then(Descriptor::file_name)
            .map(str::to_owned);
        Some(Source {
            part: part.clone(),
            content_type,
            file_name,
        })
    }
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
    pub package: Option<Package>,
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
            package: None,
        })
    }
}

fn load_archive(bytes: &[u8]) -> Result<Loaded, Error> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let names: Vec<String> = zip
        .file_names()
        .map(|n| n.map(|n| n.into_owned()))
        .collect::<Result<_, _>>()?;
    let parts: Vec<String> = names.into_iter().filter(|n| !n.ends_with('/')).collect();

    let markup = String::from_utf8(part_bytes(&mut zip, DOCUMENT_PART)?.ok_or(Error::NoDocument)?)
        .map_err(|_| Error::NotUtf8)?;

    let mut pages: Vec<PagePart> = parts.iter().filter_map(|n| page_part(n)).collect();
    pages.sort_by_key(|p| p.number);

    let mut assets: Vec<String> = parts
        .iter()
        .filter(|n| n.starts_with("assets/"))
        .cloned()
        .collect();
    assets.sort();

    let mut recordings: Vec<Recording> = parts.iter().filter_map(|n| recording(n)).collect();
    recordings.sort_by(|a, b| (a.track, &a.name).cmp(&(b.track, &b.name)));

    let package = read_package(&mut zip, parts)?;

    Ok(Loaded {
        kind: Kind::Archive,
        markup,
        pages,
        assets,
        recordings,
        package: Some(package),
    })
}

fn read_package(zip: &mut ZipArchive<Cursor<&[u8]>>, parts: Vec<String>) -> Result<Package, Error> {
    let content_types = match part_bytes(zip, CONTENT_TYPES_PART)? {
        None => Metadata::Missing,
        Some(raw) => match parse(&raw) {
            Err(e) => Metadata::Malformed(e),
            Ok(doc) => Metadata::Read(content_types_of(&doc.root)),
        },
    };
    let relationships = match part_bytes(zip, RELATIONSHIPS_PART)? {
        None => Metadata::Missing,
        Some(raw) => match parse(&raw) {
            Err(e) => Metadata::Malformed(e),
            Ok(doc) => Metadata::Read(relationships_in(&doc.root, "")),
        },
    };

    let mut relationship_targets = HashSet::new();
    for part in &parts {
        let Some(base) = relationships_base(part) else {
            continue;
        };
        let Some(raw) = part_bytes(zip, part)? else {
            continue;
        };
        if let Ok(doc) = parse(&raw) {
            relationship_targets.extend(
                relationships_in(&doc.root, base)
                    .into_iter()
                    .filter_map(|r| r.target),
            );
        }
    }

    let mut package = Package {
        parts,
        content_types,
        relationships,
        relationship_targets,
        descriptors: HashMap::new(),
    };
    let described: Vec<String> = package
        .relationships_of(SOURCE_METADATA_RELATIONSHIP)
        .filter_map(|r| r.target.clone())
        .filter(|t| package.has_part(t))
        .collect();
    for part in described {
        let Some(raw) = part_bytes(zip, &part)? else {
            continue;
        };
        let descriptor = match parse(&raw) {
            Err(e) => Descriptor::Malformed(e),
            Ok(doc) => Descriptor::Read {
                namespace: namespace_of(&doc.root, &declare(&doc.root, &Vec::new())).to_owned(),
                local_name: doc.root.local_name().to_owned(),
                name: doc.root.attr("name").map(str::to_owned),
            },
        };
        package.descriptors.insert(part, descriptor);
    }
    Ok(package)
}

fn parse(raw: &[u8]) -> Result<Document, String> {
    let text = std::str::from_utf8(raw).map_err(|_| "not UTF-8".to_owned())?;
    Document::parse(text).map_err(|e| e.to_string())
}

fn content_types_of(root: &Element) -> ContentTypes {
    let mut types = ContentTypes::default();
    for el in children_in(root, CONTENT_TYPES_NAMESPACE) {
        let content_type = el.attr("ContentType").unwrap_or_default().to_owned();
        match el.local_name() {
            "Default" => {
                let extension = el.attr("Extension").unwrap_or_default().to_lowercase();
                types.defaults.insert(extension, content_type);
            }
            "Override" => {
                let part = el.attr("PartName").unwrap_or_default();
                let part = part.trim_start_matches('/').to_lowercase();
                types.overrides.insert(part, content_type);
            }
            _ => {}
        }
    }
    types
}

fn relationships_in(root: &Element, base: &str) -> Vec<Relationship> {
    children_in(root, RELATIONSHIPS_NAMESPACE)
        .into_iter()
        .filter(|el| el.local_name() == "Relationship")
        .map(|el| Relationship {
            kind: el.attr("Type").unwrap_or_default().to_owned(),
            target: (el.attr("TargetMode") != Some("External"))
                .then(|| target_part(el.attr("Target").unwrap_or_default(), base)),
        })
        .collect()
}

type Scope = Vec<(String, String)>;

fn declare(el: &Element, outer: &Scope) -> Scope {
    let mut scope = outer.clone();
    for a in el.attrs() {
        if a.name == "xmlns" {
            scope.push((String::new(), a.value.clone()));
        } else if let Some(prefix) = a.name.strip_prefix("xmlns:") {
            scope.push((prefix.to_owned(), a.value.clone()));
        }
    }
    scope
}

fn namespace_of<'s>(el: &Element, scope: &'s Scope) -> &'s str {
    let prefix = el.name().split_once(':').map_or("", |(p, _)| p);
    scope
        .iter()
        .rev()
        .find(|(p, _)| p == prefix)
        .map_or("", |(_, uri)| uri.as_str())
}

fn children_in<'a>(root: &'a Element, namespace: &str) -> Vec<&'a Element> {
    let scope = declare(root, &Vec::new());
    root.child_elements()
        .filter(|c| namespace_of(c, &declare(c, &scope)) == namespace)
        .collect()
}

fn extension(part: &str) -> Option<String> {
    let name = part.rsplit('/').next().unwrap_or(part);
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase())
}

pub fn relationships_base(part: &str) -> Option<&str> {
    let (folder, file) = part.rsplit_once('/')?;
    if !file.ends_with(".rels") {
        return None;
    }
    if folder == "_rels" {
        return Some("");
    }
    let base = &part[..folder.strip_suffix("/_rels")?.len() + 1];
    base[..base.len() - 1]
        .split('/')
        .all(|segment| !segment.is_empty())
        .then_some(base)
}

struct UriRef {
    absolute: bool,
    path: String,
}

fn split_uri(uri: &str) -> UriRef {
    let end = uri.find(['?', '#']).unwrap_or(uri.len());
    let mut rest = &uri[..end];
    let mut absolute = false;
    if let Some((scheme, after)) = rest.split_once(':') {
        let mut chars = scheme.chars();
        if chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            absolute = true;
            rest = after;
        }
    }
    if let Some(after) = rest.strip_prefix("//") {
        absolute = true;
        rest = after.find('/').map_or("", |i| &after[i..]);
    }
    UriRef {
        absolute,
        path: percent_decode(rest),
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalize(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            s => segments.push(s),
        }
    }
    segments.join("/")
}

fn target_part(target: &str, base: &str) -> String {
    let path = split_uri(target).path;
    if path.starts_with('/') {
        normalize(&path)
    } else {
        normalize(&format!("{base}{path}"))
    }
}

pub fn markup_part(uri: &str) -> Option<String> {
    let uri = split_uri(uri);
    let part = normalize(&uri.path);
    (!uri.absolute && !part.is_empty()).then_some(part)
}

pub fn positional(name: &str) -> Option<Positional> {
    let numbered = |file: &str| -> Option<u32> {
        let digits = file.find(|c: char| !c.is_ascii_digit())?;
        let (number, ext) = file.split_at(digits);
        let valid = !number.is_empty()
            && !number.starts_with('0')
            && ext.len() > 1
            && ext.starts_with('.')
            && !ext.contains('/');
        valid.then(|| number.parse().unwrap_or(u32::MAX))
    };
    if let Some(file) = name.strip_prefix("pages/") {
        return numbered(file).map(Positional::Page);
    }
    [Modality::Audio, Modality::Video]
        .into_iter()
        .find_map(|m| Some((m, name.strip_prefix(m.folder())?.strip_prefix('/')?)))
        .and_then(|(m, file)| numbered(file).map(|n| Positional::Recording(m, n)))
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

fn page_part(name: &str) -> Option<PagePart> {
    let Some(Positional::Page(number)) = positional(name) else {
        return None;
    };
    let ext = extension(name)?;
    PAGE_EXTENSIONS.contains(&ext.as_str()).then(|| PagePart {
        number,
        name: name.to_owned(),
    })
}

fn recording(name: &str) -> Option<Recording> {
    let Some(Positional::Recording(modality, track)) = positional(name) else {
        return None;
    };
    Some(Recording {
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

    fn package(parts: &[(&str, &[u8])]) -> Package {
        load_bytes(&archive(parts)).unwrap().package.unwrap()
    }

    const TYPES: &str = "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"PDF\" ContentType=\"application/pdf\"/><Override PartName=\"/Document.xml\" ContentType=\"application/vnd.doclang.document+xml\"/></Types>";

    fn rels(body: &str) -> String {
        format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{body}</Relationships>")
    }

    #[test]
    fn bare_markup_loads_as_markup() {
        let loaded = load_bytes(MARKUP.as_bytes()).unwrap();
        assert_eq!(loaded.kind, Kind::Markup);
        assert_eq!(loaded.markup, MARKUP);
        assert!(loaded.pages.is_empty());
        assert!(loaded.package.is_none());
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
            ("pages/3.txt", b"not an image"),
            ("assets/chart.svg", b"<svg/>"),
        ]);
        let loaded = load_bytes(&bytes).unwrap();
        assert_eq!(loaded.kind, Kind::Archive);
        assert_eq!(loaded.markup, MARKUP);
        let numbers: Vec<u32> = loaded.pages.iter().map(|p| p.number).collect();
        assert_eq!(numbers, [1, 2, 10]);
        assert_eq!(loaded.assets, ["assets/chart.svg"]);
        assert!(loaded.recordings.is_empty());
        assert!(loaded.package.unwrap().source().is_none());
    }

    #[test]
    fn content_types_compare_without_case() {
        let p = package(&[
            (CONTENT_TYPES_PART, TYPES.as_bytes()),
            (DOCUMENT_PART, b"<doclang/>"),
        ]);
        let Metadata::Read(types) = &p.content_types else {
            panic!("{:?}", p.content_types);
        };
        assert_eq!(types.of("document.xml"), Some(DOCUMENT_CONTENT_TYPE));
        assert_eq!(types.of("source.pdf"), Some("application/pdf"));
        assert_eq!(types.of("source"), None);
        let p = package(&[
            (CONTENT_TYPES_PART, b"<Types"),
            (DOCUMENT_PART, b"<doclang/>"),
        ]);
        assert!(matches!(p.content_types, Metadata::Malformed(_)));
        assert_eq!(p.relationships, Metadata::Missing);
    }

    #[test]
    fn the_source_is_found_by_its_relationship_not_its_name() {
        let r = rels(
            "<Relationship Id=\"r1\" Type=\"http://doclang.ai/ns/package/2026/relationships/document\" Target=\"document.xml\"/>\
             <Relationship Id=\"r2\" Type=\"http://doclang.ai/ns/package/2026/relationships/source\" Target=\"/audio/1.mp3\"/>\
             <Relationship Id=\"r3\" Type=\"http://doclang.ai/ns/package/2026/relationships/source-metadata\" Target=\"source-metadata.xml\"/>",
        );
        let p = package(&[
            (CONTENT_TYPES_PART, b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"mp3\" ContentType=\"audio/mpeg\"/></Types>"),
            (RELATIONSHIPS_PART, r.as_bytes()),
            (DOCUMENT_PART, b"<doclang/>"),
            ("audio/1.mp3", b"ID3"),
            ("source", b"%PDF"),
            ("source-metadata.xml", b"<m:source xmlns:m=\"http://doclang.ai/ns/package/2026/source-metadata\" name=\"episode-12.mp3\"/>"),
        ]);
        assert_eq!(
            p.source(),
            Some(Source {
                part: "audio/1.mp3".to_owned(),
                content_type: Some("audio/mpeg".to_owned()),
                file_name: Some("episode-12.mp3".to_owned()),
            })
        );
        assert!(p.relationship_targets.contains("source-metadata.xml"));

        let unrelated = package(&[(DOCUMENT_PART, b"<doclang/>"), ("source", b"%PDF")]);
        assert_eq!(unrelated.source(), None);
    }

    #[test]
    fn relationships_outside_their_namespace_or_external_target_no_part() {
        let r = "<Relationships><Relationship Type=\"t\" Target=\"a\"/></Relationships>";
        let p = package(&[
            (RELATIONSHIPS_PART, r.as_bytes()),
            (DOCUMENT_PART, b"<doclang/>"),
        ]);
        assert_eq!(p.relationships, Metadata::Read(Vec::new()));
        let r = rels(
            "<Relationship Type=\"t\" Target=\"https://example.com/a\" TargetMode=\"External\"/>",
        );
        let p = package(&[
            (RELATIONSHIPS_PART, r.as_bytes()),
            (DOCUMENT_PART, b"<doclang/>"),
        ]);
        assert_eq!(
            p.relationships,
            Metadata::Read(vec![Relationship {
                kind: "t".to_owned(),
                target: None
            }])
        );
        assert!(p.relationship_targets.is_empty());
    }

    #[test]
    fn a_part_relationship_resolves_from_its_folder() {
        let r = rels("<Relationship Type=\"t\" Target=\"../assets/a%20b.png\"/>");
        let p = package(&[
            (DOCUMENT_PART, b"<doclang/>"),
            ("audio/_rels/1.mp3.rels", r.as_bytes()),
        ]);
        assert!(p.relationship_targets.contains("assets/a b.png"));
    }

    #[test]
    fn relationship_parts_and_their_folders() {
        assert_eq!(relationships_base("_rels/.rels"), Some(""));
        assert_eq!(relationships_base("audio/_rels/1.mp3.rels"), Some("audio/"));
        assert_eq!(relationships_base("a/b/_rels/c.rels"), Some("a/b/"));
        assert_eq!(relationships_base("a//_rels/c.rels"), None);
        assert_eq!(relationships_base("rels/c.rels"), None);
        assert_eq!(relationships_base("_rels/c.xml"), None);
    }

    #[test]
    fn markup_uris_name_parts_from_the_root() {
        assert_eq!(
            markup_part("assets/chart.svg").as_deref(),
            Some("assets/chart.svg")
        );
        assert_eq!(
            markup_part("/assets/./x/../c%2Esvg#f").as_deref(),
            Some("assets/c.svg")
        );
        assert_eq!(markup_part("https://example.com/a.png"), None);
        assert_eq!(markup_part("data:image/png;base64,AAAA"), None);
        assert_eq!(markup_part("//host/a.png"), None);
        assert_eq!(markup_part("#top"), None);
        assert_eq!(markup_part(""), None);
    }

    #[test]
    fn positional_parts() {
        assert_eq!(positional("pages/3.PNG"), Some(Positional::Page(3)));
        assert_eq!(positional("pages/1.png.bak"), Some(Positional::Page(1)));
        assert_eq!(
            positional("audio/2.mp3"),
            Some(Positional::Recording(Modality::Audio, 2))
        );
        for not in [
            "pages/0.png",
            "pages/01.png",
            "pages/1.",
            "pages/1",
            "pages/a.png",
            "assets/3.png",
            "audiobook/1.mp3",
            "pages/1.png/x",
        ] {
            assert_eq!(positional(not), None, "{not}");
        }
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
            ("source.pdf", b"%PDF"),
        ]);
        let again = repack(&bytes, "<doclang/>").unwrap();
        let loaded = load_bytes(&again).unwrap();
        assert_eq!(loaded.markup, "<doclang/>");
        assert_eq!(read_part(&again, "pages/1.png").unwrap().unwrap(), b"png");
        assert_eq!(read_part(&again, "source.pdf").unwrap().unwrap(), b"%PDF");
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
}
