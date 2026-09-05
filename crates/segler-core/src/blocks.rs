//! The document as a renderer sees it: blocks in reading order, each with
//! its inline runs, built from the tree and pointing back into it by
//! element id. This is what the document pane draws and what `segler
//! render` prints, so the pane's input can be checked without a window.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::ops::Range;

use crate::doclang::{self, Kind};
use crate::otsl::{self, CellKind, Grid};
use crate::tree::{Element, ElementId, Node};

/// Inline styling accumulated from the formatting elements around a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub handwriting: bool,
    pub rtl: bool,
    /// Whitespace kept as written: a `content` element.
    pub preformatted: bool,
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

/// One item of a list: its marker, its own text if it is a virtual text
/// item, and the blocks it wraps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// The `ldiv` element. It is an empty separator: the item's content is
    /// the siblings that follow it, which is why editing one goes through
    /// `SetListItemText` and a range rather than through `SetText`.
    pub id: ElementId,
    pub marker: Option<String>,
    pub runs: Vec<Run>,
    pub blocks: Vec<Block>,
    /// Whether the item's content is plain text, so a retype loses nothing.
    pub plain: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellBlock {
    pub row: usize,
    pub col: usize,
    pub rowspan: usize,
    pub colspan: usize,
    pub kind: CellKind,
    pub runs: Vec<Run>,
    pub blocks: Vec<Block>,
    /// Whether the cell's body is plain text, so a retype loses nothing.
    pub plain: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading {
        id: ElementId,
        level: u32,
        runs: Vec<Run>,
        plain: bool,
    },
    /// `text`, `page_header`, `page_footer`, `footnote`, `caption`,
    /// `key`, `value`, `hint`, `field_heading`, `marker` on its own.
    Paragraph {
        id: ElementId,
        kind: Kind,
        runs: Vec<Run>,
        blocks: Vec<Block>,
        plain: bool,
    },
    List {
        id: ElementId,
        ordered: bool,
        items: Vec<ListItem>,
    },
    Table {
        id: ElementId,
        kind: Kind,
        caption: Option<Vec<Run>>,
        rows: usize,
        cols: usize,
        cells: Vec<CellBlock>,
    },
    Picture {
        id: ElementId,
        class: Option<String>,
        /// The `src` URI, resolved by the renderer against the archive.
        src: Option<String>,
        caption: Option<Vec<Run>>,
        blocks: Vec<Block>,
    },
    Code {
        id: ElementId,
        language: Option<String>,
        text: String,
        plain: bool,
    },
    Formula {
        id: ElementId,
        text: String,
        plain: bool,
    },
    /// `group`, `field_region`, `field_item`: a box around its blocks.
    Container {
        id: ElementId,
        kind: Kind,
        blocks: Vec<Block>,
    },
    PageBreak,
    /// A DocLang element this renderer has no shape for, or a custom
    /// element: shown by name with its text.
    Other {
        id: ElementId,
        name: String,
        runs: Vec<Run>,
    },
}

impl Block {
    pub fn id(&self) -> Option<ElementId> {
        match self {
            Block::Heading { id, .. }
            | Block::Paragraph { id, .. }
            | Block::List { id, .. }
            | Block::Table { id, .. }
            | Block::Picture { id, .. }
            | Block::Code { id, .. }
            | Block::Formula { id, .. }
            | Block::Container { id, .. }
            | Block::Other { id, .. } => Some(*id),
            Block::PageBreak => None,
        }
    }
}

/// The blocks for a sequence of nodes, in order.
pub fn blocks_of(nodes: &[Node]) -> Vec<Block> {
    let mut out = Vec::new();
    for node in nodes {
        if let Node::Element(el) = node {
            out.push(block_of(el));
        }
    }
    out
}

