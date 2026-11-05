//! The editing session: the boundary between the core and any front-end.
//!
//! A front-end asks a session for view-models, plain data describing a
//! page or the selected element, draws them, and sends commands back. It
//! holds no document state of its own. The egui window is one such
//! front-end; a Swift or C# one would consume the same view-models and send
//! the same commands. Logic that a front-end would need and cannot get from
//! this module is missing from here, not from the front-end.
//!
//! Every command records what it changed as a restore snapshot rather than
//! as an inverse command, so that undo puts back the very nodes it took,
//! raw source and all, and a document edited and undone is the same bytes
//! it was. `segler corpus` measures that on every file in the corpus.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::fs;
use std::path::{Path, PathBuf};

use crate::archive::{self, Kind as SourceKind, Loaded};
use crate::blocks::{self, Block};
use crate::doclang::{self, Kind};
use crate::inline;
use crate::otsl::{self, CellKind, Grid};
use crate::replace;
use crate::tree::{self, Document, Element, ElementId, Node, Text};
use crate::validate::{self, Finding};
use crate::whitespace;

// ---------------------------------------------------------------------------
// Commands

/// An edit. Every variant names its target by id; the session refuses a
/// command whose target no longer exists rather than guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Replace the body of a text-carrying element with one run of text,
    /// keeping its head. Refused on elements whose body is structure.
    SetText { id: ElementId, text: String },
    /// Set an attribute, or remove it with `None`.
    SetAttr {
        id: ElementId,
        name: String,
        value: Option<String>,
    },
    /// Set the head's `label`, or remove it with `None`.
    SetLabel {
        id: ElementId,
        value: Option<String>,
    },
    /// Set the head's `layer`, or remove it with `None`.
    SetLayer {
        id: ElementId,
        value: Option<String>,
    },
    /// Set the head's bounding box as grid values `x_min, y_min, x_max,
    /// y_max`, or remove it with `None`.
    SetBounds {
        id: ElementId,
        bounds: Option<[u32; 4]>,
    },
    /// Change what an element is, keeping everything inside it.
    Rename { id: ElementId, kind: Kind },
    /// Move an element to `index` among the element children of `parent`.
    Move {
        id: ElementId,
        parent: ElementId,
        index: usize,
    },
    /// Insert a new, empty element of `kind` at `index` among the element
    /// children of `parent`. The new element's id comes back in [`Applied`].
    Insert {
        parent: ElementId,
        index: usize,
        kind: Kind,
    },
    /// Remove an element and everything inside it.
    Remove { id: ElementId },
    /// Replace the text of a table cell, addressed by the cell's top-left
    /// position. Refused when the cell holds structure.
    SetCellText {
        id: ElementId,
        row: usize,
        col: usize,
        text: String,
    },
    /// Replace the text of a list item, addressed by its position among the
    /// list's items counting from zero.
    ///
    /// By position rather than by the `ldiv`'s id, because an `ldiv` is an
    /// empty separator and the item's content is the siblings after it: there
    /// is no element whose body is the item, which is why `SetText` on an
    /// `ldiv` refuses and `SetText` on the list would replace every item at
    /// once. This is the same shape as [`Command::SetCellText`], for the same
    /// reason — both edit a range inside a flat run of tokens.
    SetListItemText {
        id: ElementId,
        item: usize,
        text: String,
    },
    SetTurnText {
        id: ElementId,
        cue: usize,
        turn: usize,
        text: String,
    },
    SetCueTimes {
        id: ElementId,
        cue: usize,
        text: String,
    },
    /// Change what a table cell is: its token.
    SetCellKind {
        id: ElementId,
        row: usize,
        col: usize,
        kind: CellKind,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("no element {0}")]
    NoSuchElement(ElementId),
    #[error("<{0}> carries structure, not text; edit its parts")]
    NotText(String),
    #[error("text contains a character XML cannot carry: U+{0:04X}")]
    NotXmlText(u32),
    #[error("the root element cannot be moved or removed")]
    Root,
    #[error("an element cannot be moved into itself")]
    IntoItself,
    #[error("index {index} is past the end of <{parent}>")]
    IndexOutOfRange { parent: String, index: usize },
    #[error("<{0}> has no cell at row {1}, column {2}")]
    NoSuchCell(String, usize, usize),
    #[error("<{0}> has no item {1}")]
    NoSuchListItem(String, usize),
    #[error("<{0}> has no turn {2} in cue block {1}")]
    NoSuchTurn(String, usize, usize),
    #[error("<{0}> has no cue block {1}")]
    NoSuchCue(String, usize),
    #[error("{0}")]
    BadTime(String),
    /// The field showed tags and what came back is not markup. Refused rather
    /// than fallen back on, because falling back would read a missing `>` as
    /// an instruction to delete every tag in the line.
    #[error("{0}")]
    Inline(#[from] inline::Error),
}

/// Which piece of text an edit is about to replace.
///
/// The three shapes a text edit comes in, and they are three rather than one
/// because DocLang puts a paragraph's text inside an element, a table cell's
/// between OTSL tokens, and a list item's between `ldiv` separators. Only the
/// first is an element's body; the other two are ranges of siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTarget {
    /// The body of a text-carrying element: [`Command::SetText`].
    Body(ElementId),
    /// A table cell: [`Command::SetCellText`].
    Cell {
        id: ElementId,
        row: usize,
        col: usize,
    },
    /// A list item: [`Command::SetListItemText`].
    Item { id: ElementId, item: usize },
    Turn {
        id: ElementId,
        cue: usize,
        turn: usize,
    },
}

/// What a command did, returned from `apply` and `redo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub command: Command,
    /// The element an `Insert` created; the target otherwise.
    pub element: ElementId,
}

/// How to put an element back the way it was.
#[derive(Debug, Clone)]
enum Restore {
    Children {
        id: ElementId,
        nodes: Vec<Node>,
    },
    Attr {
        id: ElementId,
        name: String,
        value: Option<String>,
    },
    Name {
        id: ElementId,
        name: String,
    },
    Many(Vec<Restore>),
}

struct Entry {
    command: Command,
    element: ElementId,
    restore: Restore,
}

// ---------------------------------------------------------------------------
// View-models

