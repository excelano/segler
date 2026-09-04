//! Validation against the specification, in two layers that mirror the
//! reference toolkit's two validators.
//!
//! The **schema** layer is the XSD, ported by hand into content-model checks
//! over the tree: which elements may appear where and in what order, which
//! attributes they take, what values those attributes may hold, and whether
//! text is allowed between them. The **rules** layer is the Schematron,
//! thirteen patterns ported one to one, each finding carrying the pattern's
//! id so that it can be read next to the toolkit's own output.
//!
//! Both layers are per specification version, and a spec bump is a diff
//! here against a diff in `doclang.xsd` and `doclang.sch`. Agreement with
//! the toolkit is measured by `segler corpus --toolkit`, never assumed.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::HashMap;
use std::fmt;

use crate::doclang::{self, Kind};
use crate::tree::{Document, Element, ElementId, Node};

/// Which validator a finding came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// The XSD content models and attribute types.
    Schema,
    /// The Schematron patterns.
    Rules,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Layer::Schema => "schema",
            Layer::Rules => "rules",
        })
    }
}

/// One problem with a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub layer: Layer,
    /// The Schematron pattern id, or `xsd` for the schema layer.
    pub rule: &'static str,
    /// The element the problem is on.
    pub element: ElementId,
    /// Where that element is, as `/doclang/text[2]/xref[1]`.
    pub path: String,
    pub message: String,
}

/// Every finding in `doc`, schema layer first, in document order within a
/// layer. An empty result is a valid document.
pub fn validate(doc: &Document) -> Vec<Finding> {
    let mut findings = Vec::new();
    schema::check(doc, &mut findings);
    rules::check(doc, &mut findings);
    findings
}

// ---------------------------------------------------------------------------
// Shared vocabulary predicates, named after the XSD groups they stand for.

fn kind(el: &Element) -> Option<Kind> {
    doclang::kind(el)
}

fn is_head_kind(k: Kind) -> bool {
    matches!(
        k,
        Kind::Label
            | Kind::Thread
            | Kind::Xref
            | Kind::Href
            | Kind::Layer
            | Kind::Location
            | Kind::Caption
            | Kind::Description
            | Kind::Summary
            | Kind::Custom
    )
}

/// The tokens that begin a cell: every OTSL marker except `nl`.
fn is_cell_start(k: Kind) -> bool {
    matches!(
        k,
        Kind::Fcel
            | Kind::Ecel
            | Kind::Ched
            | Kind::Rhed
            | Kind::Corn
            | Kind::Srow
            | Kind::Lcel
            | Kind::Ucel
            | Kind::Xcel
    )
}

fn is_table_sep(k: Kind) -> bool {
    is_cell_start(k) || k == Kind::Nl
}

fn is_formatting(k: Kind) -> bool {
    k.category() == doclang::Category::Formatting
}

fn is_headless_txt(k: Kind) -> bool {
    is_formatting(k) || matches!(k, Kind::Marker | Kind::Hint | Kind::Checkbox)
}

fn is_headless(k: Kind) -> bool {
    is_headless_txt(k)
        || matches!(
            k,
            Kind::Code
                | Kind::Formula
                | Kind::Picture
                | Kind::FieldRegion
                | Kind::FieldHeading
                | Kind::FieldItem
                | Kind::Key
                | Kind::Value
        )
}

fn is_top_level(k: Kind) -> bool {
    matches!(
        k,
        Kind::Text
            | Kind::Heading
            | Kind::Code
            | Kind::Formula
            | Kind::PageHeader
            | Kind::PageFooter
            | Kind::Footnote
            | Kind::List
            | Kind::Group
            | Kind::FieldRegion
            | Kind::FieldHeading
            | Kind::FieldItem
            | Kind::Key
            | Kind::Value
            | Kind::Picture
            | Kind::Table
            | Kind::Index
    )
}

/// `virtual_text_content`: what may follow a list divider or a cell token.
fn is_vtc(k: Kind) -> bool {
    k == Kind::Content || is_headless_txt(k) || is_top_level(k)
}

fn is_whitespace(node: &Node) -> bool {
    match node {
        Node::Text(t) => t.value().trim().is_empty(),
        Node::CData(s) => s.trim().is_empty(),
        _ => true,
    }
}

/// Element children with their index into `children()`.
fn elements(el: &Element) -> Vec<(usize, &Element)> {
    el.children()
        .iter()
        .enumerate()
        .filter_map(|(i, n)| match n {
            Node::Element(e) => Some((i, e)),
            _ => None,
        })
        .collect()
}