/// The block for one element.
pub fn block_of(el: &Element) -> Block {
    let id = el.id();
    let Some(kind) = doclang::kind(el) else {
        return Block::Other {
            id,
            name: el.name().to_owned(),
            runs: runs_of(el.children(), Style::default()).0,
        };
    };
    let head = doclang::head(el);
    let body = &el.children()[head.body.clone()];
    let caption = head.caption.map(|c| {
        let ch = doclang::head(c);
        runs_of(&c.children()[ch.body], Style::default()).0
    });
    match kind {
        Kind::Heading => {
            let (runs, _) = runs_of(body, Style::default());
            Block::Heading {
                id,
                level: el.attr("level").and_then(|v| v.parse().ok()).unwrap_or(1),
                runs,
                plain: is_plain(body),
            }
        }
        Kind::Text
        | Kind::PageHeader
        | Kind::PageFooter
        | Kind::Footnote
        | Kind::Caption
        | Kind::Key
        | Kind::Value
        | Kind::Hint
        | Kind::FieldHeading
        | Kind::Marker
        | Kind::Description
        | Kind::Summary => {
            let (runs, blocks) = runs_of(body, Style::default());
            Block::Paragraph {
                id,
                kind,
                runs,
                blocks,
                plain: is_plain(body),
            }
        }
        Kind::List => Block::List {
            id,
            ordered: el.attr("class") == Some("ordered"),
            items: list_items(el, head.body.start),
        },
        Kind::Table | Kind::Index | Kind::Tabular => {
            let grid = Grid::parse(el);
            let cells = grid
                .cells
                .iter()
                .map(|cell| {
                    let start = otsl::body_start(el, cell);
                    let nodes = &el.children()[start..cell.content.end];
                    let (runs, blocks) = runs_of(nodes, Style::default());
                    CellBlock {
                        row: cell.row,
                        col: cell.col,
                        rowspan: cell.rowspan,
                        colspan: cell.colspan,
                        kind: cell.kind,
                        runs,
                        blocks,
                        plain: is_plain(nodes),
                    }
                })
                .collect();
            Block::Table {
                id,
                kind,
                caption,
                rows: grid.rows,
                cols: grid.cols,
                cells,
            }
        }
        Kind::Picture => {
            let src = el
                .child_elements()
                .find(|c| doclang::kind(c) == Some(Kind::Src))
                .and_then(|s| s.attr("uri"))
                .map(str::to_owned);
            let inner: Vec<Block> = body
                .iter()
                .filter_map(|n| match n {
                    Node::Element(e) if !matches!(doclang::kind(e), Some(Kind::Src)) => {
                        Some(block_of(e))
                    }
                    _ => None,
                })
                .collect();
            Block::Picture {
                id,
                class: el.attr("class").map(str::to_owned),
                src,
                caption,
                blocks: inner,
            }
        }
        Kind::Code => Block::Code {
            id,
            language: head.label.map(str::to_owned),
            text: raw_text(body),
            plain: is_plain(body),
        },
        Kind::Formula => Block::Formula {
            id,
            text: raw_text(body),
            plain: is_plain(body),
        },
        Kind::Group | Kind::FieldRegion | Kind::FieldItem => Block::Container {
            id,
            kind,
            blocks: blocks_of(body),
        },
        Kind::PageBreak => Block::PageBreak,
        _ => Block::Other {
            id,
            name: el.name().to_owned(),
            runs: runs_of(body, Style::default()).0,
        },
    }
}

/// Whether a body is text a person can retype without losing structure:
/// character data only, no elements.
pub fn is_plain(nodes: &[Node]) -> bool {
    nodes
        .iter()
        .all(|n| matches!(n, Node::Text(_) | Node::CData(_)))
}