/// One page, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct PageView {
    /// One-based.
    pub number: usize,
    pub count: usize,
    /// The archive part holding this page's image, if there is one.
    pub image: Option<String>,
    /// Every located semantic element on the page, in document order.
    pub boxes: Vec<BoxView>,
    /// The page's semantic elements as a tree, in document order.
    pub rows: Vec<RowView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoxView {
    pub id: ElementId,
    pub kind: Kind,
    /// `[x_min, y_min, x_max, y_max]` as fractions of the page.
    pub rect: [f64; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowView {
    pub id: ElementId,
    /// Nesting under the page's top-level elements, from zero.
    pub depth: usize,
    pub kind: Kind,
    pub label: Option<String>,
    /// `level` for headings, `class` for lists and pictures.
    pub detail: Option<String>,
    /// The first words of the element's text.
    pub excerpt: String,
    pub located: bool,
    pub layer: String,
}

/// The selected element in full.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementView {
    pub id: ElementId,
    pub path: String,
    pub name: String,
    pub kind: Option<Kind>,
    pub page: usize,
    pub attrs: Vec<(String, String)>,
    pub label: Option<String>,
    pub layer: String,
    pub bounds: Option<[u32; 4]>,
    /// The bounds as fractions of the page.
    pub rect: Option<[f64; 4]>,
    /// All character data under the element, head included.
    pub text: String,
    /// The character data of the body alone, which is what a text edit
    /// replaces.
    pub body_text: String,
    /// Whether `SetText` applies to this element at all: whether its kind
    /// carries text rather than structure.
    ///
    /// This used to mean "and its body is plain", which is what stopped a
    /// formatted paragraph from being corrected. The two questions are now
    /// separate, because the answer to the second is a warning rather than a
    /// refusal; `plain_text` beside this is the second one.
    pub editable_text: bool,
    /// Whether the body is plain text, so a retype can lose nothing.
    ///
    /// False means the body carries formatting or other elements. The edit is
    /// still offered: [`Session::keeps_formatting`] says whether a particular
    /// new value would keep them, and only when it would not is there
    /// anything to warn about.
    pub plain_text: bool,
    /// The element's markup as it would be written.
    pub markup: String,
    pub parent: Option<ElementId>,
    /// Position among the parent's element children.
    pub index: usize,
}

/// The document as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentView {
    pub path: Option<PathBuf>,
    pub archive: bool,
    /// The declared version; `None` when the document declares none.
    pub version: Option<String>,
    pub resolution: (u32, u32),
    pub pages: usize,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

// ---------------------------------------------------------------------------
// The session

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Archive(#[from] archive::Error),
    #[error("{0}")]
    Xml(#[from] tree::Error),
    #[error("root element is <{0}>, not <doclang>")]
    NotDoclang(String),
    #[error("no path to save to")]
    NoPath,
}

pub struct Session {
    path: Option<PathBuf>,
    /// The original archive, kept so that a save carries its other parts
    /// over untouched. `None` for bare markup.
    archive: Option<Vec<u8>>,
    pages: Vec<archive::PagePart>,
    recordings: Vec<archive::Recording>,
    doc: Document,
    selected: Option<ElementId>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    /// The undo depth at the last save; `usize::MAX` once the saved state
    /// can no longer be reached by undo or redo.
    saved_at: usize,
    findings: Option<Vec<Finding>>,
}

impl Session {
    pub fn open(path: &Path) -> Result<Session, Error> {
        let bytes = fs::read(path)?;
        let loaded = archive::load_bytes(&bytes)?;
        let archive = (loaded.kind == SourceKind::Archive).then_some(bytes);
        let mut session = Session::from_loaded(loaded)?;
        session.path = Some(path.to_owned());
        session.archive = archive;
        Ok(session)
    }

    /// A session over markup already in memory, with nowhere to save until
    /// `save_to` names a place.
    pub fn from_markup(markup: &str) -> Result<Session, Error> {
        Session::from_loaded(archive::load_bytes(markup.as_bytes())?)
    }

    fn from_loaded(loaded: Loaded) -> Result<Session, Error> {
        let doc = Document::parse(&loaded.markup)?;
        if doclang::kind(&doc.root) != Some(Kind::Doclang) {
            return Err(Error::NotDoclang(doc.root.name().to_owned()));
        }
        Ok(Session {
            path: None,
            archive: None,
            pages: loaded.pages,
            recordings: loaded.recordings,
            doc,
            selected: None,
            undo: Vec::new(),
            redo: Vec::new(),
            saved_at: 0,
            findings: None,
        })
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The markup as it would be saved.
    pub fn markup(&self) -> String {
        self.doc.to_string()
    }

    pub fn recordings(&self) -> &[archive::Recording] {
        &self.recordings
    }

    pub fn recordings_of(&self, track: ElementId) -> Vec<&archive::Recording> {
        let Some(position) = self
            .doc
            .elements()
            .filter(|e| doclang::kind(e) == Some(Kind::Track))
            .position(|e| e.id() == track)
        else {
            return Vec::new();
        };
        self.recordings
            .iter()
            .filter(|r| r.track as usize == position + 1)
            .collect()
    }

    /// A part of the archive, for a front-end that wants a page image.
    pub fn part(&self, name: &str) -> Result<Option<Vec<u8>>, Error> {
        match &self.archive {
            Some(bytes) => Ok(archive::read_part(bytes, name)?),
            None => Ok(None),
        }
    }

    pub fn dirty(&self) -> bool {
        self.undo.len() != self.saved_at
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Validation findings, computed on first ask after each change.
    pub fn findings(&mut self) -> &[Finding] {
        if self.findings.is_none() {
            let mut findings = validate::validate(&self.doc);
            findings.extend(validate::recordings(&self.doc, &self.recordings));
            self.findings = Some(findings);
        }
        self.findings.as_deref().unwrap_or_default()
    }

    // -- views ----------------------------------------------------------

    pub fn view(&self) -> DocumentView {
        DocumentView {
            path: self.path.clone(),
            archive: self.archive.is_some(),
            version: doclang::version(&self.doc).map(str::to_owned),
            resolution: doclang::default_resolution(&self.doc),
            pages: doclang::pages(&self.doc).len(),
            dirty: self.dirty(),
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
        }
    }

    pub fn page_count(&self) -> usize {
        doclang::pages(&self.doc).len()
    }

    /// The view of page `number`, one-based, or `None` past the last page.
    pub fn page(&self, number: usize) -> Option<PageView> {
        let pages = doclang::pages(&self.doc);
        let page = pages.iter().find(|p| p.number == number)?;
        let resolution = doclang::default_resolution(&self.doc);
        let mut boxes = Vec::new();
        let mut rows = Vec::new();
        for node in &self.doc.root.children()[page.children.clone()] {
            if let Node::Element(el) = node {
                collect_views(el, 0, resolution, &mut boxes, &mut rows);
            }
        }
        Some(PageView {
            number,
            count: pages.len(),
            image: self
                .pages
                .iter()
                .find(|p| p.number as usize == number)
                .map(|p| p.name.clone()),
            boxes,
            rows,
        })
    }

    /// The blocks of page `number`, one-based, for a renderer.
    pub fn blocks(&self, number: usize) -> Option<Vec<Block>> {
        let pages = doclang::pages(&self.doc);
        let page = pages.iter().find(|p| p.number == number)?;
        Some(blocks::blocks_of(
            &self.doc.root.children()[page.children.clone()],
        ))
    }

    pub fn select(&mut self, id: Option<ElementId>) -> Result<(), CommandError> {
        if let Some(id) = id {
            if self.doc.find(id).is_none() {
                return Err(CommandError::NoSuchElement(id));
            }
        }
        self.selected = id;
        Ok(())
    }

    pub fn selected(&self) -> Option<ElementId> {
        self.selected.filter(|id| self.doc.find(*id).is_some())
    }

    /// The selected element in full, or `None` when nothing is selected.
    pub fn selection(&self) -> Option<ElementView> {
        self.element(self.selected()?)
    }

    /// Whether committing `text` to `target` would keep every formatting
    /// element that is there now.
    ///
    /// A front-end asks this before committing so that it can warn, and only
    /// when there is something to warn about. It answers `true` for a body
    /// that is already plain, for one whose tags all re-anchor, and for a
    /// target that does not exist — a command that is going to be refused is
    /// not a command that loses formatting.
    ///
    /// It is the same code the commit runs, not a second opinion about it:
    /// asking and doing must never disagree, so both go through [`rewrite`].
    pub fn keeps_formatting(&self, target: TextTarget, text: &str) -> bool {
        let Some((el, range)) = self.text_range(target) else {
            return true;
        };
        match rewrite(el.children(), range, text, whole_text(el)) {
            Ok((_, kept)) => kept,
            // Refused, so nothing is lost and there is nothing to ask about.
            // The command's own error is what the person sees, and it says
            // what is wrong with the markup - which a dialog offering to keep
            // the edit as plain text would not, and would offer to delete
            // every tag in the line as the remedy for a missing `>`.
            Err(_) => true,
        }
    }

    /// The text an edit field should start from, and whether it is markup.
    ///
    /// `Some` when the body carries formatting that can be spelled as tags:
    /// the field shows `The <bold>western approaches</bold> are wide` and
    /// what comes back is read as markup. `None` for a plain body, which
    /// edits as plain text — see [`inline::shows_tags`] for why everyone does
    /// not pay for the few lines that carry a tag.
    pub fn inline_text(&self, target: TextTarget) -> Option<String> {
        let (el, range) = self.text_range(target)?;
        let nodes = &el.children()[range];
        inline::shows_tags(nodes).then(|| shown_inline(nodes))
    }

    pub fn field_text(&self, target: TextTarget) -> Option<String> {
        let (el, range) = self.text_range(target)?;
        let nodes = &el.children()[range];
        Some(if inline::shows_tags(nodes) {
            shown_inline(nodes)
        } else {
            whitespace::read(nodes)
        })
    }

    /// What a text edit would have to put back, by name, in document order.
    ///
    /// For the warning: a person deciding whether to accept a flattening edit
    /// wants to know that it is a `bold` and an `italic` rather than being
    /// told that "formatting" will be lost. Comments and processing
    /// instructions are named too, because they are in the range and would go
    /// the same way.
    pub fn formatting_in(&self, target: TextTarget) -> Vec<String> {
        let Some((el, range)) = self.text_range(target) else {
            return Vec::new();
        };
        el.children()[range]
            .iter()
            .filter_map(|n| match n {
                Node::Element(e) => Some(e.name().to_owned()),
                Node::Comment(_) => Some("comment".to_owned()),
                Node::Pi { .. } => Some("processing instruction".to_owned()),
                Node::Text(_) | Node::CData(_) => None,
            })
            .collect()
    }

    /// The element and the range of its children that a text edit replaces.
    fn text_range(&self, target: TextTarget) -> Option<(&Element, std::ops::Range<usize>)> {
        match target {
            TextTarget::Body(id) => {
                let el = self.doc.find(id)?;
                doclang::kind(el)
                    .is_some_and(carries_text)
                    .then(|| (el, doclang::head(el).body))
            }
            TextTarget::Cell { id, row, col } => {
                let el = self.doc.find(id)?;
                let grid = Grid::parse(el);
                let cell = grid.at(row, col)?;
                let start = otsl::body_start(el, cell);
                Some((el, start..cell.content.end))
            }
            TextTarget::Item { id, item } => {
                let el = self.doc.find(id)?;
                if doclang::kind(el) != Some(Kind::List) {
                    return None;
                }
                let start = doclang::head(el).body.start;
                let range = blocks::item_bodies(el, start)
                    .into_iter()
                    .nth(item)
                    .map(|(_, range)| range)?;
                Some((el, range))
            }
            TextTarget::Turn { id, cue, turn } => {
                let el = self.doc.find(id)?;
                Some((el, turn_range(el, cue, turn)?))
            }
        }
    }

    pub fn element(&self, id: ElementId) -> Option<ElementView> {
        let (el, parent, path, index) = locate(&self.doc, id)?;
        let head = doclang::head(el);
        let resolution = doclang::default_resolution(&self.doc);
        let mut markup = String::new();
        el.write_to(&mut markup);
        let page = self.page_of(id).unwrap_or(1);
        Some(ElementView {
            id,
            path,
            name: el.name().to_owned(),
            kind: doclang::kind(el),
            page,
            attrs: el
                .attrs()
                .iter()
                .map(|a| (a.name.clone(), a.value.clone()))
                .collect(),
            label: head.label.map(str::to_owned),
            layer: head.layer.to_owned(),
            bounds: head
                .bounds
                .map(|b| [b.x_min.value, b.y_min.value, b.x_max.value, b.y_max.value]),
            rect: head.bounds.map(|b| b.fractions(resolution)),
            text: el.text(),
            body_text: whitespace::read(&el.children()[head.body.clone()]),
            editable_text: doclang::kind(el).is_some_and(carries_text),
            plain_text: is_plain_or_content(&el.children()[head.body.clone()]),
            markup,
            parent: parent.map(Element::id),
            index,
        })
    }

    /// The element at a path of the form `/doclang/text[2]/xref[1]`, as
    /// findings and views print it.
    pub fn find_by_path(&self, path: &str) -> Option<ElementId> {
        let mut steps = path.trim_start_matches('/').split('/');
        if steps.next()? != self.doc.root.name() {
            return None;
        }
        let mut el = &self.doc.root;
        for step in steps {
            let (name, index) = match step.split_once('[') {
                Some((name, rest)) => (name, rest.trim_end_matches(']').parse::<usize>().ok()?),
                None => (step, 1),
            };
            el = el
                .child_elements()
                .filter(|c| c.name() == name)
                .nth(index.checked_sub(1)?)?;
        }
        Some(el.id())
    }

    /// The page an element is on, one-based.
    pub fn page_of(&self, id: ElementId) -> Option<usize> {
        let (_, _, _, top) = locate(&self.doc, id)?;
        let _ = top;
        // Walk up to the root's child that contains `id`.
        let top_index = self.doc.root.children().iter().position(|n| match n {
            Node::Element(e) => e.id() == id || e.elements().any(|d| d.id() == id),
            _ => false,
        })?;
        doclang::pages(&self.doc)
            .into_iter()
            .find(|p| p.children.contains(&top_index))
            .map(|p| p.number)
    }

    // -- commands -------------------------------------------------------

    pub fn apply(&mut self, command: Command) -> Result<Applied, CommandError> {
        let (element, restore) = self.perform(&command)?;
        if self.undo.len() < self.saved_at {
            // Edited past the save point: the saved state is unreachable now.
            self.saved_at = usize::MAX;
        }
        self.redo.clear();
        self.undo.push(Entry {
            command: command.clone(),
            element,
            restore,
        });
        self.findings = None;
        Ok(Applied { command, element })
    }

    pub fn undo(&mut self) -> Option<Applied> {
        let entry = self.undo.pop()?;
        self.restore(entry.restore.clone());
        let applied = Applied {
            command: entry.command.clone(),
            element: entry.element,
        };
        self.redo.push(entry);
        self.findings = None;
        Some(applied)
    }

    pub fn redo(&mut self) -> Option<Result<Applied, CommandError>> {
        let entry = self.redo.pop()?;
        match self.perform(&entry.command) {
            Ok((element, restore)) => {
                self.undo.push(Entry {
                    command: entry.command.clone(),
                    element,
                    restore,
                });
                self.findings = None;
                Some(Ok(Applied {
                    command: entry.command,
                    element,
                }))
            }
            Err(e) => Some(Err(e)),
        }
    }

    fn restore(&mut self, restore: Restore) {
        match restore {
            Restore::Children { id, nodes } => {
                if let Some(el) = self.doc.find_mut(id) {
                    el.replace_children(nodes);
                }
            }
            Restore::Attr { id, name, value } => {
                if let Some(el) = self.doc.find_mut(id) {
                    match value {
                        Some(v) => el.set_attr(name, v),
                        None => {
                            el.remove_attr(&name);
                        }
                    }
                }
            }
            Restore::Name { id, name } => {
                if let Some(el) = self.doc.find_mut(id) {
                    el.set_name(name);
                }
            }
            Restore::Many(list) => {
                for r in list.into_iter().rev() {
                    self.restore(r);
                }
            }
        }
    }

    fn perform(&mut self, command: &Command) -> Result<(ElementId, Restore), CommandError> {
        match command {
            Command::SetText { id, text } => {
                if let Some(c) = text.chars().find(|c| !tree::is_xml_char(*c)) {
                    return Err(CommandError::NotXmlText(c as u32));
                }
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                match doclang::kind(el) {
                    Some(k) if carries_text(k) => {}
                    _ => return Err(CommandError::NotText(el.name().to_owned())),
                }
                let before = el.children().to_vec();
                let body = doclang::head(el).body;
                let whole = whole_text(el);
                let (nodes, _) = rewrite(&before, body, text, whole)?;
                el.replace_children(nodes);
                Ok((
                    *id,
                    Restore::Children {
                        id: *id,
                        nodes: before,
                    },
                ))
            }
            Command::SetAttr { id, name, value } => {
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                let old = el.attr(name).map(str::to_owned);
                match value {
                    Some(v) => el.set_attr(name.clone(), v.clone()),
                    None => {
                        el.remove_attr(name);
                    }
                }
                Ok((
                    *id,
                    Restore::Attr {
                        id: *id,
                        name: name.clone(),
                        value: old,
                    },
                ))
            }
            Command::SetLabel { id, value } => {
                self.set_head_property(*id, Kind::Label, "value", value.as_deref())
            }
            Command::SetLayer { id, value } => {
                self.set_head_property(*id, Kind::Layer, "value", value.as_deref())
            }
            Command::SetBounds { id, bounds } => {
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                let before = el.children().to_vec();
                let is_location = |n: &Node| matches!(n, Node::Element(e) if doclang::kind(e) == Some(Kind::Location));
                let first = before.iter().position(is_location);
                let last = before.iter().rposition(is_location);
                // The whitespace the file put between its locations, kept
                // for the new ones so the head keeps its shape.
                let separator = match (first, last) {
                    (Some(f), Some(l)) if l > f => match &before[f + 1] {
                        Node::Text(t) if t.value().trim().is_empty() => Some(t.value().to_owned()),
                        _ => None,
                    },
                    _ => None,
                };
                let mut nodes = before.clone();
                let at = match (first, last) {
                    (Some(f), Some(l)) => {
                        nodes.drain(f..=l);
                        f
                    }
                    _ => head_insert_index(&nodes, Kind::Location),
                };
                match bounds {
                    Some(values) => {
                        let mut fresh = Vec::new();
                        for (i, v) in values.iter().enumerate() {
                            if i > 0 {
                                if let Some(sep) = &separator {
                                    fresh.push(Node::Text(Text::new(sep.clone())));
                                }
                            }
                            let mut loc = Element::new("location");
                            loc.set_attr("value", v.to_string());
                            fresh.push(Node::Element(loc));
                        }
                        nodes.splice(at..at, fresh);
                    }
                    None => {
                        // The whitespace that led into the block would be a
                        // blank line without it.
                        if first.is_some() && at > 0 {
                            if let Node::Text(t) = &nodes[at - 1] {
                                if t.value().trim().is_empty() {
                                    nodes.remove(at - 1);
                                }
                            }
                        }
                    }
                }
                el.replace_children(nodes);
                Ok((
                    *id,
                    Restore::Children {
                        id: *id,
                        nodes: before,
                    },
                ))
            }
            Command::Rename { id, kind } => {
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                let old = el.name().to_owned();
                el.set_name(kind.name());
                Ok((*id, Restore::Name { id: *id, name: old }))
            }
            Command::SetCellText { id, row, col, text } => {
                if let Some(c) = text.chars().find(|c| !tree::is_xml_char(*c)) {
                    return Err(CommandError::NotXmlText(c as u32));
                }
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                let grid = Grid::parse(el);
                let cell = grid
                    .at(*row, *col)
                    .ok_or_else(|| CommandError::NoSuchCell(el.name().to_owned(), *row, *col))?;
                let start = otsl::body_start(el, cell);
                let range = start..cell.content.end;
                // A cell holding formatting used to be refused here, the way a
                // formatted paragraph was. Both now go through `rewrite`, which
                // keeps what it can and says when it could not, so that the
                // three text edits behave alike: an editor that lets a
                // paragraph be corrected and not the cell beside it is
                // arbitrary in a way nobody could predict.
                let before = el.children().to_vec();
                let (nodes, _) = rewrite(&before, range, text, false)?;
                el.replace_children(nodes);
                Ok((
                    *id,
                    Restore::Children {
                        id: *id,
                        nodes: before,
                    },
                ))
            }
            Command::SetListItemText { id, item, text } => {
                if let Some(c) = text.chars().find(|c| !tree::is_xml_char(*c)) {
                    return Err(CommandError::NotXmlText(c as u32));
                }
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                if doclang::kind(el) != Some(Kind::List) {
                    return Err(CommandError::NotText(el.name().to_owned()));
                }
                let body_start = doclang::head(el).body.start;
                let range = blocks::item_bodies(el, body_start)
                    .into_iter()
                    .nth(*item)
                    .map(|(_, range)| range)
                    .ok_or_else(|| CommandError::NoSuchListItem(el.name().to_owned(), *item))?;
                let before = el.children().to_vec();
                let (nodes, _) = rewrite(&before, range, text, false)?;
                el.replace_children(nodes);
                Ok((
                    *id,
                    Restore::Children {
                        id: *id,
                        nodes: before,
                    },
                ))
            }
            Command::SetTurnText {
                id,
                cue,
                turn,
                text,
            } => {
                if let Some(c) = text.chars().find(|c| !tree::is_xml_char(*c)) {
                    return Err(CommandError::NotXmlText(c as u32));
                }
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                let range = turn_range(el, *cue, *turn)
                    .ok_or_else(|| CommandError::NoSuchTurn(el.name().to_owned(), *cue, *turn))?;
                let before = el.children().to_vec();
                let (nodes, _) = rewrite(&before, range, text, false)?;
                el.replace_children(nodes);
                Ok((
                    *id,
                    Restore::Children {
                        id: *id,
                        nodes: before,
                    },
                ))
            }
            Command::SetCueTimes { id, cue, text } => {
                let (start, end) = parse_cue_times(text).map_err(CommandError::BadTime)?;
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                let parts = (doclang::kind(el) == Some(Kind::Track))
                    .then(|| blocks::cue_parts(el).into_iter().nth(*cue))
                    .flatten()
                    .ok_or_else(|| CommandError::NoSuchCue(el.name().to_owned(), *cue))?;
                let before = el.children().to_vec();
                el.replace_children(with_times(el, &parts, start, end));
                Ok((
                    *id,
                    Restore::Children {
                        id: *id,
                        nodes: before,
                    },
                ))
            }
            Command::SetCellKind { id, row, col, kind } => {
                let el = self
                    .doc
                    .find_mut(*id)
                    .ok_or(CommandError::NoSuchElement(*id))?;
                let grid = Grid::parse(el);
                let cell = grid
                    .at(*row, *col)
                    .ok_or_else(|| CommandError::NoSuchCell(el.name().to_owned(), *row, *col))?;
                let token = cell.token;
                let Some(Node::Element(tok)) = el.children_mut_at(token) else {
                    return Err(CommandError::NoSuchCell(el.name().to_owned(), *row, *col));
                };
                let old = tok.name().to_owned();
                let tok_id = tok.id();
                tok.set_name(kind.token().name());
                Ok((
                    *id,
                    Restore::Name {
                        id: tok_id,
                        name: old,
                    },
                ))
            }
            Command::Remove { id } => {
                if *id == self.doc.root.id() {
                    return Err(CommandError::Root);
                }
                let (_, parent, _, _) =
                    locate(&self.doc, *id).ok_or(CommandError::NoSuchElement(*id))?;
                let parent_id = parent.map(Element::id).ok_or(CommandError::Root)?;
                let parent = self.doc.find_mut(parent_id).expect("parent exists");
                let before = parent.children().to_vec();
                let at = before
                    .iter()
                    .position(|n| matches!(n, Node::Element(e) if e.id() == *id))
                    .expect("child exists");
                parent.remove(at);
                Ok((
                    *id,
                    Restore::Children {
                        id: parent_id,
                        nodes: before,
                    },
                ))
            }
            Command::Insert {
                parent,
                index,
                kind,
            } => {
                let parent_el = self
                    .doc
                    .find_mut(*parent)
                    .ok_or(CommandError::NoSuchElement(*parent))?;
                let before = parent_el.children().to_vec();
                let at = element_index_to_node_index(&before, *index).ok_or(
                    CommandError::IndexOutOfRange {
                        parent: parent_el.name().to_owned(),
                        index: *index,
                    },
                )?;
                let new = Element::new(kind.name());
                let new_id = new.id();
                parent_el.insert(at, Node::Element(new));
                Ok((
                    new_id,
                    Restore::Children {
                        id: *parent,
                        nodes: before,
                    },
                ))
            }
            Command::Move { id, parent, index } => {
                if *id == self.doc.root.id() {
                    return Err(CommandError::Root);
                }
                if *id == *parent
                    || self
                        .doc
                        .find(*id)
                        .is_some_and(|e| e.elements().any(|d| d.id() == *parent))
                {
                    return Err(CommandError::IntoItself);
                }
                let (_, old_parent, _, _) =
                    locate(&self.doc, *id).ok_or(CommandError::NoSuchElement(*id))?;
                let old_parent_id = old_parent.map(Element::id).ok_or(CommandError::Root)?;
                self.doc
                    .find(*parent)
                    .ok_or(CommandError::NoSuchElement(*parent))?;

                let old_parent = self.doc.find_mut(old_parent_id).expect("parent exists");
                let old_before = old_parent.children().to_vec();
                let at = old_before
                    .iter()
                    .position(|n| matches!(n, Node::Element(e) if e.id() == *id))
                    .expect("child exists");
                let node = old_parent.remove(at);

                let new_parent = self.doc.find_mut(*parent).expect("checked above");
                let new_before = new_parent.children().to_vec();
                let Some(to) = element_index_to_node_index(&new_before, *index) else {
                    // Put it back before refusing.
                    let old_parent = self.doc.find_mut(old_parent_id).expect("parent exists");
                    old_parent.replace_children(old_before);
                    return Err(CommandError::IndexOutOfRange {
                        parent: self
                            .doc
                            .find(*parent)
                            .map(|e| e.name().to_owned())
                            .unwrap_or_default(),
                        index: *index,
                    });
                };
                new_parent.insert(to, node);
                let restores = if old_parent_id == *parent {
                    vec![Restore::Children {
                        id: *parent,
                        nodes: old_before,
                    }]
                } else {
                    vec![
                        Restore::Children {
                            id: old_parent_id,
                            nodes: old_before,
                        },
                        Restore::Children {
                            id: *parent,
                            nodes: new_before,
                        },
                    ]
                };
                Ok((*id, Restore::Many(restores)))
            }
        }
    }

    /// Set or remove a single-valued head element such as `label` or
    /// `layer`, inserting it at its place in the head order when new.
    fn set_head_property(
        &mut self,
        id: ElementId,
        kind: Kind,
        attr: &str,
        value: Option<&str>,
    ) -> Result<(ElementId, Restore), CommandError> {
        let el = self
            .doc
            .find_mut(id)
            .ok_or(CommandError::NoSuchElement(id))?;
        let before = el.children().to_vec();
        let existing = before
            .iter()
            .position(|n| matches!(n, Node::Element(e) if doclang::kind(e) == Some(kind)));
        let mut nodes = before.clone();
        match (existing, value) {
            (Some(i), Some(v)) => {
                if let Node::Element(e) = &mut nodes[i] {
                    e.set_attr(attr, v);
                }
            }
            (Some(i), None) => {
                nodes.remove(i);
            }
            (None, Some(v)) => {
                let mut e = Element::new(kind.name());
                e.set_attr(attr, v);
                let at = head_insert_index(&nodes, kind);
                nodes.insert(at, Node::Element(e));
            }
            (None, None) => {}
        }
        el.replace_children(nodes);
        Ok((id, Restore::Children { id, nodes: before }))
    }

    // -- saving ---------------------------------------------------------

    /// Write back to where the document came from, if anything has changed
    /// since it was opened or last saved. A document with nothing to write
    /// is left alone: the bytes on disk are already what a write would
    /// produce, and a rewrite is not free on every platform, since under the
    /// macOS sandbox it marks the file with quarantine and gives it the
    /// process's group. Says whether it wrote.
    pub fn save(&mut self) -> Result<bool, Error> {
        let path = self.path.clone().ok_or(Error::NoPath)?;
        if !self.dirty() {
            return Ok(false);
        }
        self.save_to(&path)?;
        Ok(true)
    }

    /// Write to `path`, which becomes the session's path. An archive is
    /// repacked with every other part carried over; bare markup is written
    /// as is. The bytes replace the file whole or not at all, so a failure
    /// leaves the old file as it was; `replace` says how, and why macOS
    /// does it differently.
    pub fn save_to(&mut self, path: &Path) -> Result<(), Error> {
        let markup = self.markup();
        let bytes = match &self.archive {
            Some(original) => archive::repack(original, &markup)?,
            None => markup.into_bytes(),
        };
        replace::write(path, &bytes)?;
        if self.archive.is_some() {
            self.archive = Some(bytes);
        }
        self.path = Some(path.to_owned());
        self.saved_at = self.undo.len();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers

/// Kinds whose body is a run of text a person types, as opposed to
/// structure a person assembles.
fn carries_text(k: Kind) -> bool {
    matches!(
        k,
        Kind::Text
            | Kind::Heading
            | Kind::Caption
            | Kind::Footnote
            | Kind::PageHeader
            | Kind::PageFooter
            | Kind::Code
            | Kind::Formula
            | Kind::Key
            | Kind::Value
            | Kind::Hint
            | Kind::Marker
            | Kind::Description
            | Kind::Summary
            | Kind::FieldHeading
            | Kind::Content
            | Kind::Voice
            | Kind::Chapter
    )
}

type Time = [u64; 4];

fn parse_cue_times(text: &str) -> Result<(Time, Option<Time>), String> {
    let mut halves = text.split(['–', '—', '-']);
    let start = parse_time(halves.next().unwrap_or(""))?;
    let end = match halves.next().map(str::trim) {
        None | Some("") => None,
        Some(t) => Some(parse_time(t)?),
    };
    if halves.next().is_some() {
        return Err(format!("{text:?} is more than a start and an end time"));
    }
    Ok((start, end))
}

fn parse_time(text: &str) -> Result<Time, String> {
    let text = text.trim();
    let bad =
        || format!("{text:?} is not a time; write it as s, m:ss or h:mm:ss, with .mmm if needed");
    let (clock, fraction) = match text.split_once('.') {
        Some((c, f)) => (c, Some(f)),
        None => (text, None),
    };
    let number = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u64>().ok())
            .flatten()
            .ok_or_else(bad)
    };
    let fields: Vec<u64> = clock.split(':').map(number).collect::<Result<_, _>>()?;
    let [h, m, s] = match fields[..] {
        [s] => [0, 0, s],
        [m, s] => [0, m, s],
        [h, m, s] => [h, m, s],
        _ => return Err(bad()),
    };
    let ms = match fraction {
        None => 0,
        Some(f) if f.len() <= 3 => number(f)? * 10u64.pow(3 - f.len() as u32),
        Some(_) => return Err(bad()),
    };
    if m > 59 || s > 59 {
        return Err(format!("{text:?}: minutes and seconds run from 0 to 59"));
    }
    Ok([h, m, s, ms])
}

fn time_elements(value: Time, style: &[bool; 4]) -> Vec<Node> {
    const NAMES: [Kind; 4] = [Kind::Hours, Kind::Minutes, Kind::Seconds, Kind::Msecs];
    let [h, m, _, ms] = value;
    let wanted = [
        h > 0 || style[0],
        h > 0 || m > 0 || style[1],
        true,
        ms > 0 || style[3],
    ];
    (0..4)
        .filter(|&slot| wanted[slot])
        .map(|slot| {
            let mut el = Element::new(NAMES[slot].name());
            el.set_attr("value", value[slot].to_string());
            Node::Element(el)
        })
        .collect()
}

fn with_times(
    track: &Element,
    parts: &blocks::CueParts,
    start: Time,
    end: Option<Time>,
) -> Vec<Node> {
    let children = track.children();
    let runs = blocks::time_runs(track, &parts.times);
    let style_of = |run: Option<&Vec<usize>>| {
        let mut style = [false; 4];
        for &i in run.into_iter().flatten() {
            if let Node::Element(e) = &children[i] {
                if let Some(slot) = blocks::time_slot(doclang::kind(e)) {
                    style[slot] = true;
                }
            }
        }
        style
    };
    let start_style = style_of(runs.first());
    let end_style = if runs.len() > 1 {
        style_of(runs.get(1))
    } else {
        start_style
    };
    let mut replaced: Vec<Option<Vec<Node>>> = vec![None; children.len()];
    let first_of = |run: Option<&Vec<usize>>| run.and_then(|r| r.first().copied());
    let old: Vec<usize> = runs.iter().flatten().copied().collect();
    let start_at = first_of(runs.first()).unwrap_or(parts.bdiv + 1);
    let mut new_start = time_elements(start, &start_style);
    match (end, first_of(runs.get(1))) {
        (Some(end), Some(at)) => replaced[at] = Some(time_elements(end, &end_style)),
        (Some(end), None) => new_start.extend(time_elements(end, &end_style)),
        (None, _) => {}
    }
    let mut out = Vec::with_capacity(children.len() + 4);
    let mut start_written = false;
    for (i, node) in children.iter().enumerate() {
        if i == start_at && !start_written {
            out.extend(std::mem::take(&mut new_start));
            start_written = true;
        }
        if let Some(nodes) = replaced[i].take() {
            out.extend(nodes);
        } else if !old.contains(&i) {
            out.push(node.clone());
        }
    }
    if !start_written {
        out.extend(new_start);
    }
    out
}

fn turn_range(track: &Element, cue: usize, turn: usize) -> Option<std::ops::Range<usize>> {
    if doclang::kind(track) != Some(Kind::Track) {
        return None;
    }
    let parts = blocks::cue_parts(track).into_iter().nth(cue)?;
    blocks::turn_ranges(track, parts.transcript)
        .into_iter()
        .nth(turn)
        .map(|(_, range)| range)
}

/// `before` with the nodes in `range` replaced by one run of `text`. A
/// range that was text keeps its shape: the whitespace that framed it, and
/// CDATA if that is how the file carried it.
/// Replace `range` of `before` with `text`, keeping whatever formatting the
/// edit did not disturb. The `bool` is whether everything survived.
///
/// **This is the whole of the "retype a line" story and it is worth reading
/// once.** A body like `The <bold>western approaches</bold> are the waters`
/// is five sibling nodes, not one string with a bold range in it, and the
/// editor hands back one string. There is nothing in that string saying where
/// the tag went, so the obvious implementation — [`retext`] — writes one text
/// node and the tag is gone.
///
/// For a long time this was answered by refusing the edit, which protected
/// the tag by making the line uncorrectable. The first Windows walkthrough,
/// 2026-09-04, is what found that: a misread line in a formatted paragraph
/// could not be fixed in an editor whose whole purpose is fixing misread
/// lines.
///
/// So the tag is *re-anchored* instead. Each element in the old range is
/// looked for by its own text in what came back; where it is there exactly
/// once, the original node is put back around it, whole, so nested formatting
/// and every attribute survive untouched. A typo fixed three clauses away
/// costs nothing at all, which is the edit people actually make.
///
/// Where an element cannot be placed — its text was changed, or it now
/// appears twice and there is no way to know which one was meant — the answer
/// is the plain fallback and `false`, and the caller is expected to say so
/// before committing. Falling back quietly is the one thing this must not do:
/// that was the behaviour the refusal existed to prevent.
fn rewrite(
    before: &[Node],
    range: std::ops::Range<usize>,
    text: &str,
    whole: bool,
) -> Result<(Vec<Node>, bool), CommandError> {
    let old = &before[range.clone()];
    let shown = if inline::shows_tags(old) {
        shown_inline(old)
    } else {
        whitespace::read(old)
    };
    if shown == text {
        return Ok((before.to_vec(), true));
    }

    // Where the field showed tags, the string is markup and is read back
    // exactly. A malformed one is refused rather than fallen back on: falling
    // back would treat a missing `>` as an instruction to delete every tag in
    // the line, which is the opposite of what the person typing it meant.
    if inline::shows_tags(old) {
        let inner = encode_text_nodes(inline::from_inline(text)?);
        return Ok((splice(before, range, with_frame(old, inner)), true));
    }

    if is_plain_or_content(old) {
        let encoded = whitespace::encode(text, whole);
        if blocks::is_plain(old) && matches!(encoded[..], [Node::Text(_)]) {
            return Ok((retext(before, range, text), true));
        }
        return Ok((splice(before, range, with_frame(old, encoded)), true));
    }
    match reflow(old, text) {
        Some(inner) => Ok((splice(before, range, inner), true)),
        None => Ok((retext(before, range, text), false)),
    }
}

/// `before` with `range` replaced by `inner`.
fn splice(before: &[Node], range: std::ops::Range<usize>, inner: Vec<Node>) -> Vec<Node> {
    let mut nodes = before[..range.start].to_vec();
    nodes.extend(inner);
    nodes.extend_from_slice(&before[range.end..]);
    nodes
}

/// `inner` with the framing whitespace of `old` put back on the outside.
///
/// The frame is the file's shape and is kept exactly, so that a save diffs
/// against the original at the edit and nowhere else. It is merged
/// into a text node where there is one, so that no save gains an empty
/// sibling it did not have.
fn text_at(node: Option<&Node>) -> &str {
    match node {
        Some(Node::Text(t)) => t.value(),
        _ => "",
    }
}

fn with_frame(old: &[Node], mut inner: Vec<Node>) -> Vec<Node> {
    let first = text_at(old.first());
    let lead = &first[..first.len() - first.trim_start_matches(whitespace::is_space).len()];
    let last = text_at(old.last());
    let trail = if old.len() == 1 && lead.len() == first.len() {
        ""
    } else {
        &last[last.trim_end_matches(whitespace::is_space).len()..]
    };
    let (lead, trail) = (lead.to_owned(), trail.to_owned());
    if !lead.is_empty() {
        match inner.first_mut() {
            Some(Node::Text(t)) => t.set(format!("{lead}{}", t.value())),
            _ => inner.insert(0, Node::Text(Text::new(lead))),
        }
    }
    if !trail.is_empty() {
        match inner.last_mut() {
            Some(Node::Text(t)) => t.set(format!("{}{trail}", t.value())),
            _ => inner.push(Node::Text(Text::new(trail))),
        }
    }
    inner
}

/// `text` as nodes, with each element of `old` put back around its own text.
///
/// `None` when one cannot be placed, which is the signal to fall back.
fn reflow(old: &[Node], text: &str) -> Option<Vec<Node>> {
    let mut out: Vec<Node> = Vec::new();
    let mut rest = text;
    for node in old {
        match node {
            Node::Text(_) | Node::CData(_) => {}
            // A comment or a processing instruction has no text to anchor it
            // and no position the new string implies. Refusing is what makes
            // the warning honest: neither is a thing to drop in silence.
            Node::Comment(_) | Node::Pi { .. } => return None,
            Node::Element(_) => {
                let inner = whitespace::read(std::slice::from_ref(node));
                if inner.is_empty() {
                    return None;
                }
                let mut hits = rest.match_indices(inner.as_str());
                let (at, _) = hits.next()?;
                if hits.next().is_some() {
                    return None;
                }
                if at > 0 {
                    out.push(Node::Text(Text::new(rest[..at].to_owned())));
                }
                out.push(node.clone());
                rest = &rest[at + inner.len()..];
            }
        }
    }
    if !rest.is_empty() {
        out.push(Node::Text(Text::new(rest.to_owned())));
    }
    if out.is_empty() {
        return None;
    }
    Some(with_frame(old, encode_text_nodes(out)))
}

fn whole_text(el: &Element) -> bool {
    matches!(doclang::kind(el), Some(Kind::Code | Kind::Formula))
}

fn is_plain_or_content(nodes: &[Node]) -> bool {
    nodes.iter().all(|n| match n {
        Node::Text(_) | Node::CData(_) => true,
        Node::Element(e) => doclang::kind(e) == Some(Kind::Content),
        Node::Comment(_) | Node::Pi { .. } => false,
    })
}

fn shown_inline(nodes: &[Node]) -> String {
    whitespace::collapse(&inline::to_inline(nodes))
        .trim_matches(whitespace::is_space)
        .to_owned()
}

fn encode_text_nodes(nodes: Vec<Node>) -> Vec<Node> {
    let count = count_text_nodes(&nodes);
    let mut seen = 0;
    encode_in(nodes, count, &mut seen)
}

fn count_text_nodes(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(_) => 1,
            Node::Element(e) => count_text_nodes(e.children()),
            Node::CData(_) | Node::Comment(_) | Node::Pi { .. } => 0,
        })
        .sum()
}

fn encode_in(nodes: Vec<Node>, count: usize, seen: &mut usize) -> Vec<Node> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            Node::Text(t) => {
                let first = *seen == 0;
                *seen += 1;
                let last = *seen == count;
                out.extend(whitespace::encode_between(t.value(), false, first, last));
            }
            Node::Element(mut e) if doclang::kind(&e) != Some(Kind::Content) => {
                let children = encode_in(e.children().to_vec(), count, seen);
                e.replace_children(children);
                out.push(Node::Element(e));
            }
            other => out.push(other),
        }
    }
    out
}

