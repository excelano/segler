//! DocLang meaning read out of the tree: the element vocabulary, an
//! element's head, the document head, and pagination.
//!
//! Everything here is a view over `tree`. Nothing is copied out of the
//! tree and nothing here is a second model; the tree is the model and this
//! module knows what its names mean. Where the specification gives a
//! default, the accessor applies it, so a caller sees `body` for an element
//! with no `layer` and 512 for a document with no `default_resolution`.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::ops::Range;

use crate::tree::{Document, Element, Node};

/// The specification's grouping of its vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Document-level function: `doclang`, `head`, `page_break`.
    Special,
    /// Document components that carry a head and a body.
    Semantic,
    /// Elements of an element head.
    Property,
    /// Payload elements: `src`, `tabular`, `checkbox`, `content`.
    Payload,
    /// Inline formatting.
    Formatting,
    /// OTSL cell markers and the list divider.
    Structural,
    /// Children of the document head.
    DocumentHead,
}

macro_rules! kinds {
    ($($variant:ident = $name:literal : $category:ident),* $(,)?) => {
        /// Every element the specification defines.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Kind { $($variant),* }

        impl Kind {
            pub const ALL: &'static [Kind] = &[$(Kind::$variant),*];

            pub fn from_name(name: &str) -> Option<Kind> {
                match name { $($name => Some(Kind::$variant),)* _ => None }
            }

            pub fn name(self) -> &'static str {
                match self { $(Kind::$variant => $name),* }
            }

            pub fn category(self) -> Category {
                match self { $(Kind::$variant => Category::$category),* }
            }
        }
    };
}

kinds! {
    Doclang = "doclang": Special,
    Head = "head": Special,
    PageBreak = "page_break": Special,

    Text = "text": Semantic,
    Heading = "heading": Semantic,
    Footnote = "footnote": Semantic,
    PageHeader = "page_header": Semantic,
    PageFooter = "page_footer": Semantic,
    FieldRegion = "field_region": Semantic,
    List = "list": Semantic,
    Table = "table": Semantic,
    Index = "index": Semantic,
    Formula = "formula": Semantic,
    Code = "code": Semantic,
    Picture = "picture": Semantic,
    Marker = "marker": Semantic,
    Group = "group": Semantic,
    FieldHeading = "field_heading": Semantic,
    FieldItem = "field_item": Semantic,
    Key = "key": Semantic,
    Value = "value": Semantic,
    Hint = "hint": Semantic,
    Caption = "caption": Semantic,

    Label = "label": Property,
    Thread = "thread": Property,
    Xref = "xref": Property,
    Href = "href": Property,
    Description = "description": Property,
    Summary = "summary": Property,
    Custom = "custom": Property,
    Location = "location": Property,
    Layer = "layer": Property,

    Src = "src": Payload,
    Tabular = "tabular": Payload,
    Checkbox = "checkbox": Payload,
    Content = "content": Payload,

    Bold = "bold": Formatting,
    Italic = "italic": Formatting,
    Underline = "underline": Formatting,
    Strikethrough = "strikethrough": Formatting,
    Superscript = "superscript": Formatting,
    Subscript = "subscript": Formatting,
    Handwriting = "handwriting": Formatting,
    Rtl = "rtl": Formatting,

    Fcel = "fcel": Structural,
    Ecel = "ecel": Structural,
    Ched = "ched": Structural,
    Rhed = "rhed": Structural,
    Corn = "corn": Structural,
    Srow = "srow": Structural,
    Lcel = "lcel": Structural,
    Ucel = "ucel": Structural,
    Xcel = "xcel": Structural,
    Nl = "nl": Structural,
    Ldiv = "ldiv": Structural,

    DefaultResolution = "default_resolution": DocumentHead,
}

impl Kind {
    /// Whether the element carries an element head and a body.
    pub fn is_semantic(self) -> bool {
        self.category() == Category::Semantic
    }
}

/// The spec's grid size when `default_resolution` says nothing.
pub const DEFAULT_RESOLUTION: u32 = 512;

/// What kind of DocLang element this is, or `None` for a name the spec does
/// not define: a prefixed custom-vocabulary element, or a typo.
pub fn kind(el: &Element) -> Option<Kind> {
    if el.name().contains(':') {
        return None;
    }
    Kind::from_name(el.name())
}

/// One coordinate: the grid value and the resolution it is against, when
/// the element names one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord {
    pub value: u32,
    pub resolution: Option<u32>,
}

impl Coord {
    /// The coordinate as a fraction of the page along its axis.
    pub fn fraction(self, default_resolution: u32) -> f64 {
        let res = self.resolution.unwrap_or(default_resolution).max(1);
        f64::from(self.value) / f64::from(res)
    }
}

