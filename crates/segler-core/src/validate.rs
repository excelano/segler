//! Validation against the specification, in two layers that mirror the
//! reference toolkit's two validators.
//!
//! The **schema** layer is the XSD, ported by hand into content-model checks
//! over the tree: which elements may appear where and in what order, which
//! attributes they take, what values those attributes may hold, and whether
//! text is allowed between them. The **rules** layer is the Schematron,
//! nineteen patterns ported one to one, each finding carrying the pattern's
//! id so that it can be read next to the toolkit's own output.
//!
//! Both layers are the current specification version's, [`crate::SPEC_VERSION`],
//! and a spec bump is a diff here against a diff in `doclang.xsd` and
//! `doclang.sch`. A document declaring another version, or none, is checked
//! against this one and carries a [`Layer::Version`] warning saying so.
//! Agreement with the toolkit is measured by `segler corpus --toolkit`, never
//! assumed.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::cmp::Ordering;
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
    /// The declared version differs from the one checked against, or is
    /// missing. A warning: a document with only these findings is valid.
    Version,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Layer::Schema => "schema",
            Layer::Rules => "rules",
            Layer::Version => "version",
        })
    }
}

/// One problem with a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub layer: Layer,
    /// The Schematron pattern id, `xsd` for the schema layer, or `missing`
    /// or `other` for the version.
    pub rule: &'static str,
    /// The element the problem is on.
    pub element: ElementId,
    /// Where that element is, as `/doclang/text[2]/xref[1]`.
    pub path: String,
    pub message: String,
}

impl Finding {
    /// Whether this finding makes the document invalid, as opposed to a
    /// warning about it.
    pub fn is_defect(&self) -> bool {
        self.layer != Layer::Version
    }
}

/// Every finding in `doc`: the version warning first, then the schema
/// layer, then the rules, in document order within a layer. A result with no
/// [`Finding::is_defect`] finding is a valid document.
pub fn validate(doc: &Document) -> Vec<Finding> {
    let mut findings = Vec::new();
    version_check(doc, &mut findings);
    schema::check(doc, &mut findings);
    rules::check(doc, &mut findings);
    findings
}

pub fn recordings(doc: &Document, recordings: &[crate::archive::Recording]) -> Vec<Finding> {
    let tracks = doc
        .elements()
        .filter(|e| kind(e) == Some(Kind::Track))
        .count();
    let mut out = Vec::new();
    let mut push = |message: String| {
        out.push(Finding {
            layer: Layer::Rules,
            rule: "archive-recordings",
            element: doc.root.id(),
            path: format!("/{}", doc.root.name()),
            message,
        })
    };
    for (i, r) in recordings.iter().enumerate() {
        if r.track as usize > tracks {
            push(format!(
                "{} is the recording of track {}, and the document has {tracks} track elements",
                r.name, r.track
            ));
        }
        if let Some(first) = recordings[..i]
            .iter()
            .find(|o| o.modality == r.modality && o.track == r.track)
        {
            push(format!(
                "{} and {} are both the {} recording of track {}; a track has at most one",
                first.name,
                r.name,
                r.modality.folder(),
                r.track
            ));
        }
    }
    out
}

/// The spec's conformance clause: an instance with no version is checked
/// against the latest version the validator supports, with a warning. One
/// that declares another version is checked against this one too, since
/// before 1.0 Segler carries one rule set.
fn version_check(doc: &Document, out: &mut Vec<Finding>) {
    if kind(&doc.root) != Some(Kind::Doclang) {
        return;
    }
    let (rule, message) = match doclang::version(doc) {
        Some(crate::SPEC_VERSION) => return,
        Some(v) => (
            "other",
            format!(
                "the document declares DocLang {v}; it was checked as DocLang {}",
                crate::SPEC_VERSION
            ),
        ),
        None => (
            "missing",
            format!(
                "the document declares no DocLang version; it was checked as DocLang {}",
                crate::SPEC_VERSION
            ),
        ),
    };
    out.push(Finding {
        layer: Layer::Version,
        rule,
        element: doc.root.id(),
        path: format!("/{}", doc.root.name()),
        message,
    });
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
            | Kind::Track
    )
}

