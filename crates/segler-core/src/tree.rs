//! The lossless document tree.
//!
//! Every element, attribute, text run, CDATA section, comment and processing
//! instruction in a file survives a parse and a serialize, in order, and so
//! do the bytes around them: indentation, the form of each start tag, the
//! way each character was escaped. A document parsed and written back is the
//! same bytes. That is the property that separates an editor's model from a
//! converter's, and it is checked on the specification's corpus by
//! `segler corpus` rather than assumed.
//!
//! The mechanism is a raw cache. An element remembers the exact source of its
//! start tag and a text run remembers its exact escaped source, and each is
//! written back verbatim until something edits it. An edit drops the cache and
//! the node is regenerated in a canonical form: double-quoted attributes, the
//! five predefined entities, `<name/>` for an element with no children.
//! Untouched nodes never change; edited nodes diff only where they were
//! edited.
//!
//! The tree knows nothing about DocLang. Names are kept as written, prefix and
//! all, and no namespace is resolved. The `doclang` module reads DocLang
//! meaning out of this tree; this module only keeps the bytes.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::borrow::Cow;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use xmlparser::{ElementEnd, Token, Tokenizer};

/// Identifies one element for the life of the process. Ids are never reused,
/// so a selection can outlive an edit and be checked rather than trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(u64);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl ElementId {
    fn fresh() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not well-formed XML: {0}")]
    Syntax(String),
    #[error("document type declarations are not allowed")]
    Dtd,
    #[error("undeclared entity &{0};")]
    UndeclaredEntity(String),
    #[error("character reference &#{0}; is not an XML character")]
    BadCharRef(String),
    #[error("no root element")]
    NoRoot,
}

impl From<xmlparser::Error> for Error {
    fn from(e: xmlparser::Error) -> Self {
        Error::Syntax(e.to_string())
    }
}

/// A whole file: whatever came before the root element, the root, and
/// whatever came after. The prolog and epilog are kept as bytes because
/// nothing edits them; the XML declaration lives in the prolog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Whether the file began with a byte-order mark, written back if so.
    pub bom: bool,
    pub prolog: String,
    pub root: Element,
    pub epilog: String,
}

/// One node in an element's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(Text),
    /// A CDATA section, holding its literal content.
    CData(String),
    /// A comment, holding the text between `<!--` and `-->`.
    Comment(String),
    /// A processing instruction: `<?target data?>`.
    Pi {
        target: String,
        data: Option<String>,
    },
}

/// A run of character data. `value` is what the text means, with entities
/// decoded; `raw` is how the file spelled it, kept until the value changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    value: String,
    raw: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// The name as written, prefix included.
    pub name: String,
    /// The value with entities decoded and whitespace normalized.
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    id: ElementId,
    /// The name as written, prefix included.
    name: String,
    attrs: Vec<Attribute>,
    children: Vec<Node>,
    /// The exact source of the start tag, through `>` or `/>`, valid while
    /// the name and attributes are untouched and, for an empty tag, while
    /// the element has no children.
    raw_start: Option<String>,
    /// The exact source of the end tag, kept only when it differs from the
    /// canonical `</name>`.
    raw_end: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing

impl Document {
    /// Parse a file's contents. A document type declaration is refused, as
    /// the reference toolkit refuses it; there is nothing in DocLang for one
    /// to declare and it is the way entity expansion attacks arrive.
    pub fn parse(source: &str) -> Result<Document, Error> {
        let (bom, src) = match source.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, source),
        };

        // (element under construction, byte offset of its `<`)
        let mut stack: Vec<(Element, usize)> = Vec::new();
        let mut root: Option<(Element, usize, usize)> = None;

        for token in Tokenizer::from(src) {
            match token? {
                Token::DtdStart { .. } | Token::EmptyDtd { .. } => return Err(Error::Dtd),
                Token::EntityDeclaration { .. } | Token::DtdEnd { .. } => return Err(Error::Dtd),

                // Outside the root these are part of the prolog or epilog,
                // which are kept as bytes.
                Token::Declaration { .. } => {}
                Token::Comment { text, .. } => {
                    if let Some((el, _)) = stack.last_mut() {
                        el.children.push(Node::Comment(text.as_str().to_owned()));
                    }
                }
                Token::ProcessingInstruction {
                    target, content, ..
                } => {
                    if let Some((el, _)) = stack.last_mut() {
                        el.children.push(Node::Pi {
                            target: target.as_str().to_owned(),
                            data: content.map(|c| c.as_str().to_owned()),
                        });
                    }
                }

                Token::ElementStart {
                    prefix,
                    local,
                    span,
                } => {
                    let name = qualified(prefix.as_str(), local.as_str());
                    stack.push((Element::new(name), span.start()));
                }
                Token::Attribute {
                    prefix,
                    local,
                    value,
                    ..
                } => {
                    let (el, _) = stack.last_mut().expect("attribute outside a start tag");
                    el.attrs.push(Attribute {
                        name: qualified(prefix.as_str(), local.as_str()),
                        value: decode_attribute(value.as_str())?.into_owned(),
                    });
                }
                Token::ElementEnd { end, span } => match end {
                    ElementEnd::Open => {
                        let (el, start) = stack.last_mut().expect("`>` outside a start tag");
                        el.raw_start = Some(src[*start..span.end()].to_owned());
                    }
                    ElementEnd::Empty => {
                        let (mut el, start) = stack.pop().expect("`/>` outside a start tag");
                        el.raw_start = Some(src[start..span.end()].to_owned());
                        attach(&mut stack, &mut root, el, start, span.end());
                    }
                    ElementEnd::Close(..) => {
                        let (mut el, start) = stack.pop().expect("end tag without a start tag");
                        let raw = &src[span.start()..span.end()];
                        if raw != format!("</{}>", el.name) {
                            el.raw_end = Some(raw.to_owned());
                        }
                        attach(&mut stack, &mut root, el, start, span.end());
                    }
                },

                Token::Text { text } => {
                    let (el, _) = stack.last_mut().expect("text outside the root");
                    el.children.push(Node::Text(Text {
                        value: decode(text.as_str())?.into_owned(),
                        raw: Some(text.as_str().to_owned()),
                    }));
                }
                Token::Cdata { text, .. } => {
                    let (el, _) = stack.last_mut().expect("CDATA outside the root");
                    el.children.push(Node::CData(text.as_str().to_owned()));
                }
            }
        }

        if let Some((el, _)) = stack.last() {
            return Err(Error::Syntax(format!("<{}> is never closed", el.name)));
        }
        let (root, start, end) = root.ok_or(Error::NoRoot)?;
        Ok(Document {
            bom,
            prolog: src[..start].to_owned(),
            root,
            epilog: src[end..].to_owned(),
        })
    }

    /// A new document around `root`, with the declaration a DocLang file
    /// conventionally carries.
    pub fn new(root: Element) -> Document {
        Document {
            bom: false,
            prolog: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n".to_owned(),
            root,
            epilog: "\n".to_owned(),
        }
    }

    /// Every element in document order, the root first.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        let mut pending = vec![&self.root];
        std::iter::from_fn(move || {
            let el = pending.pop()?;
            pending.extend(el.child_elements().collect::<Vec<_>>().into_iter().rev());
            Some(el)
        })
    }

    pub fn find(&self, id: ElementId) -> Option<&Element> {
        self.elements().find(|e| e.id == id)
    }

    pub fn find_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        fn walk(el: &mut Element, id: ElementId) -> Option<&mut Element> {
            if el.id == id {
                return Some(el);
            }
            el.children.iter_mut().find_map(|n| match n {
                Node::Element(child) => walk(child, id),
                _ => None,
            })
        }
        walk(&mut self.root, id)
    }
}

fn attach(
    stack: &mut [(Element, usize)],
    root: &mut Option<(Element, usize, usize)>,
    el: Element,
    start: usize,
    end: usize,
) {
    match stack.last_mut() {
        Some((parent, _)) => parent.children.push(Node::Element(el)),
        None => *root = Some((el, start, end)),
    }
}

fn qualified(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_owned()
    } else {
        format!("{prefix}:{local}")
    }
}

/// Decode the predefined entities and character references in a run of
/// text. Anything else after an ampersand is an error: DocLang has no DTD
/// to declare an entity in.
pub fn decode(raw: &str) -> Result<Cow<'_, str>, Error> {
    if !raw.contains('&') {
        return Ok(Cow::Borrowed(raw));
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(semi) = after.find(';') else {
            return Err(Error::UndeclaredEntity(after.to_owned()));
        };
        let name = &after[..semi];
        match name {
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let Some(num) = name.strip_prefix('#') else {
                    return Err(Error::UndeclaredEntity(name.to_owned()));
                };
                let code = match num.strip_prefix('x') {
                    Some(hex) => u32::from_str_radix(hex, 16),
                    None => num.parse::<u32>(),
                }
                .map_err(|_| Error::BadCharRef(num.to_owned()))?;
                let ch = char::from_u32(code)
                    .filter(|c| is_xml_char(*c))
                    .ok_or_else(|| Error::BadCharRef(num.to_owned()))?;
                out.push(ch);
            }
        }
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Ok(Cow::Owned(out))
}

/// Attribute values additionally have literal whitespace characters
/// normalized to spaces, as XML requires; a character reference to a newline
/// survives, a newline typed into the value does not.
fn decode_attribute(raw: &str) -> Result<Cow<'_, str>, Error> {
    if raw.contains(['\t', '\n', '\r']) {
        let normalized: String = raw
            .chars()
            .map(|c| {
                if matches!(c, '\t' | '\n' | '\r') {
                    ' '
                } else {
                    c
                }
            })
            .collect();
        return decode(&normalized).map(|c| Cow::Owned(c.into_owned()));
    }
    decode(raw)
}