/// Inline runs from a sequence of nodes, plus any block-level elements
/// found among them, which a paragraph renders after its runs.
pub fn runs_of(nodes: &[Node], style: Style) -> (Vec<Run>, Vec<Block>) {
    let mut runs: Vec<Run> = Vec::new();
    let mut blocks = Vec::new();
    for node in nodes {
        match node {
            Node::Text(t) => push_run(&mut runs, t.value(), style),
            Node::CData(s) => push_run(&mut runs, s, style),
            Node::Comment(_) | Node::Pi { .. } => {}
            Node::Element(el) => match doclang::kind(el) {
                Some(k) if k.category() == doclang::Category::Formatting => {
                    let mut inner = style;
                    match k {
                        Kind::Bold => inner.bold = true,
                        Kind::Italic => inner.italic = true,
                        Kind::Underline => inner.underline = true,
                        Kind::Strikethrough => inner.strikethrough = true,
                        Kind::Superscript => inner.superscript = true,
                        Kind::Subscript => inner.subscript = true,
                        Kind::Handwriting => inner.handwriting = true,
                        Kind::Rtl => inner.rtl = true,
                        _ => {}
                    }
                    let (r, b) = runs_of(el.children(), inner);
                    runs.extend(r);
                    blocks.extend(b);
                }
                Some(Kind::Content) => {
                    let mut inner = style;
                    inner.preformatted = true;
                    runs.push(Run {
                        text: el.text(),
                        style: inner,
                    });
                }
                Some(Kind::Checkbox) => {
                    let mark = if el.attr("class") == Some("selected") {
                        "☑ "
                    } else {
                        "☐ "
                    };
                    push_run(&mut runs, mark, style);
                }
                Some(Kind::Marker) | Some(Kind::Hint) => {
                    let (r, _) = runs_of(el.children(), style);
                    runs.extend(r);
                }
                Some(k) if k.is_property() => {}
                Some(_) | None => blocks.push(block_of(el)),
            },
        }
    }
    // Trim the whitespace pretty-printing leaves at either end.
    if let Some(first) = runs.first_mut() {
        if !first.style.preformatted {
            first.text = first.text.trim_start().to_owned();
        }
    }
    if let Some(last) = runs.last_mut() {
        if !last.style.preformatted {
            last.text = last.text.trim_end().to_owned();
        }
    }
    runs.retain(|r| !r.text.is_empty());
    (runs, blocks)
}

/// Append text to the runs, collapsing whitespace as XML does by default
/// and merging with the previous run when the style is the same.
fn push_run(runs: &mut Vec<Run>, text: &str, style: Style) {
    let collapsed = collapse(text);
    if collapsed.is_empty() {
        return;
    }
    match runs.last_mut() {
        Some(last) if last.style == style => last.text.push_str(&collapsed),
        _ => runs.push(Run {
            text: collapsed,
            style,
        }),
    }
}

/// Runs of whitespace become one space; a run that is only whitespace
/// becomes one space so words on either side stay apart.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

/// Text as written, for code and formulas, with the framing whitespace of
/// a pretty-printed body removed.
fn raw_text(nodes: &[Node]) -> String {
    let mut out = String::new();
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t.value()),
            Node::CData(s) => out.push_str(s),
            Node::Element(e) => out.push_str(&e.text()),
            _ => {}
        }
    }
    out.trim_matches(|c: char| c == '\n' || c == '\r')
        .to_owned()
}

/// Where each list item's content sits among `list`'s children.
///
/// An `ldiv` is an empty separator and the item's content is the siblings
/// that follow it, so an item is a *range* rather than an element. That is
/// the same shape an OTSL cell has, and it is why editing one goes through
/// [`crate::session::Command::SetListItemText`] and a range, where every
/// other body edit goes through an element.
///
/// Public because the session computes the same ranges to apply an edit, and
/// two implementations of this would part company the first time a head
/// element was added to the skip list below.
pub fn item_bodies(list: &Element, body_start: usize) -> Vec<(ElementId, Range<usize>)> {
    let children = list.children();
    let dividers: Vec<usize> = children
        .iter()
        .enumerate()
        .skip(body_start)
        .filter_map(|(i, n)| match n {
            Node::Element(e) if doclang::kind(e) == Some(Kind::Ldiv) => Some(i),
            _ => None,
        })
        .collect();
    dividers
        .iter()
        .enumerate()
        .map(|(n, &at)| {
            let end = dividers.get(n + 1).copied().unwrap_or(children.len());
            let id = match &children[at] {
                Node::Element(e) => e.id(),
                _ => unreachable!("a divider is an element"),
            };
            // The item's own head, if it is a virtual text, precedes its body.
            let mut start = at + 1;
            while start < end {
                match &children[start] {
                    Node::Element(e) if doclang::kind(e).is_some_and(Kind::is_property) => {
                        start += 1
                    }
                    _ => break,
                }
            }
            (id, start..end)
        })
        .collect()
}