/// A bounding box from the four `location` elements of a head, in the
/// spec's order: `x_min, y_min, x_max, y_max` from the top left of the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x_min: Coord,
    pub y_min: Coord,
    pub x_max: Coord,
    pub y_max: Coord,
}

impl Bounds {
    /// `[x_min, y_min, x_max, y_max]` as fractions of the page, given the
    /// document's `(width, height)` default resolution.
    pub fn fractions(&self, default_resolution: (u32, u32)) -> [f64; 4] {
        let (w, h) = default_resolution;
        [
            self.x_min.fraction(w),
            self.y_min.fraction(h),
            self.x_max.fraction(w),
            self.y_max.fraction(h),
        ]
    }
}

/// An element head, read from the leading property children of a semantic
/// element. `body` is the range of the element's children after the head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head<'a> {
    pub label: Option<&'a str>,
    pub thread: Option<u64>,
    pub xref: Option<u64>,
    pub href: Option<&'a str>,
    /// The `layer` value, with the spec's default applied.
    pub layer: &'a str,
    pub bounds: Option<Bounds>,
    pub caption: Option<&'a Element>,
    pub description: Option<&'a Element>,
    pub summary: Option<&'a Element>,
    pub custom: Option<&'a Element>,
    /// Indices into `children()` of everything after the head.
    pub body: Range<usize>,
}

/// Read the head of `el`. Tolerant of order: any property element that
/// appears before the first body node is taken, so that a document the
/// validator will reject for head order still shows its geometry.
pub fn head(el: &Element) -> Head<'_> {
    let mut head = Head {
        label: None,
        thread: None,
        xref: None,
        href: None,
        layer: "body",
        bounds: None,
        caption: None,
        description: None,
        summary: None,
        custom: None,
        body: 0..el.children().len(),
    };
    let mut locations: Vec<Coord> = Vec::with_capacity(4);
    let mut body_start = el.children().len();

    for (i, node) in el.children().iter().enumerate() {
        let child = match node {
            Node::Element(e) => e,
            Node::Text(t) if t.value().trim().is_empty() => continue,
            Node::Comment(_) | Node::Pi { .. } => continue,
            _ => {
                body_start = i;
                break;
            }
        };
        match kind(child) {
            Some(Kind::Label) => head.label = Some(child.attr("value").unwrap_or("undefined")),
            Some(Kind::Thread) => {
                head.thread = child.attr("thread_id").and_then(|v| v.parse().ok())
            }
            Some(Kind::Xref) => head.xref = child.attr("thread_id").and_then(|v| v.parse().ok()),
            Some(Kind::Href) => head.href = child.attr("uri"),
            Some(Kind::Layer) => head.layer = child.attr("value").unwrap_or("body"),
            Some(Kind::Location) if locations.len() < 4 => {
                if let Some(value) = child.attr("value").and_then(|v| v.parse().ok()) {
                    locations.push(Coord {
                        value,
                        resolution: child.attr("resolution").and_then(|v| v.parse().ok()),
                    });
                }
            }
            Some(Kind::Caption) => head.caption = Some(child),
            Some(Kind::Description) => head.description = Some(child),
            Some(Kind::Summary) => head.summary = Some(child),
            Some(Kind::Custom) => head.custom = Some(child),
            _ => {
                body_start = i;
                break;
            }
        }
    }

    if let [x_min, y_min, x_max, y_max] = locations[..] {
        head.bounds = Some(Bounds {
            x_min,
            y_min,
            x_max,
            y_max,
        });
    }
    head.body = body_start..el.children().len();
    head
}

/// The document's `version` attribute, with the spec's default applied.
pub fn version(doc: &Document) -> &str {
    doc.root.attr("version").unwrap_or(crate::SPEC_VERSION)
}

/// Whether the root declares the DocLang namespace as its default.
pub fn namespaced(doc: &Document) -> bool {
    doc.root.attr("xmlns") == Some(crate::NAMESPACE)
}

/// The document head element, if the first element child of the root is one.
pub fn document_head(doc: &Document) -> Option<&Element> {
    doc.root
        .child_elements()
        .next()
        .filter(|e| kind(e) == Some(Kind::Head))
}

/// The `(width, height)` grid the document's locations are against, with
/// the spec's default for either axis that is not declared.
pub fn default_resolution(doc: &Document) -> (u32, u32) {
    let dr = document_head(doc).and_then(|h| {
        h.child_elements()
            .find(|e| kind(e) == Some(Kind::DefaultResolution))
    });
    let axis = |name: &str| {
        dr.and_then(|e| e.attr(name))
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RESOLUTION)
    };
    (axis("width"), axis("height"))
}