/// The components of a track timestamp run.
fn is_time_part(k: Kind) -> bool {
    matches!(k, Kind::Hours | Kind::Minutes | Kind::Seconds | Kind::Msecs)
}

/// `transcript_content`: what a track cue block holds after its media.
fn is_transcript(k: Kind) -> bool {
    k == Kind::Content || k == Kind::Voice || is_headless_txt(k)
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
        /// `src? vtc*`: `cover`, `frame`, `audio`.
        Media,
        /// `cover? (bdiv timestamp timestamp? chapter? frame? audio?
        /// transcript*)*`: `track`.
        Track,
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
            | Layer | Src | Fcel | Ecel | Ched | Rhed | Corn | Srow | Lcel | Ucel | Xcel | Nl
            | Bdiv | Hours | Minutes | Seconds | Msecs => m(false, false, Body::Empty),
            Content => m(true, false, Body::TextOnly),
            Description | Summary => m(true, false, Body::Choice(|c| c == Content)),
            Bold | Italic | Underline | Strikethrough | Superscript | Subscript | Handwriting
            | Rtl | Hint => m(
                true,
                false,
                Body::Choice(|c| c == Content || is_formatting(c)),
            ),
            Text | Heading | Caption | PageHeader | PageFooter | Footnote | Key | Value
            | Marker | Voice | Chapter => m(
                true,
                true,
                Body::Choice(|c| c == Content || is_headless_txt(c) || is_top_level(c)),
            ),
            Code | Formula => m(true, true, Body::Choice(|c| c == Content || is_headless(c))),
            FieldRegion | FieldItem => m(
                true,
                true,
                Body::Choice(|c| is_headless_txt(c) || is_top_level(c)),
            ),
            FieldHeading => m(
                true,
                true,
                Body::Choice(|c| c == Content || is_headless_txt(c) || is_top_level(c)),
            ),
            Group => m(false, true, Body::Choice(is_top_level)),
            List => m(true, true, Body::Items),
            Table | Index => m(true, true, Body::Cells),
            Tabular => m(true, false, Body::Cells),
            Picture => m(true, true, Body::Picture),
            Cover | Frame | Audio => m(true, true, Body::Media),
            Track => m(true, true, Body::Track),
            Ldiv => m(false, false, Body::Divider),
        }
    }

    /// The attributes an element declares, with their value constraint.
    enum Attr {
        Enum(&'static [&'static str]),
        NonNegative,
        Positive,
        /// An integer from the first bound to the second, both included.
        Range(i128, i128),
        /// `MAJOR.MINOR`, the XSD's `[0-9]+\.[0-9]+`.
        MinorVersion,
        Any,
    }

    fn attributes(k: Kind) -> &'static [(&'static str, bool, Attr)] {
        use Attr::*;
        use Kind::*;
        match k {
            Doclang => &[("version", false, MinorVersion)],
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
            Hours => &[("value", true, NonNegative)],
            Minutes | Seconds => &[("value", true, Range(0, 59))],
            Msecs => &[("value", true, Range(0, 999))],
            DefaultResolution => &[
                ("width", false, NonNegative),
                ("height", false, NonNegative),
            ],
            _ => &[],
        }
    }

    fn is_minor_version(s: &str) -> bool {
        let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
        s.split_once('.')
            .is_some_and(|(major, minor)| digits(major) && digits(minor))
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
                        Attr::Range(lo, hi) => {
                            is_integer(&a.value).is_some_and(|n| (*lo..=*hi).contains(&n))
                        }
                        Attr::MinorVersion => is_minor_version(&a.value),
                    };
                    if !ok {
                        let expected = match constraint {
                            Attr::Enum(values) => format!("one of {}", values.join(", ")),
                            Attr::NonNegative => "a non-negative integer".to_owned(),
                            Attr::Positive => "a positive integer".to_owned(),
                            Attr::Range(lo, hi) => format!("an integer from {lo} to {hi}"),
                            Attr::MinorVersion => "a MAJOR.MINOR version".to_owned(),
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
            Body::Media => {
                let mut j = i;
                if children
                    .get(j)
                    .is_some_and(|(_, c)| kind(c) == Some(Kind::Src))
                {
                    j += 1;
                }
                for (_, c) in &children[j..] {
                    match kind(c) {
                        Some(Kind::Src) => push(
                            el,
                            path,
                            format!("<src> must come first in the body of <{}>", el.name()),
                        ),
                        _ => reject_unless(el, c, path, is_vtc, push),
                    }
                }
            }
            Body::Track => check_track(el, &children, i, path, push),
        }
    }

    /// `cover?` then cue blocks, each `bdiv timestamp timestamp? chapter?
    /// frame? audio? transcript*`. A cue block that goes wrong is reported
    /// once and skipped to the next `bdiv`.
    fn check_track(
        el: &Element,
        children: &[(usize, &Element)],
        start: usize,
        path: &str,
        push: &mut impl FnMut(&Element, &str, String),
    ) {
        let at = |j: usize| children.get(j).and_then(|(_, c)| kind(c));
        let mut j = start;
        if at(j) == Some(Kind::Cover) {
            j += 1;
        }
        while j < children.len() {
            let (_, c) = children[j];
            if kind(c) != Some(Kind::Bdiv) {
                push(
                    el,
                    path,
                    format!("a cue block begins with <bdiv>; found <{}>", c.name()),
                );
                j = next_bdiv(children, j + 1);
                continue;
            }
            j += 1;
            match timestamp(children, j) {
                Ok(next) => j = next,
                Err(found) => {
                    push(el, path, format!("a cue block's start timestamp is hours? minutes? seconds msecs?; found {found}"));
                    j = next_bdiv(children, j);
                    continue;
                }
            }
            if at(j).is_some_and(is_time_part) {
                match timestamp(children, j) {
                    Ok(next) => j = next,
                    Err(found) => {
                        push(el, path, format!("a cue block's end timestamp is hours? minutes? seconds msecs?; found {found}"));
                        j = next_bdiv(children, j);
                        continue;
                    }
                }
            }
            for optional in [Kind::Chapter, Kind::Frame, Kind::Audio] {
                if at(j) == Some(optional) {
                    j += 1;
                }
            }
            while j < children.len() && at(j) != Some(Kind::Bdiv) {
                let (_, c) = children[j];
                match kind(c) {
                    Some(k)
                        if is_time_part(k)
                            || matches!(
                                k,
                                Kind::Chapter | Kind::Frame | Kind::Audio | Kind::Cover
                            ) =>
                    {
                        push(
                            el,
                            path,
                            format!("<{}> is out of order in a cue block of <track>", c.name()),
                        )
                    }
                    _ => reject_unless(el, c, path, is_transcript, push),
                }
                j += 1;
            }
        }
    }

    /// Consume `hours? minutes? seconds msecs?` from `j`, returning the
    /// index after it, or what was found where `seconds` was due.
    fn timestamp(children: &[(usize, &Element)], mut j: usize) -> Result<usize, String> {
        let at = |j: usize| children.get(j).and_then(|(_, c)| kind(c));
        if at(j) == Some(Kind::Hours) {
            j += 1;
        }
        if at(j) == Some(Kind::Minutes) {
            j += 1;
        }
        if at(j) != Some(Kind::Seconds) {
            return Err(children
                .get(j)
                .map_or("nothing".to_owned(), |(_, c)| format!("<{}>", c.name())));
        }
        j += 1;
        if at(j) == Some(Kind::Msecs) {
            j += 1;
        }
        Ok(j)
    }

    fn next_bdiv(children: &[(usize, &Element)], from: usize) -> usize {
        (from..children.len())
            .find(|&j| kind(children[j].1) == Some(Kind::Bdiv))
            .unwrap_or(children.len())
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
            } else if s == 5 && next_slot <= 6 {
                // Captions repeat.
                next_slot = 6;
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

            // track-structure
            if k == Kind::Track && !children.is_empty() {
                if let Some((_, c)) = first_body {
                    if !matches!(kind(c), Some(Kind::Cover | Kind::Bdiv)) {
                        push("track-structure", format!("Track must have bdiv (optionally preceded by a single cover) as first element after the optional element head; found {}", c.name()));
                    }
                }
                if let Some((first_bdiv, _)) =
                    children.iter().find(|(_, c)| kind(c) == Some(Kind::Bdiv))
                {
                    if let Some(text) = text_in(&v.el.children()[..*first_bdiv]) {
                        push("track-structure", format!("Track must not contain non-whitespace text before its first cue block (bdiv); found {text:?}"));
                    }
                }
            }

            // track-cue-block, track-cue-block-timestamp-order,
            // track-audio-requires-end, track-cue-block-sequence
            if k == Kind::Track {
                let bdivs: Vec<usize> = children
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, c))| kind(c) == Some(Kind::Bdiv))
                    .map(|(n, _)| n)
                    .collect();
                for (b, &n) in bdivs.iter().enumerate() {
                    let end = bdivs.get(b + 1).copied().unwrap_or(children.len());
                    let cue = &children[n + 1..end];
                    let node_end = cue_node_end(v.el, &children, end);
                    let first = cue.first().map(|(i, c)| (*i, *c));
                    if let Some((_, c)) = first {
                        if !matches!(kind(c), Some(Kind::Hours | Kind::Minutes | Kind::Seconds)) {
                            push("track-cue-block", format!("A track cue block must begin with a start timestamp: its first element must be hours, minutes or seconds; found {}", c.name()));
                        }
                    }
                    let text_end = first.map_or(node_end, |(i, _)| i);
                    if let Some(text) = text_in(&v.el.children()[children[n].0 + 1..text_end]) {
                        push("track-cue-block", format!("A track cue block must not contain non-whitespace text before its start timestamp; found {text:?}"));
                    }

                    let seconds: Vec<usize> = (n + 1..end)
                        .filter(|&j| kind(children[j].1) == Some(Kind::Seconds))
                        .collect();
                    if let [s1, s2] = seconds[..] {
                        let (start, finish) =
                            (millis(&children, Some(s1)), millis(&children, Some(s2)));
                        if !matches!(
                            finish.partial_cmp(&start),
                            Some(Ordering::Greater | Ordering::Equal)
                        ) {
                            push("track-cue-block-timestamp-order", format!("A track cue block end timestamp must not be earlier than its start timestamp; found start={start}ms, end={finish}ms"));
                        }
                    }
                    if cue.iter().any(|(_, c)| kind(c) == Some(Kind::Audio)) && seconds.len() != 2 {
                        push("track-audio-requires-end", "A track cue block with an audio clip must carry an end time; the clip spans the cue block's interval [start, end]".to_owned());
                    }

                    if b > 0 {
                        let start = millis(&children, seconds_after(&children, n));
                        let previous = millis(&children, seconds_after(&children, bdivs[b - 1]));
                        if !matches!(
                            start.partial_cmp(&previous),
                            Some(Ordering::Greater | Ordering::Equal)
                        ) {
                            push("track-cue-block-sequence", format!("Track cue blocks must appear in non-decreasing order of start time; found this cue block start={start}ms, previous cue block start={previous}ms"));
                        }
                    }
                }

                // track-chapter-strictly-increasing
                let block_start = |j: usize| {
                    let bdiv = (0..j)
                        .rev()
                        .find(|&b| kind(children[b].1) == Some(Kind::Bdiv));
                    millis(&children, bdiv.and_then(|b| seconds_after(&children, b)))
                };
                let chapters: Vec<usize> = (0..children.len())
                    .filter(|&j| kind(children[j].1) == Some(Kind::Chapter))
                    .collect();
                for pair in chapters.windows(2) {
                    let (previous, start) = (block_start(pair[0]), block_start(pair[1]));
                    if start.partial_cmp(&previous) != Some(Ordering::Greater) {
                        push("track-chapter-strictly-increasing", format!("Each chapter must begin strictly later than the previous chapter; two chapters cannot mark the same instant; found this chapter start={start}ms, previous chapter start={previous}ms"));
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
                    | Kind::Voice
                    | Kind::Chapter
                    | Kind::Cover
                    | Kind::Frame
                    | Kind::Audio
                    | Kind::Track
            ) {
                // For a list, table, index or track only its own head counts,
                // the part before its first body element: an item's, a cell's
                // or a cue block's own head is the virtual-text patterns'
                // business.
                let own = match k {
                    Kind::List => children
                        .iter()
                        .find(|(_, c)| kind(c) == Some(Kind::Ldiv))
                        .map_or(v.el.children().len(), |(i, _)| *i),
                    Kind::Table | Kind::Index => children
                        .iter()
                        .find(|(_, c)| kind(c).is_some_and(is_table_sep))
                        .map_or(v.el.children().len(), |(i, _)| *i),
                    Kind::Track => first_body.map_or(v.el.children().len(), |(i, _)| i),
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

    /// Non-whitespace text among `nodes`, trimmed and joined, if any.
    fn text_in(nodes: &[Node]) -> Option<String> {
        let text: String = nodes
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) if !t.value().trim().is_empty() => Some(t.value().trim()),
                Node::CData(c) if !c.trim().is_empty() => Some(c.trim()),
                _ => None,
            })
            .collect();
        (!text.is_empty()).then_some(text)
    }

    /// The node index a cue block ends at: the next `bdiv`'s, or the end of
    /// the track.
    fn cue_node_end(track: &Element, children: &[(usize, &Element)], next: usize) -> usize {
        children
            .get(next)
            .map_or(track.children().len(), |(i, _)| *i)
    }

    /// `following-sibling::dl:seconds[1]` of the element at `from`, which
    /// may lie past the next `bdiv`, as the XPath does not stop there.
    fn seconds_after(children: &[(usize, &Element)], from: usize) -> Option<usize> {
        (from + 1..children.len()).find(|&j| kind(children[j].1) == Some(Kind::Seconds))
    }

    /// A timestamp in milliseconds from the run anchored on the `seconds` at
    /// `seconds`: the `minutes` directly before it, the `hours` directly
    /// before those (or before the seconds), the `msecs` directly after.
    /// NaN where the XPath's `number()` would give it, which fails every
    /// comparison as the assertions expect.
    fn millis(children: &[(usize, &Element)], seconds: Option<usize>) -> f64 {
        let Some(s) = seconds else {
            return f64::NAN;
        };
        let value = |j: usize| number(children[j].1.attr("value").unwrap_or(""));
        let is = |j: Option<usize>, want: Kind| j.filter(|&j| kind(children[j].1) == Some(want));
        let before = |j: usize| j.checked_sub(1);
        let minutes = is(before(s), Kind::Minutes);
        let hours = is(before(minutes.unwrap_or(s)), Kind::Hours);
        let msecs = is(Some(s + 1).filter(|&j| j < children.len()), Kind::Msecs);
        3_600_000.0 * hours.map_or(0.0, value)
            + 60_000.0 * minutes.map_or(0.0, value)
            + 1000.0 * value(s)
            + msecs.map_or(0.0, value)
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

    /// The defects in `src`; the version warning is tested on its own.
    fn findings(src: &str) -> Vec<(Layer, &'static str)> {
        validate(&Document::parse(src).unwrap())
            .into_iter()
            .filter(Finding::is_defect)
            .map(|f| (f.layer, f.rule))
            .collect()
    }

    #[test]
    fn a_valid_document_has_no_findings() {
        let src = r#"<doclang xmlns="https://www.doclang.ai/ns/v0" version="0.9"><head><default_resolution width="1000"/></head>
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
            findings("<doclang version=\"6\"/>"),
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
    fn another_or_no_version_is_a_warning_and_still_valid() {
        let current = format!("<doclang version=\"{}\"/>", crate::SPEC_VERSION);
        assert_eq!(validate(&Document::parse(&current).unwrap()), []);
        for src in ["<doclang version=\"0.7\"/>", "<doclang/>"] {
            let all = validate(&Document::parse(src).unwrap());
            assert_eq!(all.len(), 1, "{src}");
            assert_eq!(all[0].layer, Layer::Version);
            assert!(!all[0].is_defect());
        }
    }

    #[test]
    fn captions_repeat_and_markers_and_field_headings_widened() {
        let v = |body: &str| format!("<doclang version=\"0.9\">{body}</doclang>");
        assert_eq!(findings(&v("<picture><caption>a</caption><caption>b</caption><description>d</description></picture>")), []);
        assert_eq!(
            findings(&v("<picture><caption>a</caption><description>d</description><caption>b</caption></picture>")),
            [(Layer::Schema, "xsd")]
        );
        assert_eq!(
            findings(&v(
                "<list><ldiv><marker><text>1.</text></marker></ldiv>one</list>"
            )),
            []
        );
        assert_eq!(findings(&v("<field_region><field_heading><content>A  B</content></field_heading></field_region>")), []);
    }

    #[test]
    fn tracks() {
        let v = |body: &str| format!("<doclang version=\"0.9\"><track>{body}</track></doclang>");
        let ok = "<cover><src uri=\"c.png\"/></cover><bdiv/><seconds value=\"1\"/><minutes value=\"0\"/><seconds value=\"2\"/><msecs value=\"500\"/><chapter>One</chapter><audio><src uri=\"a.mp3\"/></audio><voice>Ann</voice> hello <bold>there</bold><bdiv/><hours value=\"1\"/><seconds value=\"0\"/><chapter>Two</chapter>bye";
        assert_eq!(findings(&v(ok)), []);
        let schema = [(Layer::Schema, "xsd")];
        assert_eq!(
            findings(&v("<bdiv/><minutes value=\"60\"/><seconds value=\"0\"/>")),
            schema
        );
        assert_eq!(
            findings(&v(
                "<bdiv/><seconds value=\"0\"/><seconds value=\"1\"/><seconds value=\"2\"/>"
            )),
            schema
        );
        assert_eq!(
            findings(&v(
                "<bdiv/><seconds value=\"0\"/><frame/><chapter>x</chapter>"
            )),
            schema
        );
        let rule = |body: &str| {
            findings(&v(body))
                .into_iter()
                .filter(|f| f.0 == Layer::Rules)
                .map(|f| f.1)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            rule("<bdiv/><seconds value=\"5\"/><seconds value=\"4\"/>"),
            ["track-cue-block-timestamp-order"]
        );
        assert_eq!(
            rule("<bdiv/><seconds value=\"5\"/><bdiv/><seconds value=\"4\"/>"),
            ["track-cue-block-sequence"]
        );
        assert_eq!(
            rule("<bdiv/><seconds value=\"5\"/><audio/>"),
            ["track-audio-requires-end"]
        );
        assert_eq!(
            rule("<bdiv/>early<seconds value=\"5\"/>"),
            ["track-cue-block"]
        );
        assert_eq!(
            rule("stray<bdiv/><seconds value=\"5\"/>"),
            ["track-structure"]
        );
        assert_eq!(
            rule("<bdiv/><seconds value=\"5\"/><chapter>a</chapter><bdiv/><seconds value=\"5\"/><chapter>b</chapter>"),
            ["track-chapter-strictly-increasing"]
        );
    }

    #[test]
    fn a_recording_needs_its_track_and_a_track_one_recording_per_kind() {
        use crate::archive::{Modality, Recording};
        let doc =
            Document::parse("<doclang><track><bdiv/><seconds value=\"0\"/></track></doclang>")
                .unwrap();
        let r = |modality, track, name: &str| Recording {
            modality,
            track,
            name: name.to_owned(),
        };
        assert_eq!(
            recordings(
                &doc,
                &[
                    r(Modality::Audio, 1, "audio/1.wav"),
                    r(Modality::Video, 1, "video/1.mp4")
                ]
            ),
            []
        );
        let found = recordings(
            &doc,
            &[
                r(Modality::Audio, 1, "audio/1.mp3"),
                r(Modality::Audio, 1, "audio/1.wav"),
                r(Modality::Video, 2, "video/2.mp4"),
            ],
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found
            .iter()
            .all(|f| f.rule == "archive-recordings" && f.is_defect()));
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