fn retext(before: &[Node], range: std::ops::Range<usize>, text: &str) -> Vec<Node> {
    let old = &before[range.clone()];
    let textual = |n: &Node| matches!(n, Node::Text(_) | Node::CData(_));
    let (lead, trail, cdata) = if !old.is_empty() && old.iter().all(textual) {
        let joined: String = old
            .iter()
            .map(|n| match n {
                Node::Text(t) => t.value(),
                Node::CData(s) => s.as_str(),
                _ => "",
            })
            .collect();
        let (lead, trail) = frame(&joined);
        (lead, trail, old.iter().any(|n| matches!(n, Node::CData(_))))
    } else {
        (String::new(), String::new(), false)
    };
    let mut nodes = before[..range.start].to_vec();
    if cdata {
        if !lead.is_empty() {
            nodes.push(Node::Text(Text::new(lead)));
        }
        nodes.push(Node::CData(text.to_owned()));
        if !trail.is_empty() {
            nodes.push(Node::Text(Text::new(trail)));
        }
    } else {
        nodes.push(Node::Text(Text::new(format!("{lead}{text}{trail}"))));
    }
    nodes.extend_from_slice(&before[range.end..]);
    nodes
}

/// The leading and trailing whitespace of a text run.
fn frame(s: &str) -> (String, String) {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new());
    }
    let start = s.find(trimmed).unwrap_or(0);
    (s[..start].to_owned(), s[start + trimmed.len()..].to_owned())
}