/// Walk every element with its path, parent, and the kinds of its ancestors.
struct Visit<'a, 'b> {
    el: &'a Element,
    parent: Option<&'a Element>,
    path: &'b str,
    ancestors: &'b [Kind],
}

fn walk<'a>(
    el: &'a Element,
    parent: Option<&'a Element>,
    path: &str,
    ancestors: &mut Vec<Kind>,
    f: &mut impl FnMut(Visit<'a, '_>),
) {
    f(Visit {
        el,
        parent,
        path,
        ancestors,
    });
    let pushed = match kind(el) {
        Some(k) => {
            ancestors.push(k);
            true
        }
        None => false,
    };
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for child in el.child_elements() {
        let n = seen.entry(child.name()).or_insert(0);
        *n += 1;
        let child_path = format!("{path}/{}[{n}]", child.name());
        walk(child, Some(el), &child_path, ancestors, f);
    }
    if pushed {
        ancestors.pop();
    }
}

fn walk_document<'a>(doc: &'a Document, f: &mut impl FnMut(Visit<'a, '_>)) {
    let path = format!("/{}", doc.root.name());
    walk(&doc.root, None, &path, &mut Vec::new(), f);
}

// ---------------------------------------------------------------------------
// The schema layer

mod schema {
    use super::*;

    const RULE: &str = "xsd";

    /// How an element's content is shaped, from the XSD's complex types.
    #[derive(Clone, Copy)]
    enum Body {
        /// No element children.
        Empty,
        /// Character data only: the `content` element.
        TextOnly,
        /// Anything, validated where a declaration exists: `head`, `custom`.
        Any,
        /// A repeated choice of the kinds the predicate admits.
        Choice(fn(Kind) -> bool),
        /// `head? (top_level | page_break)*`: the root.
        Root,
        /// `ldiv (head? vtc*)`, repeated: `list`.
        Items,
        /// `table_sep (head? vtc*)`, repeated: `table`, `index`, `tabular`.
        Cells,
        /// `src? tabular? vtc*`: `picture`.
        Picture,
        /// `marker?`: `ldiv`.
        Divider,
    }

    struct Model {
        /// Whether character data may appear between children.
        mixed: bool,
        /// Whether an element head may open the content.
        head: bool,
        body: Body,
    }

    fn model(k: Kind) -> Model {
        use Kind::*;
        let m = |mixed, head, body| Model { mixed, head, body };
        match k {
            Doclang => m(false, false, Body::Root),
            Head => m(true, false, Body::Any),
            Custom => m(false, false, Body::Any),
            PageBreak | Checkbox | DefaultResolution | Location | Label | Thread | Xref | Href
            | Layer | Src | Fcel | Ecel | Ched | Rhed | Corn | Srow | Lcel | Ucel | Xcel | Nl => {
                m(false, false, Body::Empty)
            }
            Content => m(true, false, Body::TextOnly),
            Description | Summary => m(true, false, Body::Choice(|c| c == Content)),
            Bold | Italic | Underline | Strikethrough | Superscript | Subscript | Handwriting
            | Rtl | Hint => m(
                true,
                false,
                Body::Choice(|c| c == Content || is_formatting(c)),
            ),
            Marker => m(
                true,
                true,
                Body::Choice(|c| c == Content || is_formatting(c) || c == Checkbox),
            ),
            Text | Heading | Caption | PageHeader | PageFooter | Footnote | Key | Value => m(
                true,
                true,
                Body::Choice(|c| c == Content || is_headless_txt(c) || is_top_level(c)),
            ),
            Code | Formula => m(true, true, Body::Choice(|c| c == Content || is_headless(c))),
            FieldRegion | FieldHeading | FieldItem => m(
                true,
                true,
                Body::Choice(|c| is_headless_txt(c) || is_top_level(c)),
            ),
            Group => m(false, true, Body::Choice(is_top_level)),
            List => m(true, true, Body::Items),
            Table | Index => m(true, true, Body::Cells),
            Tabular => m(true, false, Body::Cells),
            Picture => m(true, true, Body::Picture),
            Ldiv => m(false, false, Body::Divider),
        }
    }

