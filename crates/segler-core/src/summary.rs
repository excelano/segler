//! A first look at a document: what version it claims, how many pages it
//! has, and which elements it uses. This is what `segler inspect` prints and
//! what the desktop window shows before anything is selected.
//!
//! It reads the XML directly rather than through the document model, because
//! it has to work on documents the model may refuse: a summary of an invalid
//! file is exactly what a person fixing it wants first.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::BTreeMap;

use roxmltree::Document;

/// Counts and declarations gathered from one pass over the markup.
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
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not well-formed XML: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("root element is <{0}>, not <doclang>")]
    NotDoclang(String),
}

impl Summary {
    /// Summarize DocLang markup. Well-formedness and a `doclang` root are the
    /// only things this insists on.
    pub fn of(markup: &str) -> Result<Summary, Error> {
        let doc = Document::parse(markup)?;
        let root = doc.root_element();
        if root.tag_name().name() != "doclang" {
            return Err(Error::NotDoclang(root.tag_name().name().to_owned()));
        }

        let mut elements = BTreeMap::new();
        for node in doc.descendants().filter(|n| n.is_element()) {
            *elements
                .entry(node.tag_name().name().to_owned())
                .or_insert(0) += 1;
        }
        let pages = elements.get("page_break").copied().unwrap_or(0) + 1;

        Ok(Summary {
            version: root
                .attribute("version")
                .unwrap_or(crate::SPEC_VERSION)
                .to_owned(),
            namespaced: root.tag_name().namespace() == Some(crate::NAMESPACE),
            pages,
            elements,
        })
    }

    /// Elements that carry geometry: each has four `location` children, so
    /// this is the `location` count over four, rounded down.
    pub fn located(&self) -> usize {
        self.elements.get("location").copied().unwrap_or(0) / 4
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
        assert_eq!(s.located(), 1);
    }

    #[test]
    fn defaults_when_root_is_bare() {
        let s = Summary::of("<doclang><text>x</text></doclang>").unwrap();
        assert_eq!(s.version, crate::SPEC_VERSION);
        assert!(!s.namespaced);
        assert_eq!(s.pages, 1);
        assert_eq!(s.located(), 0);
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