/// Where a new head element of `kind` goes among `nodes`: after the last
/// head element that precedes it in the head order, else at the front. The
/// two repeatable kinds, locations and captions, go after their own kind.
fn head_insert_index(nodes: &[Node], kind: Kind) -> usize {
    let slot = kind.head_slot().unwrap_or(0);
    let mut at = 0;
    for (i, n) in nodes.iter().enumerate() {
        if let Node::Element(e) = n {
            match doclang::kind(e).and_then(Kind::head_slot) {
                Some(s)
                    if s <= slot
                        && (s != slot || matches!(kind, Kind::Location | Kind::Caption)) =>
                {
                    at = i + 1
                }
                Some(_) => break,
                None => break,
            }
        }
    }
    at
}

/// The node index at which an element would sit at `index` among the
/// element children, or `None` if `index` is past the end.
fn element_index_to_node_index(nodes: &[Node], index: usize) -> Option<usize> {
    let mut seen = 0;
    for (i, n) in nodes.iter().enumerate() {
        if seen == index {
            return Some(i);
        }
        if matches!(n, Node::Element(_)) {
            seen += 1;
        }
    }
    (seen == index).then_some(nodes.len())
}

/// An element with its parent, path and index among the parent's element
/// children.
fn locate(doc: &Document, id: ElementId) -> Option<(&Element, Option<&Element>, String, usize)> {
    fn go<'a>(
        el: &'a Element,
        parent: Option<&'a Element>,
        path: &str,
        index: usize,
        id: ElementId,
    ) -> Option<(&'a Element, Option<&'a Element>, String, usize)> {
        if el.id() == id {
            return Some((el, parent, path.to_owned(), index));
        }
        let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (i, child) in el.child_elements().enumerate() {
            let n = seen.entry(child.name()).or_insert(0);
            *n += 1;
            if let Some(found) = go(
                child,
                Some(el),
                &format!("{path}/{}[{n}]", child.name()),
                i,
                id,
            ) {
                return Some(found);
            }
        }
        None
    }
    go(&doc.root, None, &format!("/{}", doc.root.name()), 0, id)
}