    /// The attributes an element declares, with their value constraint.
    enum Attr {
        Enum(&'static [&'static str]),
        NonNegative,
        Positive,
        Any,
    }

    fn attributes(k: Kind) -> &'static [(&'static str, bool, Attr)] {
        use Attr::*;
        use Kind::*;
        match k {
            Doclang => &[("version", false, Enum(&["0.7"]))],
            Heading | FieldHeading => &[("level", false, Positive)],
            List => &[("class", false, Enum(&["ordered", "unordered"]))],
            Picture => &[("class", false, Enum(&["undefined", "chart"]))],
            Value => &[("class", false, Enum(&["read_only", "fillable"]))],
            Checkbox => &[("class", false, Enum(&["unselected", "selected"]))],
            Layer => &[("value", false, Enum(&["body", "background", "furniture"]))],
            Label => &[("value", false, Any)],
            Thread | Xref => &[("thread_id", true, Positive)],
            Href | Src => &[("uri", true, Any)],
            Location => &[
                ("value", true, NonNegative),
                ("resolution", false, Positive),
            ],
            DefaultResolution => &[
                ("width", false, NonNegative),
                ("height", false, NonNegative),
            ],
            _ => &[],
        }
    }

    fn is_integer(s: &str) -> Option<i128> {
        let s = s.trim();
        let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse::<i128>().ok()
    }

    pub(super) fn check(doc: &Document, out: &mut Vec<Finding>) {
        let mut push = |el: &Element, path: &str, message: String| {
            out.push(Finding {
                layer: Layer::Schema,
                rule: RULE,
                element: el.id(),
                path: path.to_owned(),
                message,
            })
        };

        let root = &doc.root;
        match root.attr("xmlns") {
            None | Some(crate::NAMESPACE) => {}
            Some(other) => push(
                root,
                &format!("/{}", root.name()),
                format!(
                    "root element is in namespace {other:?}, not {}",
                    crate::NAMESPACE
                ),
            ),
        }
        if kind(root) != Some(Kind::Doclang) {
            push(
                root,
                &format!("/{}", root.name()),
                format!("root element is <{}>, not <doclang>", root.name()),
            );
            return;
        }

        walk_document(doc, &mut |v| {
            let Some(k) = kind(v.el) else {
                return;
            };
            check_attributes(v.el, k, v.path, &mut push);
            check_content(v.el, k, v.path, &mut push);
        });
    }

    fn check_attributes(
        el: &Element,
        k: Kind,
        path: &str,
        push: &mut impl FnMut(&Element, &str, String),
    ) {
        let declared = attributes(k);
        for a in el.attrs() {
            if a.name == "xmlns" || a.name.starts_with("xmlns:") || a.name.starts_with("xml:") {
                continue;
            }
            match declared.iter().find(|(name, ..)| *name == a.name) {
                None => push(
                    el,
                    path,
                    format!("<{}> does not take a {} attribute", el.name(), a.name),
                ),
                Some((name, _, constraint)) => {
                    let ok = match constraint {
                        Attr::Any => true,
                        Attr::Enum(values) => values.contains(&a.value.as_str()),
                        Attr::NonNegative => is_integer(&a.value).is_some_and(|n| n >= 0),
                        Attr::Positive => is_integer(&a.value).is_some_and(|n| n > 0),
                    };
                    if !ok {
                        let expected = match constraint {
                            Attr::Enum(values) => format!("one of {}", values.join(", ")),
                            Attr::NonNegative => "a non-negative integer".to_owned(),
                            Attr::Positive => "a positive integer".to_owned(),
                            Attr::Any => unreachable!(),
                        };
                        push(
                            el,
                            path,
                            format!("{name}={:?} on <{}> is not {expected}", a.value, el.name()),
                        );
                    }
                }
            }
        }
        for (name, required, _) in declared {
            if *required && el.attr(name).is_none() {
                push(
                    el,
                    path,
                    format!("<{}> requires a {name} attribute", el.name()),
                );
            }
        }
    }

    fn check_content(
        el: &Element,
        k: Kind,
        path: &str,
        push: &mut impl FnMut(&Element, &str, String),
    ) {
        let model = model(k);
        if !model.mixed {
            if let Some(text) = el.children().iter().find(|n| !is_whitespace(n)) {
                let shown: String = match text {
                    Node::Text(t) => t.value().trim().chars().take(30).collect(),
                    Node::CData(s) => s.trim().chars().take(30).collect(),
                    _ => String::new(),
                };
                push(
                    el,
                    path,
                    format!("<{}> does not allow text content ({shown:?})", el.name()),
                );
            }
        }

        let children = elements(el);
        let mut i = 0;
        if model.head {
            i = match_head(el, &children, 0, path, push);
        }

        match model.body {
            Body::Empty | Body::TextOnly => {
                if let Some((_, c)) = children.first() {
                    push(
                        el,
                        path,
                        format!(
                            "<{}> is an empty element; <{}> is not allowed inside it",
                            el.name(),
                            c.name()
                        ),
                    );
                }
            }
            Body::Any => {}
            Body::Choice(allowed) => {
                for (_, c) in &children[i..] {
                    reject_unless(el, c, path, allowed, push);
                }
            }
            Body::Divider => {
                for (n, (_, c)) in children.iter().enumerate() {
                    if n > 0 || kind(c) != Some(Kind::Marker) {
                        push(
                            el,
                            path,
                            format!(
                                "<ldiv> may hold one <marker> and nothing else; found <{}>",
                                c.name()
                            ),
                        );
                    }
                }
            }
            Body::Root => {
                for (n, (_, c)) in children.iter().enumerate() {
                    match kind(c) {
                        Some(Kind::Head) if n == 0 => {}
                        Some(Kind::Head) => push(
                            el,
                            path,
                            "<head> must be the first child of <doclang>".to_owned(),
                        ),
                        Some(Kind::PageBreak) => {}
                        _ => reject_unless(el, c, path, is_top_level, push),
                    }
                }
            }
            Body::Items => {
                if let Some((_, c)) = children.get(i) {
                    if kind(c) != Some(Kind::Ldiv) {
                        push(
                            el,
                            path,
                            format!("a list item begins with <ldiv>; found <{}>", c.name()),
                        );
                    }
                }
                let mut j = i;
                while j < children.len() {
                    let (_, c) = children[j];
                    if kind(c) == Some(Kind::Ldiv) {
                        j = match_head(el, &children, j + 1, path, push);
                        continue;
                    }
                    reject_unless(el, c, path, is_vtc, push);
                    j += 1;
                }
            }
            Body::Cells => {
                if let Some((_, c)) = children.get(i) {
                    if !kind(c).is_some_and(is_table_sep) {
                        push(
                            el,
                            path,
                            format!("a cell begins with an OTSL token; found <{}>", c.name()),
                        );
                    }
                }
                let mut j = i;
                while j < children.len() {
                    let (_, c) = children[j];
                    if kind(c).is_some_and(is_table_sep) {
                        j = match_head(el, &children, j + 1, path, push);
                        continue;
                    }
                    reject_unless(el, c, path, is_vtc, push);
                    j += 1;
                }
            }
            Body::Picture => {
                let mut j = i;
                if let Some((_, c)) = children.get(j) {
                    if kind(c) == Some(Kind::Src) {
                        j += 1;
                    }
                }
                if let Some((_, c)) = children.get(j) {
                    if kind(c) == Some(Kind::Tabular) {
                        j += 1;
                    }
                }
                for (_, c) in &children[j..] {
                    match kind(c) {
                        Some(Kind::Src) => push(
                            el,
                            path,
                            "<src> must come first in the picture body".to_owned(),
                        ),
                        Some(Kind::Tabular) => push(
                            el,
                            path,
                            "<tabular> must directly follow <src>, or come first without one"
                                .to_owned(),
                        ),
                        _ => reject_unless(el, c, path, is_vtc, push),
                    }
                }
            }
        }
    }

    /// Consume an element head starting at `start`, in the XSD's order, and
    /// return the index of the first body element. A head element out of
    /// order or a location block of the wrong size is reported.
    fn match_head(
        parent: &Element,
        children: &[(usize, &Element)],
        start: usize,
        path: &str,
        push: &mut impl FnMut(&Element, &str, String),
    ) -> usize {
        // Positions in the head sequence; a kind may only appear at or after
        // the position that follows the last one seen.
        fn slot(k: Kind) -> Option<u8> {
            Some(match k {
                Kind::Label => 0,
                Kind::Thread => 1,
                Kind::Xref | Kind::Href => 2,
                Kind::Layer => 3,
                Kind::Location => 4,
                Kind::Caption => 5,
                Kind::Description => 6,
                Kind::Summary => 7,
                Kind::Custom => 8,
                _ => return None,
            })
        }
        let mut next_slot = 0u8;
        let mut locations = 0usize;
        let mut i = start;
        while i < children.len() {
            let (_, c) = children[i];
            let Some(s) = kind(c).and_then(slot) else {
                break;
            };
            if s == 4 && next_slot <= 4 {
                locations += 1;
                next_slot = 4;
            } else if s < next_slot {
                push(
                    parent,
                    path,
                    format!(
                        "<{}> is out of order in the element head of <{}>",
                        c.name(),
                        parent.name()
                    ),
                );
            } else {
                next_slot = s + 1;
            }
            i += 1;
        }
        if locations != 0 && locations != 4 {
            push(
                parent,
                path,
                format!(
                    "<{}> has {locations} <location> elements; a location block is exactly four",
                    parent.name()
                ),
            );
        }
        i
    }

    fn reject_unless(
        parent: &Element,
        c: &Element,
        path: &str,
        allowed: fn(Kind) -> bool,
        push: &mut impl FnMut(&Element, &str, String),
    ) {
        match kind(c) {
            Some(k) if allowed(k) => {}
            Some(k) if is_head_kind(k) => push(
                parent,
                path,
                format!(
                    "<{}> belongs in the element head of <{}>, before any content",
                    c.name(),
                    parent.name()
                ),
            ),
            Some(_) => push(
                parent,
                path,
                format!("<{}> is not allowed in <{}>", c.name(), parent.name()),
            ),
            None => push(
                parent,
                path,
                format!("<{}> is not a DocLang element", c.name()),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// The rules layer

mod rules {
    use super::*;

    pub(super) fn check(doc: &Document, out: &mut Vec<Finding>) {
        if kind(&doc.root) != Some(Kind::Doclang) {
            return;
        }
        let (width, height) = default_resolution_as_numbers(doc);
        let threads: Vec<(String, String)> = thread_hosts(doc);

        walk_document(doc, &mut |v| {
            let Some(k) = kind(v.el) else {
                return;
            };
            let mut push = |rule: &'static str, message: String| {
                out.push(Finding {
                    layer: Layer::Rules,
                    rule,
                    element: v.el.id(),
                    path: v.path.to_owned(),
                    message,
                })
            };
            let children = elements(v.el);
            let first_body = children
                .iter()
                .find(|(_, c)| !kind(c).is_some_and(is_head_kind))
                .map(|(i, c)| (*i, *c));

            // list-structure
            if k == Kind::List {
                if let Some((_, c)) = first_body {
                    if kind(c) != Some(Kind::Ldiv) {
                        push("list-structure", format!("list must have ldiv as first element after the optional element head; found {}", c.name()));
                    }
                }
            }

            // table-structure
            if matches!(k, Kind::Table | Kind::Index) {
                if let Some((_, c)) = first_body {
                    if !kind(c).is_some_and(is_cell_start) {
                        push("table-structure", format!("table and index must have a cell-starting token as first element after the optional element head; found {}", c.name()));
                    }
                }
            }

            // table-rectangular-grid
            if matches!(k, Kind::Table | Kind::Index)
                && children.iter().any(|(_, c)| kind(c) == Some(Kind::Nl))
            {
                let mut rows: Vec<usize> = Vec::new();
                let mut count = 0;
                for (_, c) in &children {
                    match kind(c) {
                        Some(Kind::Nl) => {
                            rows.push(count);
                            count = 0;
                        }
                        Some(ck) if is_cell_start(ck) => count += 1,
                        _ => {}
                    }
                }
                if rows.iter().any(|n| *n != rows[0]) {
                    push("table-rectangular-grid", format!("table and index must follow the rectangular rule: all rows must have the same number of cells; first row has {} cells, but at least one other row has a different count", rows[0]));
                }
            }

            // element-head-placement
            if matches!(
                k,
                Kind::Text
                    | Kind::Heading
                    | Kind::Code
                    | Kind::Formula
                    | Kind::Caption
                    | Kind::Description
                    | Kind::Summary
                    | Kind::PageHeader
                    | Kind::PageFooter
                    | Kind::Footnote
                    | Kind::Picture
                    | Kind::Marker
                    | Kind::FieldRegion
                    | Kind::FieldHeading
                    | Kind::FieldItem
                    | Kind::Key
                    | Kind::Value
                    | Kind::List
                    | Kind::Table
                    | Kind::Index
                    | Kind::Group
            ) {
                // For a list, table or index only its own head counts: text
                // in an earlier item or cell followed by a head element in a
                // later one is that item's business, and the two virtual-text
                // patterns below judge it. The Schematron's XPath reads wider
                // than that, but it never ran on those elements in the
                // reference toolkit (DESIGN.md §6), and the corpus marks such
                // documents valid.
                let own = match k {
                    Kind::List => children
                        .iter()
                        .find(|(_, c)| kind(c) == Some(Kind::Ldiv))
                        .map_or(v.el.children().len(), |(i, _)| *i),
                    Kind::Table | Kind::Index => children
                        .iter()
                        .find(|(_, c)| kind(c).is_some_and(is_table_sep))
                        .map_or(v.el.children().len(), |(i, _)| *i),
                    _ => v.el.children().len(),
                };
                let last_head = children
                    .iter()
                    .rev()
                    .find(|(i, c)| *i < own && kind(c).is_some_and(is_head_kind))
                    .map(|(i, _)| *i);
                if let Some(last) = last_head {
                    let stray: Vec<String> = v.el.children()[..last]
                        .iter()
                        .filter_map(|n| match n {
                            Node::Text(t) if !t.value().trim().is_empty() => {
                                Some(t.value().trim().to_owned())
                            }
                            _ => None,
                        })
                        .collect();
                    if !stray.is_empty() {
                        push("element-head-placement", format!("property elements in the element head must appear before any non-whitespace text content; found text before element head: {:?}", stray.join("")));
                    }
                }
            }

            // xref-href-mutual-exclusivity
            let has = |want: Kind| children.iter().any(|(_, c)| kind(c) == Some(want));
            if has(Kind::Xref) && has(Kind::Href) {
                push("xref-href-mutual-exclusivity", "element head must not contain both xref and href elements; they are mutually exclusive".to_owned());
            }

            // xref-thread-defined
            if k == Kind::Xref {
                let id = v.el.attr("thread_id").unwrap_or("");
                if !threads.iter().any(|(tid, _)| tid == id) {
                    push("xref-thread-defined", format!("xref references thread_id={id:?} but no thread element defines that id"));
                }
            }

            // location-value-range
            if k == Kind::Location {
                let siblings = v.parent.map(elements).unwrap_or_default();
                let index = siblings
                    .iter()
                    .take_while(|(_, c)| c.id() != v.el.id())
                    .filter(|(_, c)| kind(c) == Some(Kind::Location))
                    .count()
                    + 1;
                let x_axis = index % 2 == 1;
                let limit = match v.el.attr("resolution") {
                    Some(r) => number(r),
                    None if x_axis => width,
                    None => height,
                };
                let value = number(v.el.attr("value").unwrap_or(""));
                if !(value >= 0.0 && value < limit) {
                    push("location-value-range", format!("location value must satisfy 0 <= value < axis_limit; found value={}, axis_limit={limit}, axis={}, location-index={index}", v.el.attr("value").unwrap_or(""), if x_axis { "x" } else { "y" }));
                }
            }

            // location-block-order
            if has(Kind::Location) {
                let locs: Vec<&Element> = children
                    .iter()
                    .filter(|(_, c)| kind(c) == Some(Kind::Location))
                    .map(|(_, c)| *c)
                    .collect();
                let coord = |i: usize, default: f64| -> f64 {
                    let Some(l) = locs.get(i) else {
                        return f64::NAN;
                    };
                    let res = l.attr("resolution").map_or(default, number);
                    number(l.attr("value").unwrap_or("")) / res
                };
                let (x0, y0, x1, y1) = (
                    coord(0, width),
                    coord(1, height),
                    coord(2, width),
                    coord(3, height),
                );
                if !(x0 <= x1 && y0 <= y1) {
                    push("location-block-order", "location block must satisfy x0_norm <= x1_norm and y0_norm <= y1_norm, where *_norm is each coordinate normalized by its effective resolution".to_owned());
                }
            }

            // thread-host-type-consistency
            if k == Kind::Doclang {
                let mut by_id: HashMap<&str, Vec<&str>> = HashMap::new();
                for (id, host) in &threads {
                    let hosts = by_id.entry(id).or_default();
                    if !hosts.contains(&host.as_str()) {
                        hosts.push(host);
                    }
                }
                let mut mixed: Vec<&str> = by_id
                    .iter()
                    .filter(|(_, h)| h.len() > 1)
                    .map(|(id, _)| *id)
                    .collect();
                mixed.sort_unstable();
                if !mixed.is_empty() {
                    push("thread-host-type-consistency", format!("all thread elements with the same thread_id must use the same host element type; thread_id {} mixes types", mixed.join(", ")));
                }
            }

            // list-virtual-text-element-head
            if k == Kind::List {
                for (n, (start, c)) in children.iter().enumerate() {
                    if kind(c) != Some(Kind::Ldiv) {
                        continue;
                    }
                    let end = children[n + 1..]
                        .iter()
                        .find(|(_, d)| kind(d) == Some(Kind::Ldiv))
                        .map_or(v.el.children().len(), |(i, _)| *i);
                    if let Some(text) = text_before_head(&v.el.children()[start + 1..end]) {
                        push("list-virtual-text-element-head", format!("in list items (virtual text), property elements in the element head must appear before any non-whitespace text content; found text before element head: {text:?}"));
                    }
                }
            }

            // table-virtual-text-element-head
            if matches!(k, Kind::Table | Kind::Index) {
                for (n, (start, c)) in children.iter().enumerate() {
                    if !kind(c).is_some_and(is_cell_start) {
                        continue;
                    }
                    let end = children[n + 1..]
                        .iter()
                        .find(|(_, d)| kind(d).is_some_and(is_table_sep))
                        .map_or(v.el.children().len(), |(i, _)| *i);
                    if let Some(text) = text_before_head(&v.el.children()[start + 1..end]) {
                        push("table-virtual-text-element-head", format!("in table and index cells (virtual text), property elements in the element head must appear before any non-whitespace text content; found text before element head: {text:?}"));
                    }
                }
            }

            // field-structure-placement
            match k {
                Kind::FieldHeading | Kind::FieldItem
                    if !v.ancestors.contains(&Kind::FieldRegion) =>
                {
                    push(
                        "field-structure-placement",
                        "field_heading and field_item must be descendants of field_region"
                            .to_owned(),
                    );
                }
                Kind::Key | Kind::Value if !v.ancestors.contains(&Kind::FieldItem) => {
                    push(
                        "field-structure-placement",
                        "key and value must be descendants of field_item".to_owned(),
                    );
                }
                _ => {}
            }
            if k == Kind::FieldItem {
                // Keys under this item that have exactly one field_item
                // ancestor in the whole document, as the XPath counts them.
                let above = v
                    .ancestors
                    .iter()
                    .filter(|a| **a == Kind::FieldItem)
                    .count();
                let own = count_keys(v.el, above + 1);
                if own > 1 {
                    push("field-structure-placement", format!("a field_item may contain at most one own descendant key; found own-key-count={own}"));
                }
            }

            // picture-body
            if k == Kind::Picture {
                let first = first_body.map(|(_, c)| c.id());
                let src = children
                    .iter()
                    .find(|(_, c)| kind(c) == Some(Kind::Src))
                    .map(|(i, c)| (*i, c.id()));
                let tabular = children
                    .iter()
                    .find(|(_, c)| kind(c) == Some(Kind::Tabular))
                    .map(|(i, c)| (*i, c.id()));
                if tabular.is_some() && v.el.attr("class") != Some("chart") {
                    push(
                        "picture-body",
                        "element tabular is only allowed in picture with class=\"chart\""
                            .to_owned(),
                    );
                }
                if let Some((_, id)) = src {
                    if first != Some(id) {
                        push("picture-body", "element src must be the first element of the picture body when present".to_owned());
                    }
                }
                if let Some((ti, tid)) = tabular {
                    let ok = match src {
                        None => first == Some(tid),
                        Some((si, _)) => {
                            children.iter().position(|(i, _)| *i == ti)
                                == children.iter().position(|(i, _)| *i == si).map(|p| p + 1)
                        }
                    };
                    if !ok {
                        push("picture-body", "element tabular must immediately follow src when src is present, otherwise it may be the first body element".to_owned());
                    }
                }
            }
        });
    }

    /// XPath `number()`: a numeric string or NaN.
    fn number(s: &str) -> f64 {
        s.trim().parse::<f64>().unwrap_or(f64::NAN)
    }

    fn default_resolution_as_numbers(doc: &Document) -> (f64, f64) {
        let dr = doclang::document_head(doc).and_then(|h| {
            h.child_elements()
                .find(|e| kind(e) == Some(Kind::DefaultResolution))
        });
        let axis = |name: &str| dr.and_then(|e| e.attr(name)).map_or(512.0, number);
        (axis("width"), axis("height"))
    }

    /// Every thread element's id with its host type, as the Schematron
    /// classifies hosts: a list item or a table cell when the thread sits
    /// after a divider or cell token, otherwise the parent's name.
    fn thread_hosts(doc: &Document) -> Vec<(String, String)> {
        let mut out = Vec::new();
        walk_document(doc, &mut |v| {
            if kind(v.el) != Some(Kind::Thread) {
                return;
            }
            let Some(parent) = v.parent else { return };
            let before: Vec<Kind> = elements(parent)
                .iter()
                .take_while(|(_, c)| c.id() != v.el.id())
                .filter_map(|(_, c)| kind(c))
                .collect();
            let host = match kind(parent) {
                Some(Kind::List) if before.contains(&Kind::Ldiv) => "list-item".to_owned(),
                Some(Kind::Table | Kind::Index) if before.iter().any(|k| is_cell_start(*k)) => {
                    "table-cell".to_owned()
                }
                _ => parent.local_name().to_owned(),
            };
            out.push((v.el.attr("thread_id").unwrap_or("").to_owned(), host));
        });
        out
    }

    /// Non-whitespace text that comes before the first head element in a
    /// run of nodes, if there is a head element to come before.
    fn text_before_head(nodes: &[Node]) -> Option<String> {
        let first_head = nodes
            .iter()
            .position(|n| matches!(n, Node::Element(e) if kind(e).is_some_and(is_head_kind)))?;
        let text: String = nodes[..first_head]
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) if !t.value().trim().is_empty() => Some(t.value().trim()),
                _ => None,
            })
            .collect();
        (!text.is_empty()).then_some(text)
    }

    /// Descendant `key` elements whose number of `field_item` ancestors in
    /// the document equals `target`. `depth` is how many field_item
    /// ancestors the current element has, itself included.
    fn count_keys(el: &Element, depth_here: usize) -> usize {
        fn go(el: &Element, depth: usize, target: usize) -> usize {
            let mut n = 0;
            for c in el.child_elements() {
                let d = depth + usize::from(kind(c) == Some(Kind::FieldItem));
                if kind(c) == Some(Kind::Key) && d == target {
                    n += 1;
                }
                n += go(c, d, target);
            }
            n
        }
        go(el, depth_here, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn findings(src: &str) -> Vec<(Layer, &'static str)> {
        validate(&Document::parse(src).unwrap())
            .into_iter()
            .map(|f| (f.layer, f.rule))
            .collect()
    }

    #[test]
    fn a_valid_document_has_no_findings() {
        let src = r#"<doclang xmlns="https://www.doclang.ai/ns/v0" version="0.7"><head><default_resolution width="1000"/></head>
<heading level="2"><location value="1"/><location value="2"/><location value="3"/><location value="4"/>T</heading>
<list class="ordered"><ldiv/><label value="x"/>one<ldiv/><text>two</text></list>
<table><ched/>A<ched/>B<nl/><fcel/>1<fcel/>2<nl/></table>
<picture class="chart"><src uri="a.png"/><tabular><fcel/>1<nl/></tabular></picture>
<page_break/><text><thread thread_id="1"/>a</text><text><xref thread_id="1"/>b</text></doclang>"#;
        assert_eq!(findings(src), []);
    }

    #[test]
    fn schema_findings() {
        assert_eq!(
            findings("<doclang version=\"0.6\"/>"),
            [(Layer::Schema, "xsd")]
        );
        assert_eq!(findings("<doclang><text><layer value=\"body\"/><location value=\"1\"/><location value=\"2\"/><location value=\"3\"/><location value=\"4\"/><label/>x</text></doclang>"), [(Layer::Schema, "xsd")]);
        assert_eq!(findings("<doclang><custom>raw</custom></doclang>").len(), 2);
        assert_eq!(
            findings("<doclang><heading level=\"0\">x</heading></doclang>"),
            [(Layer::Schema, "xsd")]
        );
        assert_eq!(
            findings("<doclang><text>a<br/>b</text></doclang>"),
            [(Layer::Schema, "xsd")]
        );
        assert_eq!(
            findings("<doclang><description><bold>x</bold></description></doclang>").len(),
            2
        );
    }

    #[test]
    fn rule_findings() {
        assert!(
            findings("<doclang><table><ched/>a<ched/>b<nl/><fcel/>1<nl/></table></doclang>")
                .contains(&(Layer::Rules, "table-rectangular-grid"))
        );
        assert!(
            findings("<doclang><text><xref thread_id=\"9\"/>x</text></doclang>")
                .contains(&(Layer::Rules, "xref-thread-defined"))
        );
        assert!(findings("<doclang><text><location value=\"512\"/><location value=\"0\"/><location value=\"0\"/><location value=\"0\"/>x</text></doclang>").contains(&(Layer::Rules, "location-value-range")));
        assert!(findings("<doclang><text><location value=\"5\"/><location value=\"0\"/><location value=\"1\"/><location value=\"0\"/>x</text></doclang>").contains(&(Layer::Rules, "location-block-order")));
        assert!(findings("<doclang><key>x</key></doclang>")
            .contains(&(Layer::Rules, "field-structure-placement")));
        assert!(findings("<doclang><picture><tabular/></picture></doclang>")
            .contains(&(Layer::Rules, "picture-body")));
        assert!(findings("<doclang><text><thread thread_id=\"1\"/>a</text><picture><thread thread_id=\"1\"/></picture></doclang>").contains(&(Layer::Rules, "thread-host-type-consistency")));
        assert!(
            findings("<doclang><list><ldiv/>text<label/></list></doclang>")
                .contains(&(Layer::Rules, "list-virtual-text-element-head"))
        );
    }
}
