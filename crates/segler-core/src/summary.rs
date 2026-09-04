//! A first look at a document: what version it claims, how many pages it
//! has, and which elements it uses. This is what `segler inspect` prints and
//! what the desktop window shows before anything is selected.
//!
//! It asks only for well-formed XML with a `doclang` root, so that a summary
//! of an invalid document is available: that is what a person fixing one
//! wants first.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::BTreeMap;

use crate::doclang;
use crate::tree::Document;

/// Counts and declarations gathered from one pass over the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// The root's `version` attribute, or the spec's default when absent.
    pub version: String,
    /// Whether the root declares the DocLang namespace.
    pub namespaced: bool,
    /// Pages, as the spec counts them: `page_break` elements plus one.
    pub pages: usize,
    /// Every element name in the document with how often it appears,
    /// including the root.
    pub elements: BTreeMap<String, usize>,
    /// Semantic elements whose head carries a bounding box.
    pub located: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Xml(#[from] crate::tree::Error),
    #[error("root element is <{0}>, not <doclang>")]
    NotDoclang(String),
}

impl Summary {
    /// Summarize DocLang markup.
    pub fn of(markup: &str) -> Result<Summary, Error> {
        let doc = Document::parse(markup)?;
        Summary::of_document(&doc)
    }

    pub fn of_document(doc: &Document) -> Result<Summary, Error> {
        if doclang::kind(&doc.root) != Some(doclang::Kind::Doclang) {
            return Err(Error::NotDoclang(doc.root.name().to_owned()));
        }
        let mut elements = BTreeMap::new();
        let mut located = 0;
        for el in doc.elements() {
            *elements.entry(el.name().to_owned()).or_insert(0) += 1;
            if doclang::kind(el).is_some_and(doclang::Kind::is_semantic)
                && doclang::head(el).bounds.is_some()
            {
                located += 1;
            }
        }
        Ok(Summary {
            version: doclang::version(doc).to_owned(),
            namespaced: doclang::namespaced(doc),
            pages: doclang::pages(doc).len(),
            elements,
            located,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_pages_elements_and_version() {
        let markup = r#"<doclang xmlns="https://www.doclang.ai/ns/v0" version="0.7">
  <heading><location value="1"/><location value="2"/><location value="3"/><location value="4"/>Title</heading>
  <text>One</text>
  <page_break/>
  <text>Two</text>
</doclang>"#;
        let s = Summary::of(markup).unwrap();
        assert_eq!(s.version, "0.7");
        assert!(s.namespaced);
        assert_eq!(s.pages, 2);
        assert_eq!(s.elements["text"], 2);
        assert_eq!(s.elements["doclang"], 1);
        assert_eq!(s.located, 1);
    }

    #[test]
    fn defaults_when_root_is_bare() {
        let s = Summary::of("<doclang><text>x</text></doclang>").unwrap();
        assert_eq!(s.version, crate::SPEC_VERSION);
        assert!(!s.namespaced);
        assert_eq!(s.pages, 1);
        assert_eq!(s.located, 0);
    }

    #[test]
    fn wrong_root_is_named() {
        match Summary::of("<html/>") {
            Err(Error::NotDoclang(name)) => assert_eq!(name, "html"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn malformed_xml_is_an_error() {
        assert!(matches!(Summary::of("<doclang>"), Err(Error::Xml(_))));
    }
}