fn collect_views(
    el: &Element,
    depth: usize,
    resolution: (u32, u32),
    boxes: &mut Vec<BoxView>,
    rows: &mut Vec<RowView>,
) {
    let Some(kind) = doclang::kind(el) else {
        return;
    };
    if !kind.is_semantic() {
        return;
    }
    let head = doclang::head(el);
    if let Some(b) = head.bounds {
        boxes.push(BoxView {
            id: el.id(),
            kind,
            rect: b.fractions(resolution),
        });
    }
    let detail = match kind {
        Kind::Heading | Kind::FieldHeading => el.attr("level").map(str::to_owned),
        Kind::List | Kind::Picture | Kind::Value => el.attr("class").map(str::to_owned),
        _ => None,
    };
    rows.push(RowView {
        id: el.id(),
        depth,
        kind,
        label: head.label.map(str::to_owned),
        detail,
        excerpt: excerpt(&body_text(el, head.body.clone())),
        located: head.bounds.is_some(),
        layer: head.layer.to_owned(),
    });
    for node in &el.children()[head.body] {
        if let Node::Element(child) = node {
            collect_views(child, depth + 1, resolution, boxes, rows);
        }
    }
}

/// The text of an element's body, head excluded.
fn body_text(el: &Element, body: std::ops::Range<usize>) -> String {
    let mut out = String::new();
    for node in &el.children()[body] {
        match node {
            Node::Element(e) => out.push_str(&e.text()),
            Node::Text(t) => out.push_str(t.value()),
            Node::CData(s) => out.push_str(s),
            _ => {}
        }
    }
    out
}