/// One page of the document body: a range over the root's children between
/// `page_break` elements, the document head excluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// One-based, matching `pages/N.png` in an archive.
    pub number: usize,
    pub children: Range<usize>,
}

/// The pages of a document, as the spec counts them: `page_break` elements
/// plus one. A document with no breaks is one page.
pub fn pages(doc: &Document) -> Vec<Page> {
    let children = doc.root.children();
    let mut start = match document_head(doc) {
        Some(h) => children
            .iter()
            .position(|n| matches!(n, Node::Element(e) if e.id() == h.id()))
            .map_or(0, |i| i + 1),
        None => 0,
    };
    let mut pages = Vec::new();
    for (i, node) in children.iter().enumerate() {
        if let Node::Element(e) = node {
            if kind(e) == Some(Kind::PageBreak) {
                pages.push(Page {
                    number: pages.len() + 1,
                    children: start..i,
                });
                start = i + 1;
            }
        }
    }
    pages.push(Page {
        number: pages.len() + 1,
        children: start..children.len(),
    });
    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_name() {
        for k in Kind::ALL {
            assert_eq!(Kind::from_name(k.name()), Some(*k));
        }
        assert_eq!(Kind::ALL.len(), 56);
        assert_eq!(Kind::from_name("acme_smiles"), None);
    }

    #[test]
    fn prefixed_names_are_not_vocabulary() {
        let doc = Document::parse("<custom xmlns:a=\"u\"><a:text>x</a:text></custom>").unwrap();
        let inner = doc.root.child_elements().next().unwrap();
        assert_eq!(kind(inner), None);
        assert_eq!(inner.local_name(), "text");
    }

    #[test]
    fn head_reads_properties_and_finds_the_body() {
        let doc = Document::parse(
            "<doclang><picture class=\"chart\">\n  <label value=\"bar_chart\"/>\n  <thread thread_id=\"3\"/>\n  <href uri=\"http://x\"/>\n  <layer value=\"furniture\"/>\n  <location value=\"10\"/><location value=\"20\"/><location value=\"30\" resolution=\"100\"/><location value=\"40\"/>\n  <caption>Cap</caption>\n  <src uri=\"a.png\"/>\n</picture></doclang>",
        )
        .unwrap();
        let pic = doc.root.child_elements().next().unwrap();
        let h = head(pic);
        assert_eq!(h.label, Some("bar_chart"));
        assert_eq!(h.thread, Some(3));
        assert_eq!(h.href, Some("http://x"));
        assert_eq!(h.layer, "furniture");
        let b = h.bounds.unwrap();
        assert_eq!(
            b.x_max,
            Coord {
                value: 30,
                resolution: Some(100)
            }
        );
        assert_eq!(
            b.fractions((512, 512)),
            [10.0 / 512.0, 20.0 / 512.0, 0.3, 40.0 / 512.0]
        );
        assert_eq!(h.caption.unwrap().text(), "Cap");
        let body: Vec<&Element> = pic.children()[h.body]
            .iter()
            .filter_map(|n| match n {
                Node::Element(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(body.len(), 1);
        assert_eq!(kind(body[0]), Some(Kind::Src));
    }

    #[test]
    fn head_defaults_and_headless_body() {
        let doc = Document::parse("<doclang><text>Plain</text></doclang>").unwrap();
        let t = doc.root.child_elements().next().unwrap();
        let h = head(t);
        assert_eq!(h.layer, "body");
        assert!(h.bounds.is_none());
        assert_eq!(h.body, 0..1);
    }

    #[test]
    fn document_head_resolution_and_pages() {
        let doc = Document::parse(
            "<doclang version=\"0.7\" xmlns=\"https://www.doclang.ai/ns/v0\"><head><default_resolution width=\"1024\"/></head><text>a</text><page_break/><text>b</text><page_break/></doclang>",
        )
        .unwrap();
        assert_eq!(version(&doc), "0.7");
        assert!(namespaced(&doc));
        assert_eq!(default_resolution(&doc), (1024, 512));
        let p = pages(&doc);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].children, 1..2);
        assert_eq!(p[1].children, 3..4);
        assert_eq!(p[2].children, 5..5);
        assert_eq!(p[2].number, 3);
    }

    #[test]
    fn a_bare_document_is_one_page_at_default_resolution() {
        let doc = Document::parse("<doclang/>").unwrap();
        assert_eq!(default_resolution(&doc), (512, 512));
        assert_eq!(
            pages(&doc),
            [Page {
                number: 1,
                children: 0..0
            }]
        );
        assert!(!namespaced(&doc));
    }
}