/// The XML 1.0 character range.
pub fn is_xml_char(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

// ---------------------------------------------------------------------------
// The element API

impl Element {
    /// A new element with no attributes and no children. It serializes as
    /// `<name/>` until it is given content.
    pub fn new(name: impl Into<String>) -> Element {
        Element {
            id: ElementId::fresh(),
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
            raw_start: None,
            raw_end: None,
        }
    }

    pub fn id(&self) -> ElementId {
        self.id
    }

    /// The name as written, prefix included.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The name without any prefix.
    pub fn local_name(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Set an attribute, replacing one of the same name in place or adding
    /// it at the end. The start tag is regenerated on the next write.
    pub fn set_attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        match self.attrs.iter_mut().find(|a| a.name == name) {
            Some(a) => a.value = value,
            None => self.attrs.push(Attribute { name, value }),
        }
        self.touch_tag();
    }

    pub fn remove_attr(&mut self, name: &str) -> Option<Attribute> {
        let i = self.attrs.iter().position(|a| a.name == name)?;
        self.touch_tag();
        Some(self.attrs.remove(i))
    }

    pub fn children(&self) -> &[Node] {
        &self.children
    }

    pub fn child_elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn child_elements_mut(&mut self) -> impl Iterator<Item = &mut Element> {
        self.children.iter_mut().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn push(&mut self, node: Node) {
        self.leave_empty_form();
        self.children.push(node);
    }

    pub fn insert(&mut self, index: usize, node: Node) {
        self.leave_empty_form();
        self.children.insert(index, node);
    }

    pub fn remove(&mut self, index: usize) -> Node {
        self.children.remove(index)
    }

    /// Replace all content with one text run.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.leave_empty_form();
        self.children.clear();
        self.children.push(Node::Text(Text::new(text)));
    }

    /// The character data of this element and everything under it, in
    /// order, comments and processing instructions excluded.
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        for child in &self.children {
            match child {
                Node::Element(e) => e.collect_text(out),
                Node::Text(t) => out.push_str(&t.value),
                Node::CData(s) => out.push_str(s),
                Node::Comment(_) | Node::Pi { .. } => {}
            }
        }
    }

    fn touch_tag(&mut self) {
        self.raw_start = None;
        self.raw_end = None;
    }

    /// An element parsed from `<x/>` that gains children needs a real start
    /// tag; one parsed from `<x></x>` keeps its own.
    fn leave_empty_form(&mut self) {
        if self.raw_start.as_deref().is_some_and(|s| s.ends_with("/>")) {
            self.raw_start = None;
        }
    }
}

impl Text {
    pub fn new(value: impl Into<String>) -> Text {
        Text {
            value: value.into(),
            raw: None,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.raw = None;
    }
}

// ---------------------------------------------------------------------------
// Writing

impl Document {
    pub fn write_to(&self, out: &mut String) {
        if self.bom {
            out.push('\u{feff}');
        }
        out.push_str(&self.prolog);
        self.root.write_to(out);
        out.push_str(&self.epilog);
    }
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.write_to(&mut s);
        f.write_str(&s)
    }
}

impl Element {
    pub fn write_to(&self, out: &mut String) {
        match &self.raw_start {
            Some(raw) => {
                out.push_str(raw);
                if raw.ends_with("/>") {
                    return;
                }
            }
            None => {
                out.push('<');
                out.push_str(&self.name);
                for a in &self.attrs {
                    out.push(' ');
                    out.push_str(&a.name);
                    out.push_str("=\"");
                    escape_into(&a.value, true, out);
                    out.push('"');
                }
                if self.children.is_empty() {
                    out.push_str("/>");
                    return;
                }
                out.push('>');
            }
        }
        for child in &self.children {
            child.write_to(out);
        }
        match &self.raw_end {
            Some(raw) => out.push_str(raw),
            None => {
                out.push_str("</");
                out.push_str(&self.name);
                out.push('>');
            }
        }
    }
}

impl Node {
    pub fn write_to(&self, out: &mut String) {
        match self {
            Node::Element(e) => e.write_to(out),
            Node::Text(t) => match &t.raw {
                Some(raw) => out.push_str(raw),
                None => escape_into(&t.value, false, out),
            },
            Node::CData(s) => {
                out.push_str("<![CDATA[");
                // A literal `]]>` inside a section would end it early; split
                // the section around it.
                out.push_str(&s.replace("]]>", "]]]]><![CDATA[>"));
                out.push_str("]]>");
            }
            Node::Comment(s) => {
                out.push_str("<!--");
                out.push_str(s);
                out.push_str("-->");
            }
            Node::Pi { target, data } => {
                out.push_str("<?");
                out.push_str(target);
                if let Some(d) = data {
                    out.push(' ');
                    out.push_str(d);
                }
                out.push_str("?>");
            }
        }
    }
}