fn excerpt(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out = String::new();
    for w in words {
        if out.len() + w.len() > 60 {
            out.push('…');
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

impl Element {
    /// Every element under this one, in document order, itself excluded.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        let mut pending: Vec<&Element> = self
            .child_elements()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        std::iter::from_fn(move || {
            let el = pending.pop()?;
            pending.extend(el.child_elements().collect::<Vec<_>>().into_iter().rev());
            Some(el)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<doclang>\n  <heading level=\"2\">\n    <location value=\"10\"/><location value=\"20\"/><location value=\"30\"/><location value=\"40\"/>\n    Title\n  </heading>\n  <text>Body &amp; soul</text>\n  <page_break/>\n  <list><ldiv/>one<ldiv/>two</list>\n</doclang>\n";

    fn session() -> Session {
        Session::from_markup(DOC).unwrap()
    }

    fn id_at(s: &Session, path: &str) -> ElementId {
        s.find_by_path(path).unwrap()
    }

    #[test]
    fn pages_boxes_and_rows() {
        let s = session();
        assert_eq!(s.page_count(), 2);
        let p1 = s.page(1).unwrap();
        assert_eq!(p1.boxes.len(), 1);
        assert_eq!(
            p1.boxes[0].rect,
            [10.0 / 512.0, 20.0 / 512.0, 30.0 / 512.0, 40.0 / 512.0]
        );
        let kinds: Vec<Kind> = p1.rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, [Kind::Heading, Kind::Text]);
        assert_eq!(p1.rows[0].detail.as_deref(), Some("2"));
        assert_eq!(p1.rows[0].excerpt, "Title");
        assert_eq!(p1.rows[1].excerpt, "Body & soul");
        let p2 = s.page(2).unwrap();
        assert_eq!(p2.rows.len(), 1);
        assert!(s.page(3).is_none());
        assert_eq!(s.page_of(id_at(&s, "/doclang/list[1]")), Some(2));
    }

    #[test]
    fn every_command_undoes_to_the_same_bytes() {
        let mut s = session();
        let heading = id_at(&s, "/doclang/heading[1]");
        let text = id_at(&s, "/doclang/text[1]");
        let list = id_at(&s, "/doclang/list[1]");
        let root = s.document().root.id();
        let commands = vec![
            Command::SetText {
                id: heading,
                text: "New <title>".into(),
            },
            Command::SetAttr {
                id: heading,
                name: "level".into(),
                value: Some("3".into()),
            },
            Command::SetAttr {
                id: heading,
                name: "level".into(),
                value: None,
            },
            Command::SetLabel {
                id: text,
                value: Some("body".into()),
            },
            Command::SetLayer {
                id: text,
                value: Some("furniture".into()),
            },
            Command::SetBounds {
                id: text,
                bounds: Some([1, 2, 3, 4]),
            },
            Command::SetBounds {
                id: heading,
                bounds: None,
            },
            Command::Rename {
                id: text,
                kind: Kind::Footnote,
            },
            Command::Move {
                id: text,
                parent: root,
                index: 0,
            },
            Command::Insert {
                parent: root,
                index: 1,
                kind: Kind::Text,
            },
            Command::Remove { id: list },
        ];
        let n = commands.len();
        for c in commands {
            s.apply(c).unwrap();
        }
        assert_ne!(s.markup(), DOC);
        assert!(s.dirty());
        for _ in 0..n {
            assert!(s.undo().is_some());
        }
        assert_eq!(s.markup(), DOC);
        assert!(!s.dirty());
        for _ in 0..n {
            s.redo().unwrap().unwrap();
        }
        assert!(s.dirty());
        assert!(s.redo().is_none());
    }

    #[test]
    fn set_text_keeps_head_and_framing() {
        let mut s = session();
        let heading = id_at(&s, "/doclang/heading[1]");
        s.apply(Command::SetText {
            id: heading,
            text: "Revised".into(),
        })
        .unwrap();
        let view = s.element(heading).unwrap();
        assert_eq!(view.bounds, Some([10, 20, 30, 40]));
        assert_eq!(view.text.trim(), "Revised");
        assert!(
            view.markup.ends_with("\n    Revised\n  </heading>"),
            "{}",
            view.markup
        );
        assert_eq!(
            s.apply(Command::SetText {
                id: id_at(&s, "/doclang/list[1]"),
                text: "x".into()
            }),
            Err(CommandError::NotText("list".into()))
        );
        assert_eq!(
            s.apply(Command::SetText {
                id: heading,
                text: "a\u{0}b".into()
            }),
            Err(CommandError::NotXmlText(0))
        );
    }

    #[test]
    fn box_and_text_edits_keep_the_file_shape() {
        let src = "<doclang>\n  <text>\n    <location value=\"54\"/>\n    <location value=\"153\"/>\n    <location value=\"236\"/>\n    <location value=\"274\"/>\n<![CDATA[We introduce <Docling>.]]>  </text>\n</doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        s.apply(Command::SetBounds {
            id: text,
            bounds: Some([1, 2, 3, 4]),
        })
        .unwrap();
        s.apply(Command::SetText {
            id: text,
            text: "Revised <text>".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>\n    <location value=\"1\"/>\n    <location value=\"2\"/>\n    <location value=\"3\"/>\n    <location value=\"4\"/>\n<![CDATA[Revised <text>]]>  </text>"
        );
        s.apply(Command::SetBounds {
            id: text,
            bounds: None,
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>\n<![CDATA[Revised <text>]]>  </text>"
        );
        for _ in 0..3 {
            s.undo();
        }
        assert_eq!(s.markup(), src);
    }

    /// A list item is a range between `ldiv` separators, not an element, so
    /// this is the only edit whose target is a position.
    #[test]
    fn list_item_text_edits_by_position() {
        let mut s = session();
        let list = id_at(&s, "/doclang/list[1]");

        s.apply(Command::SetListItemText {
            id: list,
            item: 1,
            text: "the second, retyped".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(list).unwrap().markup,
            "<list><ldiv/>one<ldiv/>the second, retyped</list>"
        );

        // The other item is untouched, which is the whole point of addressing
        // a range rather than the list's body.
        s.apply(Command::SetListItemText {
            id: list,
            item: 0,
            text: "the first".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(list).unwrap().markup,
            "<list><ldiv/>the first<ldiv/>the second, retyped</list>"
        );

        s.undo();
        s.undo();
        assert_eq!(s.markup(), DOC);
    }

    #[test]
    fn a_list_item_past_the_end_is_refused() {
        let mut s = session();
        let list = id_at(&s, "/doclang/list[1]");
        assert_eq!(
            s.apply(Command::SetListItemText {
                id: list,
                item: 2,
                text: "third".into(),
            }),
            Err(CommandError::NoSuchListItem("list".into(), 2))
        );
        // And a target that is not a list at all.
        let text = id_at(&s, "/doclang/text[1]");
        assert_eq!(
            s.apply(Command::SetListItemText {
                id: text,
                item: 0,
                text: "x".into(),
            }),
            Err(CommandError::NotText("text".into()))
        );
        assert_eq!(s.markup(), DOC);
    }

    /// The edit people actually make: a word fixed somewhere else in the
    /// line. The field shows the tags, so what comes back carries them.
    #[test]
    fn formatted_text_edits_as_markup() {
        let src = "<doclang>\n  <text>The <bold>western approaches</bold> are the watrs west of the <italic>channel</italic>.</text>\n</doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        let target = TextTarget::Body(text);

        // The field starts from the markup, not from the flattened words.
        assert_eq!(
            s.inline_text(target).unwrap(),
            "The <bold>western approaches</bold> are the watrs west of the <italic>channel</italic>."
        );

        let fixed =
            "The <bold>western approaches</bold> are the waters west of the <italic>channel</italic>.";
        assert!(s.keeps_formatting(target, fixed));
        s.apply(Command::SetText {
            id: text,
            text: fixed.into(),
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>The <bold>western approaches</bold> are the waters west of the <italic>channel</italic>.</text>"
        );
        s.undo();
        assert_eq!(s.markup(), src);
    }

    /// Nested formatting is spelled and read back exactly, so an edit around
    /// it costs nothing.
    #[test]
    fn nested_formatting_survives_an_edit_around_it() {
        let src =
            "<doclang><text>Plain <bold>bold <italic>both</italic></bold> and more</text></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        s.apply(Command::SetText {
            id: text,
            text: "Plain <bold>bold <italic>both</italic></bold> and MORE".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>Plain <bold>bold <italic>both</italic></bold> and MORE</text>"
        );
        s.undo();
        assert_eq!(s.markup(), src);
    }

    /// The field showed the tags, so typing without them means removing them.
    /// That is an instruction rather than an accident, and nothing asks.
    #[test]
    fn typing_over_the_tags_removes_them_deliberately() {
        let src = "<doclang><text>The <bold>western approaches</bold> are wide</text></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        let plain = "The Western Approaches are wide";
        assert!(s.keeps_formatting(TextTarget::Body(text), plain));
        s.apply(Command::SetText {
            id: text,
            text: plain.into(),
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>The Western Approaches are wide</text>"
        );
        s.undo();
        assert_eq!(s.markup(), src);
    }

    /// Formatting can be added where the field shows tags, which no amount of
    /// matching the words up afterwards could ever do.
    #[test]
    fn formatting_can_be_added() {
        let src = "<doclang><text>one <bold>two</bold> three</text></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        s.apply(Command::SetText {
            id: text,
            text: "one <bold>two</bold> <italic>three</italic>".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>one <bold>two</bold> <italic>three</italic></text>"
        );
    }

    /// A malformed or unknown tag is refused and the document is untouched.
    /// Falling back to plain here would read a missing `>` as an instruction
    /// to delete every tag in the line.
    #[test]
    fn bad_markup_is_refused_and_changes_nothing() {
        let src = "<doclang><text>one <bold>two</bold> three</text></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        for bad in [
            "one <bold>two three",
            "one <blink>two</blink> three",
            "one <bold class=\"x\">two</bold> three",
        ] {
            let e = s
                .apply(Command::SetText {
                    id: text,
                    text: bad.into(),
                })
                .unwrap_err();
            assert!(matches!(e, CommandError::Inline(_)), "{bad}: {e:?}");
        }
        assert_eq!(s.markup(), src);
        assert!(!s.dirty());

        // And a refusal is not a loss of formatting: a window that asked
        // "keep this as plain text?" here would be offering to delete every
        // tag in the line as the remedy for a missing `>`.
        assert!(s.keeps_formatting(TextTarget::Body(text), "one <bold>two three"));
    }

    /// The frame is the file's shape: a save diffs against the original at
    /// the edit and nowhere else.
    #[test]
    fn editing_markup_keeps_the_framing_whitespace() {
        let src = "<doclang><text>\n    Lead <bold>bold</bold> trail\n  </text></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        assert_eq!(
            s.inline_text(TextTarget::Body(text)).unwrap(),
            "Lead <bold>bold</bold> trail"
        );
        s.apply(Command::SetText {
            id: text,
            text: "Lead <bold>bold</bold> TRAIL".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>\n    Lead <bold>bold</bold> TRAIL\n  </text>"
        );
    }

    /// A plain body has no tags to show, so its field stays plain and a
    /// literal angle bracket needs no escaping from the person typing it.
    #[test]
    fn a_plain_body_edits_as_plain_text() {
        let mut s = session();
        let text = id_at(&s, "/doclang/text[1]");
        assert!(s.inline_text(TextTarget::Body(text)).is_none());
        s.apply(Command::SetText {
            id: text,
            text: "a < b & c".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(text).unwrap().markup,
            "<text>a &lt; b &amp; c</text>"
        );
        s.undo();
        assert_eq!(s.markup(), DOC);
    }

    // ---- what cannot be spelled as tags, and so still re-anchors ----

    /// A body carrying something with no inline spelling keeps the older
    /// behaviour: put back what can be found, and say so when it cannot.
    #[test]
    fn a_body_with_no_inline_spelling_re_anchors() {
        let src = "<doclang><table><fcel/>one <text>two</text> three<nl/></table></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let table = id_at(&s, "/doclang/table[1]");
        let target = TextTarget::Cell {
            id: table,
            row: 0,
            col: 0,
        };
        // `<text>` is not formatting, so no field would show it as a tag.
        assert!(s.inline_text(target).is_none());

        // Its text is still there, so it is put back whole.
        assert!(s.keeps_formatting(target, "one two THREE"));
        s.apply(Command::SetCellText {
            id: table,
            row: 0,
            col: 0,
            text: "one two THREE".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(table).unwrap().markup,
            "<table><fcel/>one <text>two</text> THREE<nl/></table>"
        );
        s.undo();
        assert_eq!(s.markup(), src);
    }

    /// Two occurrences and no way to know which was meant.
    #[test]
    fn an_ambiguous_anchor_falls_back_rather_than_guessing() {
        let src = "<doclang><text>a <xref thread_id=\"1\"/>tide b</text></doclang>";
        let s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        assert!(s.inline_text(TextTarget::Body(text)).is_none());
        assert!(!s.keeps_formatting(TextTarget::Body(text), "a tide and a tide"));
    }

    /// A comment has no text to anchor it and no position the new string
    /// implies, so it is a fallback rather than something to drop in silence.
    #[test]
    fn a_comment_in_the_body_refuses_to_re_anchor() {
        let src = "<doclang><text>before<!-- why -->after</text></doclang>";
        let s = Session::from_markup(src).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        assert!(s.inline_text(TextTarget::Body(text)).is_none());
        assert!(!s.keeps_formatting(TextTarget::Body(text), "before and after"));
        assert!(s.keeps_formatting(TextTarget::Body(text), "beforeafter"));
    }

    /// Asking and doing must never disagree, so both go through `rewrite`.
    #[test]
    fn keeps_formatting_is_true_for_plain_bodies_and_absent_targets() {
        let mut s = session();
        let text = id_at(&s, "/doclang/text[1]");
        let list = id_at(&s, "/doclang/list[1]");
        assert!(s.keeps_formatting(TextTarget::Body(text), "anything at all"));
        assert!(s.keeps_formatting(TextTarget::Item { id: list, item: 0 }, "anything"));
        // Past the end: the command will be refused, and a refusal is not a
        // loss of formatting.
        assert!(s.keeps_formatting(TextTarget::Item { id: list, item: 9 }, "x"));
        // A structural target `SetText` would refuse.
        assert!(s.keeps_formatting(TextTarget::Body(list), "x"));
        s.apply(Command::SetText {
            id: text,
            text: "plain".into(),
        })
        .unwrap();
        s.undo();
        assert_eq!(s.markup(), DOC);
    }

    #[test]
    fn cell_edits_address_the_grid() {
        let src = "<doclang><table>\n  <ched/>A<ched/>B<nl/>\n  <fcel/>1<fcel/><text>two</text><nl/>\n</table></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let table = id_at(&s, "/doclang/table[1]");
        s.apply(Command::SetCellText {
            id: table,
            row: 0,
            col: 1,
            text: "Bee".into(),
        })
        .unwrap();
        s.apply(Command::SetCellKind {
            id: table,
            row: 1,
            col: 0,
            kind: CellKind::RowHeader,
        })
        .unwrap();
        assert_eq!(
            s.element(table).unwrap().markup,
            "<table>\n  <ched/>A<ched/>Bee<nl/>\n  <rhed/>1<fcel/><text>two</text><nl/>\n</table>"
        );
        // A cell holding an element used to be refused here. It is now edited
        // like anything else, with `keeps_formatting` answering first so the
        // window can say what the commit is about to discard - the `<text>`
        // wrapper, in this case, since "two" is nowhere in "x" to anchor it.
        let inner = TextTarget::Cell {
            id: table,
            row: 1,
            col: 1,
        };
        assert!(!s.keeps_formatting(inner, "x"));
        assert!(s.keeps_formatting(inner, "two and more"));
        s.apply(Command::SetCellText {
            id: table,
            row: 1,
            col: 1,
            text: "x".into(),
        })
        .unwrap();
        assert_eq!(
            s.element(table).unwrap().markup,
            "<table>\n  <ched/>A<ched/>Bee<nl/>\n  <rhed/>1<fcel/>x<nl/>\n</table>"
        );
        s.undo();
        assert!(matches!(
            s.apply(Command::SetCellText {
                id: table,
                row: 5,
                col: 0,
                text: "x".into()
            }),
            Err(CommandError::NoSuchCell(..))
        ));
        s.undo();
        s.undo();
        assert_eq!(s.markup(), src);
        let blocks = s.blocks(1).unwrap();
        assert!(matches!(
            blocks[0],
            Block::Table {
                rows: 2,
                cols: 2,
                ..
            }
        ));
    }

    #[test]
    fn head_properties_land_in_order() {
        let mut s = session();
        let text = id_at(&s, "/doclang/text[1]");
        s.apply(Command::SetBounds {
            id: text,
            bounds: Some([1, 2, 3, 4]),
        })
        .unwrap();
        s.apply(Command::SetLayer {
            id: text,
            value: Some("background".into()),
        })
        .unwrap();
        s.apply(Command::SetLabel {
            id: text,
            value: Some("note".into()),
        })
        .unwrap();
        let m = s.element(text).unwrap().markup;
        assert_eq!(m, "<text><label value=\"note\"/><layer value=\"background\"/><location value=\"1\"/><location value=\"2\"/><location value=\"3\"/><location value=\"4\"/>Body &amp; soul</text>");
        assert!(!s.findings().iter().any(|f| f.is_defect()));
        s.apply(Command::SetLabel {
            id: text,
            value: None,
        })
        .unwrap();
        assert!(s.element(text).unwrap().label.is_none());
    }

    #[test]
    fn selection_survives_edits_and_dies_with_its_element() {
        let mut s = session();
        let text = id_at(&s, "/doclang/text[1]");
        s.select(Some(text)).unwrap();
        s.apply(Command::SetText {
            id: text,
            text: "x".into(),
        })
        .unwrap();
        assert_eq!(s.selection().unwrap().id, text);
        s.apply(Command::Remove { id: text }).unwrap();
        assert!(s.selection().is_none());
        assert_eq!(s.select(Some(text)), Err(CommandError::NoSuchElement(text)));
        assert_eq!(
            s.apply(Command::Remove {
                id: s.document().root.id()
            }),
            Err(CommandError::Root)
        );
    }

    #[test]
    fn saved_mark_tracks_undo_and_redo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.dclg");
        std::fs::write(&path, DOC).unwrap();
        let mut s = Session::open(&path).unwrap();
        let text = id_at(&s, "/doclang/text[1]");
        s.apply(Command::SetText {
            id: text,
            text: "one".into(),
        })
        .unwrap();
        s.save().unwrap();
        assert!(!s.dirty());
        s.undo();
        assert!(s.dirty());
        s.redo();
        assert!(!s.dirty());
        s.undo();
        s.apply(Command::SetText {
            id: text,
            text: "two".into(),
        })
        .unwrap();
        assert!(s.dirty());
        s.undo();
        assert!(
            s.dirty(),
            "the saved state is unreachable after a divergent edit"
        );
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("<text>one</text>"));
    }

    #[test]
    fn save_with_nothing_changed_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.dclg");
        std::fs::write(&path, DOC).unwrap();
        let mut s = Session::open(&path).unwrap();
        // Something a write would replace, put there behind the session's
        // back so the test can tell a skipped write from an identical one.
        std::fs::write(&path, "untouched").unwrap();
        assert!(!s.save().unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "untouched");

        let text = id_at(&s, "/doclang/text[1]");
        s.apply(Command::SetText {
            id: text,
            text: "one".into(),
        })
        .unwrap();
        assert!(s.save().unwrap());
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("<text>one</text>"));

        s.undo();
        s.redo();
        assert!(!s.dirty());
        std::fs::write(&path, "untouched").unwrap();
        assert!(!s.save().unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "untouched");
    }

    #[test]
    fn find_by_path_and_views() {
        let s = session();
        let v = s.element(id_at(&s, "/doclang/text[1]")).unwrap();
        assert_eq!(v.path, "/doclang/text[1]");
        assert_eq!(v.index, 1);
        assert_eq!(v.page, 1);
        assert!(s.find_by_path("/doclang/text[2]").is_none());
        assert!(s.find_by_path("/html").is_none());
        let d = s.view();
        assert_eq!((d.pages, d.dirty, d.archive), (2, false, false));
    }

    fn edited(src: &str, path: &str, typed: &str) -> (String, String) {
        let mut s = Session::from_markup(src).unwrap();
        let id = id_at(&s, path);
        s.apply(Command::SetText {
            id,
            text: typed.to_owned(),
        })
        .unwrap();
        let saved = s.markup();
        let reopened = Session::from_markup(&saved).unwrap();
        let read = reopened.element(id_at(&reopened, path)).unwrap().body_text;
        (saved, read)
    }

    #[test]
    fn typed_whitespace_survives_a_save_and_reopen() {
        let src = "<doclang><text>\n  Old words\n</text></doclang>";
        for typed in ["a line\nbreak", "two  spaces", " leading space"] {
            let (saved, read) = edited(src, "/doclang/text[1]", typed);
            assert_eq!(read, typed, "{saved}");
        }
        let (saved, _) = edited(src, "/doclang/text[1]", "new words");
        assert_eq!(saved, "<doclang><text>\n  new words\n</text></doclang>");
    }

    #[test]
    fn an_edit_that_changes_no_words_changes_nothing() {
        let src = "<doclang><text>\n  Some   words\n  across lines <bold>and</bold> a tag\n</text><code><content>fn a() {\n    b();\n}</content></code></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        for path in ["/doclang/text[1]", "/doclang/code[1]"] {
            let id = id_at(&s, path);
            let shown = s
                .inline_text(TextTarget::Body(id))
                .unwrap_or_else(|| s.element(id).unwrap().body_text);
            s.apply(Command::SetText { id, text: shown }).unwrap();
        }
        assert_eq!(s.markup(), src);
    }

    #[test]
    fn a_formatted_edit_gains_no_content_for_ordinary_spaces() {
        let (saved, _) = edited(
            "<doclang><text>The <bold>western</bold> approaches</text></doclang>",
            "/doclang/text[1]",
            "The <bold>western</bold> wide approaches",
        );
        assert_eq!(
            saved,
            "<doclang><text>The <bold>western</bold> wide approaches</text></doclang>"
        );
    }

    #[test]
    fn code_keeps_its_lines_in_one_content() {
        let (saved, read) = edited(
            "<doclang><code>x</code></doclang>",
            "/doclang/code[1]",
            "fn a() {\n    b();\n}",
        );
        assert_eq!(
            saved,
            "<doclang><code><content>fn a() {\n    b();\n}</content></code></doclang>"
        );
        assert_eq!(read, "fn a() {\n    b();\n}");
    }

    #[test]
    fn a_turn_edits_between_its_speaker_and_the_next() {
        let src = "<doclang><track><bdiv/><seconds value=\"2\"/><voice>John</voice> Hi Mary! <voice>Mary</voice> Hello John!</track></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let track = id_at(&s, "/doclang/track[1]");
        assert_eq!(
            s.field_text(TextTarget::Turn {
                id: track,
                cue: 0,
                turn: 1
            })
            .as_deref(),
            Some("Hello John!")
        );
        s.apply(Command::SetTurnText {
            id: track,
            cue: 0,
            turn: 0,
            text: "Hi there, Mary!".into(),
        })
        .unwrap();
        assert_eq!(
            s.markup(),
            "<doclang><track><bdiv/><seconds value=\"2\"/><voice>John</voice> Hi there, Mary! <voice>Mary</voice> Hello John!</track></doclang>"
        );
        assert!(s
            .apply(Command::SetTurnText {
                id: track,
                cue: 0,
                turn: 2,
                text: "x".into(),
            })
            .is_err());
        let voice = id_at(&s, "/doclang/track[1]/voice[2]");
        s.apply(Command::SetText {
            id: voice,
            text: "Maria".into(),
        })
        .unwrap();
        assert!(s.markup().contains("<voice>Maria</voice>"));
    }

    #[test]
    fn cue_times_are_set_added_and_removed_in_the_files_own_components() {
        let src = "<doclang version=\"0.9\"><track><bdiv/><minutes value=\"0\"/><seconds value=\"2\"/> Hi. <bdiv/><minutes value=\"0\"/><seconds value=\"9\"/> Bye.</track></doclang>";
        let mut s = Session::from_markup(src).unwrap();
        let track = id_at(&s, "/doclang/track[1]");
        let set = |s: &mut Session, cue, text: &str| {
            s.apply(Command::SetCueTimes {
                id: track,
                cue,
                text: text.to_owned(),
            })
        };
        set(&mut s, 0, "0:03 – 0:05.5").unwrap();
        assert!(s.markup().contains("<bdiv/><minutes value=\"0\"/><seconds value=\"3\"/><minutes value=\"0\"/><seconds value=\"5\"/><msecs value=\"500\"/> Hi. "));
        set(&mut s, 0, "0:03").unwrap();
        assert!(s
            .markup()
            .contains("<bdiv/><minutes value=\"0\"/><seconds value=\"3\"/> Hi. "));
        set(&mut s, 1, "1:02:03").unwrap();
        assert!(s.markup().contains(
            "<bdiv/><hours value=\"1\"/><minutes value=\"2\"/><seconds value=\"3\"/> Bye."
        ));
        assert!(matches!(
            set(&mut s, 1, "0:75"),
            Err(CommandError::BadTime(_))
        ));
        assert!(matches!(
            set(&mut s, 1, "soon"),
            Err(CommandError::BadTime(_))
        ));
        assert!(!s
            .findings()
            .iter()
            .any(|f| f.rule == "track-cue-block-sequence"));
        set(&mut s, 1, "0:01").unwrap();
        assert!(s
            .findings()
            .iter()
            .any(|f| f.rule == "track-cue-block-sequence"));
        for _ in 0..4 {
            s.undo();
        }
        assert_eq!(s.markup(), src);
    }
}