fn list_items(list: &Element, body_start: usize) -> Vec<ListItem> {
    let children = list.children();
    item_bodies(list, body_start)
        .into_iter()
        .map(|(id, body)| {
            let ldiv = list
                .child_elements()
                .find(|e| e.id() == id)
                .expect("the divider this range was built from");
            let marker = ldiv
                .child_elements()
                .find(|c| doclang::kind(c) == Some(Kind::Marker))
                .map(|m| collapse(&m.text()).trim().to_owned())
                .filter(|m| !m.is_empty());
            let nodes = &children[body.clone()];
            let (runs, blocks) = runs_of(nodes, Style::default());
            ListItem {
                id,
                marker,
                runs,
                blocks,
                plain: is_plain(nodes),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Document;

    fn first(src: &str) -> Block {
        let doc = Document::parse(&format!("<doclang>{src}</doclang>")).unwrap();
        let block = block_of(doc.root.child_elements().next().unwrap());
        block
    }

    #[test]
    fn runs_merge_collapse_and_style() {
        let b =
            first("<text>\n  Plain <bold>bold <italic>both</italic></bold> and\n  more  \n</text>");
        let Block::Paragraph { runs, plain, .. } = b else {
            panic!("{b:?}")
        };
        assert!(!plain);
        let texts: Vec<(&str, bool, bool)> = runs
            .iter()
            .map(|r| (r.text.as_str(), r.style.bold, r.style.italic))
            .collect();
        assert_eq!(
            texts,
            [
                ("Plain ", false, false),
                ("bold ", true, false),
                ("both", true, true),
                (" and more", false, false)
            ]
        );
    }

    #[test]
    fn plain_bodies_are_marked_plain_and_heads_are_skipped() {
        let b = first("<heading level=\"2\"><location value=\"1\"/><location value=\"2\"/><location value=\"3\"/><location value=\"4\"/><caption>Cap</caption>Title</heading>");
        let Block::Heading {
            level, runs, plain, ..
        } = b
        else {
            panic!("{b:?}")
        };
        assert_eq!((level, plain), (2, true));
        assert_eq!(runs[0].text, "Title");
    }

    #[test]
    fn lists_with_markers_and_wrapped_items() {
        let b = first("<list class=\"ordered\"><ldiv><marker>1.</marker></ldiv>First <bold>one</bold><ldiv/><text>Wrapped</text><ldiv/><location value=\"1\"/><location value=\"2\"/><location value=\"3\"/><location value=\"4\"/>Located</list>");
        let Block::List { ordered, items, .. } = b else {
            panic!("{b:?}")
        };
        assert!(ordered);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].marker.as_deref(), Some("1."));
        assert_eq!(items[0].runs.len(), 2);
        assert!(items[1].runs.is_empty());
        assert!(matches!(items[1].blocks[0], Block::Paragraph { .. }));
        assert_eq!(items[2].runs[0].text, "Located");
    }

    #[test]
    fn tables_carry_their_grid_and_caption() {
        let b = first("<table><caption>T1</caption><ched/>A<lcel/><nl/><fcel/><text>x</text><fcel/>y<nl/></table>");
        let Block::Table {
            rows,
            cols,
            cells,
            caption,
            ..
        } = b
        else {
            panic!("{b:?}")
        };
        assert_eq!((rows, cols, cells.len()), (2, 2, 3));
        assert_eq!(caption.unwrap()[0].text, "T1");
        assert_eq!(
            (cells[0].colspan, cells[0].kind),
            (2, CellKind::ColumnHeader)
        );
        assert!(!cells[1].plain && !cells[1].blocks.is_empty());
        assert!(cells[2].plain);
        assert_eq!(cells[2].runs[0].text, "y");
    }

    #[test]
    fn pictures_code_and_checkboxes() {
        let b = first("<picture class=\"chart\"><label value=\"bar\"/><caption>Fig</caption><src uri=\"assets/a.png\"/><text>inner</text></picture>");
        let Block::Picture {
            src,
            caption,
            blocks,
            class,
            ..
        } = b
        else {
            panic!("{b:?}")
        };
        assert_eq!(src.as_deref(), Some("assets/a.png"));
        assert_eq!(caption.unwrap()[0].text, "Fig");
        assert_eq!((blocks.len(), class.as_deref()), (1, Some("chart")));
        let c = first("<code><label value=\"rust\"/>\nfn main() {}\n</code>");
        let Block::Code { language, text, .. } = c else {
            panic!("{c:?}")
        };
        assert_eq!(
            (language.as_deref(), text.as_str()),
            (Some("rust"), "fn main() {}")
        );
        let t = first("<text><checkbox class=\"selected\"/>Done</text>");
        let Block::Paragraph { runs, .. } = t else {
            panic!("{t:?}")
        };
        assert_eq!(runs[0].text, "☑ Done");
    }
}