/// Escape a value for text content, or for a double-quoted attribute when
/// `attribute` is set. Characters outside the XML range pass through; the
/// edit commands refuse them before they reach a node.
fn escape_into(value: &str, attribute: bool, out: &mut String) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(src: &str) {
        let doc = Document::parse(src).unwrap();
        assert_eq!(doc.to_string(), src);
    }

    #[test]
    fn identity_on_every_syntax_the_corpus_uses() {
        roundtrip("<doclang/>");
        roundtrip("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<doclang>\n  <text>Hi</text>\n</doclang>\n");
        roundtrip(
            "<doclang><!-- a comment --><text>a &amp; b &#169; &#xA9; &lt;c&gt;</text></doclang>",
        );
        roundtrip("<doclang><code><![CDATA[if (a < b) x]]></code></doclang>");
        roundtrip("<doclang   version='0.7'\n  xmlns=\"https://www.doclang.ai/ns/v0\"><page_break /><x></x></doclang>");
        roundtrip("<a><?pi some data?><b/></a>\n\n");
        roundtrip(
            "<doclang><custom xmlns:acme=\"u\"><acme:smiles>C1</acme:smiles></custom></doclang>",
        );
        roundtrip("<a></a >");
        roundtrip("\u{feff}<a/>");
    }

    #[test]
    fn entities_decode_and_attributes_normalize() {
        let doc =
            Document::parse("<a t=\"x\ny&#10;z &quot;q&quot;\"><![CDATA[<raw>]]>&lt;&#65;</a>")
                .unwrap();
        assert_eq!(doc.root.attr("t"), Some("x y\nz \"q\""));
        assert_eq!(doc.root.text(), "<raw><A");
    }

    #[test]
    fn edits_regenerate_only_what_changed() {
        let src = "<doclang>\n  <heading  level='2'>Old</heading>\n  <page_break />\n</doclang>";
        let mut doc = Document::parse(src).unwrap();
        let heading = doc.root.child_elements().next().unwrap().id();
        let h = doc.find_mut(heading).unwrap();
        h.set_attr("level", "3");
        h.set_text("New & <improved>");
        assert_eq!(
            doc.to_string(),
            "<doclang>\n  <heading level=\"3\">New &amp; &lt;improved&gt;</heading>\n  <page_break />\n</doclang>"
        );
    }

    #[test]
    fn an_empty_tag_that_gains_children_opens_up() {
        let mut doc = Document::parse("<a><b/></a>").unwrap();
        let b = doc.root.child_elements().next().unwrap().id();
        doc.find_mut(b).unwrap().push(Node::Text(Text::new("x")));
        assert_eq!(doc.to_string(), "<a><b>x</b></a>");
        let mut fresh = Element::new("c");
        assert_eq!(
            Document::new(fresh.clone()).to_string(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<c/>\n"
        );
        fresh.set_attr("k", "v \"q\"");
        let mut s = String::new();
        fresh.write_to(&mut s);
        assert_eq!(s, "<c k=\"v &quot;q&quot;\"/>");
    }

    #[test]
    fn cdata_delimiter_in_edited_content_is_split() {
        let mut root = Element::new("a");
        root.push(Node::CData("x]]>y".into()));
        let mut s = String::new();
        root.write_to(&mut s);
        assert_eq!(s, "<a><![CDATA[x]]]]><![CDATA[>y]]></a>");
        assert_eq!(Document::parse(&s).unwrap().root.text(), "x]]>y");
    }

    #[test]
    fn refusals() {
        assert!(matches!(
            Document::parse("<!DOCTYPE a><a/>"),
            Err(Error::Dtd)
        ));
        assert!(matches!(
            Document::parse("<a>&nbsp;</a>"),
            Err(Error::UndeclaredEntity(_))
        ));
        assert!(matches!(
            Document::parse("<a>&#0;</a>"),
            Err(Error::BadCharRef(_))
        ));
        assert!(matches!(Document::parse("<a>"), Err(Error::Syntax(_))));
        assert!(matches!(
            Document::parse("  "),
            Err(Error::NoRoot | Error::Syntax(_))
        ));
    }

    #[test]
    fn ids_are_stable_and_findable() {
        let doc = Document::parse("<a><b/><c><d/></c></a>").unwrap();
        let names: Vec<&str> = doc.elements().map(Element::name).collect();
        assert_eq!(names, ["a", "b", "c", "d"]);
        let d = doc.elements().last().unwrap().id();
        assert_eq!(doc.find(d).unwrap().name(), "d");
        assert!(doc.find(ElementId::fresh()).is_none());
    }
}
