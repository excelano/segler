//! Segler, the desktop application. Presented to a person as **Segler**; the
//! crate and the binary are `segler-desktop`, so nothing on `PATH` collides
//! with the command-line tool.
//!
//! One window, three panes and a problems list, one selection. The page
//! pane draws the image and its boxes, the structure pane the tree, the
//! element pane the selected element with its controls, and clicking in any
//! of them selects in all of them. Everything the window shows is a
//! view-model from `segler_core::session`, and everything it changes goes
//! back as a command; the window holds no document state of its own.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

// `deny` rather than `forbid`, and the difference is the whole of the
// exception. `forbid` cannot be lifted anywhere beneath it, and receiving a
// document from macOS needs one Objective-C method, which cannot be written
// without `unsafe`. Every module below is still denied; `opened_document` is
// the single `allow`, and `segler-core`, where documents are actually read and
// written, keeps `forbid` untouched. `CLAUDE.md` says what the exception costs.
#![deny(unsafe_code)]
// Windows creates a console for a console-subsystem process, and a file manager
// launching this one is not attached to a terminal, so a double-clicked
// document would open a black console window behind the application.
// slipcase-desktop found that by looking at the first frame Windows ever drew
// of it, and `packaging/windows/build-msix.ps1` refuses a binary that lacks
// this rather than wait to be told again. The attribute is ignored everywhere
// else, and it is off in a debug build because that is where a panic message
// still has somewhere to go.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod document;
mod inspector;
// The one exception to `deny(unsafe_code)` above, and the only module in this
// application that writes `unsafe`. macOS is the only platform of the three
// that does not deliver a double-clicked document as `argv[1]`.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod opened_document;
mod page;
mod system_theme;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Modifiers, ViewportCommand};
use segler_core::session::{Command, Session, TextTarget};
use segler_core::tree::ElementId;

use document::{DocumentPane, Selection};
use inspector::{Editor, Requests};
use page::{PagePane, Pick};

/// Reverse-DNS on macOS, the desktop entry's basename on Linux, the window
/// class a Wayland compositor matches an icon against.
const APP_ID: &str = "segler-desktop";

/// The window's icon on Windows, which has no `.desktop` entry to find one in.
///
/// `APP_ID` above is how Linux answers this question and it does nothing here:
/// `with_app_id` is Wayland's `xdg_toplevel.set_app_id`, and neither egui,
/// eframe nor winit turns it into anything on Windows. Windows takes a window's
/// icon from a resource compiled into the executable, and compiling one needs
/// `rc.exe` or `windres`, which `DESIGN.md` §5 keeps out of the build. So the
/// icon is carried as bytes and handed to the window at run time, which needs
/// no build step at all. slipcase-desktop measured all of this; the file is
/// built from the same drawing every platform's icon comes from.
#[cfg(target_os = "windows")]
const WINDOW_ICON: &[u8] = include_bytes!("../../../packaging/windows/segler.ico");

/// The icon at the largest size the drawing carries without being upscaled.
///
/// A window gets one image and Windows scales it to 16 in the title bar and 32
/// in the task bar, doubling both at 200%. 64 is a whole multiple of those
/// four, so each is an integer downsample of the same drawing. It is not a
/// whole multiple of what the intermediate scalings ask for - 125% wants 20 and
/// 40, 150% wants 24 and 48 - and those are resampled; slipcase-desktop looked
/// at both and the cost is nothing a person notices, which is why 64 stays the
/// choice: it is the largest entry no scaling has to enlarge.
#[cfg(target_os = "windows")]
fn window_icon() -> Option<egui::IconData> {
    let directory = ico::IconDir::read(std::io::Cursor::new(WINDOW_ICON)).ok()?;
    let entry = directory.entries().iter().find(|e| e.width() == 64)?;
    let image = entry.decode().ok()?;
    Some(egui::IconData {
        rgba: image.rgba_data().to_vec(),
        width: image.width(),
        height: image.height(),
    })
}

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_app_id(APP_ID)
        .with_title("Segler")
        .with_inner_size([1280.0, 860.0])
        .with_min_inner_size([800.0, 500.0]);

    // Shadowed rather than made mutable, so that no platform without an icon to
    // set carries an unused `mut`.
    #[cfg(target_os = "windows")]
    let viewport = match window_icon() {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    };

    // On macOS the bundle's `.icns` is the icon, and eframe has to be told
    // not to override it. `epi_integration.rs` substitutes its own egui logo
    // for any viewport that names no icon, and `app_icon.rs` hands that to
    // `-[NSApplication setApplicationIconImage:]`, which outranks the bundle.
    // David saw the egui logo on the Dock during the walkthrough of
    // 2026-09-06, as he had on slipcase-desktop's; Launch Services and Finder
    // resolved the right drawing throughout, which is why only looking at the
    // Dock finds it. An empty `IconData` declines the icon rather than
    // replacing it: eframe turns one into `None` and sets nothing.
    #[cfg(target_os = "macos")]
    let viewport = viewport.with_icon(egui::IconData::default());

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let path = std::env::args_os().nth(1).map(PathBuf::from);

    // Before `eframe`, because macOS dispatches the document that launched this
    // application before `eframe`'s creation closure is reached, and AppKit's
    // own handler refuses it there. slipcase-desktop measured both ways:
    // registering later opened a document double-clicked into a running window
    // and lost the one that started it.
    #[cfg(target_os = "macos")]
    opened_document::watch();

    eframe::run_native(
        "Segler",
        options,
        Box::new(move |cc| {
            // After AppKit has installed its own handler for this event, and so
            // late that the window exists to be woken. The other two platforms
            // read the path out of `argv` above and never reach this.
            #[cfg(target_os = "macos")]
            opened_document::wake_with(&cc.egui_ctx);

            system_theme::follow(&cc.egui_ctx);
            Ok(Box::new(App::start(path)))
        }),
    )
}

/// Which modal, if any, is up.
enum Dialog {
    None,
    /// Confirm removing an element. Undo brings it back, and the confirm is
    /// still asked, because a keystroke on the wrong row is easy.
    Remove(ElementId),
    /// The window was asked to close with unsaved changes.
    Close,
    /// A text edit that would discard formatting the core could not put back.
    /// Holds the command, so confirming applies exactly what was asked for.
    Flatten(Command),
}

/// The text a command is about to write, and where, for the three commands
/// that replace a run of nodes with one string. `None` for everything else,
/// which has no formatting to lose.
fn flattening_target(command: &Command) -> Option<(TextTarget, &str)> {
    match command {
        Command::SetText { id, text } => Some((TextTarget::Body(*id), text)),
        Command::SetCellText { id, row, col, text } => Some((
            TextTarget::Cell {
                id: *id,
                row: *row,
                col: *col,
            },
            text,
        )),
        Command::SetListItemText { id, item, text } => Some((
            TextTarget::Item {
                id: *id,
                item: *item,
            },
            text,
        )),
        _ => None,
    }
}

struct App {
    session: Option<Session>,
    page_number: usize,
    /// The zoom factor, or `None` to fit the page to the pane's width,
    /// which is how a document opens.
    zoom: Option<f32>,
    /// The factor the page pane used last frame, for the toolbar.
    zoom_shown: f32,
    pane: PagePane,
    document: DocumentPane,
    editor: Editor,
    /// A selected table cell, as (table, row, column).
    selected_cell: Option<(ElementId, usize, usize)>,
    /// Whether the page scan is shown beside the document.
    show_scan: bool,
    show_structure: bool,
    show_element: bool,
    /// Pictures whose inner rows are open in the structure pane.
    expanded: std::collections::HashSet<ElementId>,
    dialog: Dialog,
    /// Set when a selection came from somewhere other than the structure
    /// pane, so that the tree scrolls to it once.
    scroll_to_selection: bool,
    /// Whether the next close request may proceed.
    allow_close: bool,
    status: String,
    last_title: String,
}

impl App {
    fn start(path: Option<PathBuf>) -> Self {
        let mut app = App {
            session: None,
            page_number: 1,
            zoom: None,
            zoom_shown: 1.0,
            pane: PagePane::default(),
            document: DocumentPane::default(),
            editor: Editor::default(),
            selected_cell: None,
            show_scan: false,
            show_structure: true,
            show_element: true,
            expanded: std::collections::HashSet::new(),
            dialog: Dialog::None,
            scroll_to_selection: false,
            allow_close: false,
            status: String::new(),
            last_title: String::new(),
        };
        if let Some(path) = path {
            app.open(&path);
        }
        app
    }

    fn open(&mut self, path: &Path) {
        match Session::open(path) {
            Ok(session) => {
                self.session = Some(session);
                self.page_number = 1;
                self.zoom = None;
                self.pane.forget();
                self.document.forget();
                self.selected_cell = None;
                self.expanded.clear();
                self.editor = Editor::default();
                self.status = format!("Opened {}", path.display());
            }
            Err(e) => self.status = format!("{}: {e}", path.display()),
        }
    }

    fn pick_file(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("DocLang", &["dclg", "dclx", "xml"])
            .pick_file();
        if let Some(path) = picked {
            self.open(&path);
        }
    }

    fn save(&mut self) -> bool {
        let Some(session) = &mut self.session else {
            return false;
        };
        let result = if session.path().is_some() {
            session.save()
        } else {
            match rfd::FileDialog::new()
                .add_filter("DocLang", &["dclg", "dclx"])
                .save_file()
            {
                Some(path) => session.save_to(&path),
                None => return false,
            }
        };
        match result {
            Ok(()) => {
                self.status = format!(
                    "Saved {}",
                    session
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                );
                true
            }
            Err(e) => {
                self.status = format!("Save failed: {e}");
                false
            }
        }
    }

    /// Apply a command, or stop first if it would discard formatting.
    ///
    /// One gate for both panes rather than a flag threaded through each. A
    /// text edit replaces a run of nodes with one string, so a tag inside it
    /// survives only where the core can find its text again in what was
    /// typed; where it cannot, this asks before doing it. Undo would put it
    /// back either way — the confirm is here because a person who has just
    /// corrected one word will not think to look.
    ///
    /// It asks nothing at all for the ordinary edit. `keeps_formatting` is
    /// true for plain text and for every tag that re-anchors, which is most
    /// of them, and a warning that fires on every commit is one nobody reads.
    fn apply(&mut self, command: Command) {
        let Some(session) = &self.session else {
            return;
        };
        if let Some(target) = flattening_target(&command) {
            let (target, text) = target;
            if !session.keeps_formatting(target, text) {
                self.dialog = Dialog::Flatten(command);
                return;
            }
        }
        self.apply_now(command);
    }

    /// Apply without asking. The confirm path and every non-text command.
    fn apply_now(&mut self, command: Command) {
        let Some(session) = &mut self.session else {
            return;
        };
        match session.apply(command) {
            Ok(applied) => {
                self.status = describe(&applied.command);
                self.editor = Editor::default();
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    fn undo(&mut self) {
        if let Some(session) = &mut self.session {
            self.status = match session.undo() {
                Some(a) => format!("Undid: {}", describe(&a.command)),
                None => "Nothing to undo".to_owned(),
            };
            self.editor = Editor::default();
        }
    }

    fn redo(&mut self) {
        if let Some(session) = &mut self.session {
            self.status = match session.redo() {
                Some(Ok(a)) => format!("Redid: {}", describe(&a.command)),
                Some(Err(e)) => e.to_string(),
                None => "Nothing to redo".to_owned(),
            };
            self.editor = Editor::default();
        }
    }

    fn go_to_page(&mut self, number: usize) {
        let count = self.session.as_ref().map_or(1, Session::page_count);
        self.page_number = number.clamp(1, count.max(1));
    }

    fn select(&mut self, id: Option<ElementId>, scroll: bool) {
        if let Some(session) = &mut self.session {
            if session.select(id).is_ok() {
                self.scroll_to_selection = scroll;
                if self.selected_cell.is_some_and(|(t, ..)| Some(t) != id) {
                    self.selected_cell = None;
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        if !typing && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.document.cancel_edit();
        }
        let (structure, element) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::Num1),
                i.consume_key(Modifiers::COMMAND, Key::Num2),
            )
        });
        if structure {
            self.show_structure = !self.show_structure;
        }
        if element {
            self.show_element = !self.show_element;
        }
        let (open, save, undo, redo, prev, next, delete) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                    || i.consume_key(Modifiers::COMMAND, Key::Y),
                !typing && i.consume_key(Modifiers::NONE, Key::PageUp),
                !typing && i.consume_key(Modifiers::NONE, Key::PageDown),
                !typing && i.consume_key(Modifiers::NONE, Key::Delete),
            )
        });
        if open {
            self.pick_file();
        }
        if save {
            self.save();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if prev {
            self.go_to_page(self.page_number.saturating_sub(1));
        }
        if next {
            self.go_to_page(self.page_number + 1);
        }
        if delete {
            if let Some(id) = self.session.as_ref().and_then(Session::selected) {
                self.dialog = Dialog::Remove(id);
            }
        }
    }

    fn title(&self) -> String {
        match &self.session {
            Some(s) => {
                let name = s
                    .path()
                    .and_then(Path::file_name)
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".to_owned());
                format!("{}{name} — Segler", if s.dirty() { "• " } else { "" })
            }
            None => "Segler".to_owned(),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open…").clicked() {
                self.pick_file();
            }
            let has_doc = self.session.is_some();
            let dirty = self.session.as_ref().is_some_and(Session::dirty);
            if ui
                .add_enabled(has_doc && dirty, egui::Button::new("Save"))
                .clicked()
            {
                self.save();
            }
            ui.separator();
            let can_undo = self.session.as_ref().is_some_and(Session::can_undo);
            let can_redo = self.session.as_ref().is_some_and(Session::can_redo);
            if ui
                .add_enabled(can_undo, egui::Button::new("Undo"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(can_redo, egui::Button::new("Redo"))
                .clicked()
            {
                self.redo();
            }
            ui.separator();
            let count = self.session.as_ref().map_or(0, Session::page_count);
            if ui
                .add_enabled(self.page_number > 1, egui::Button::new("◀"))
                .clicked()
            {
                self.go_to_page(self.page_number - 1);
            }
            let mut n = self.page_number;
            if ui
                .add_enabled(
                    count > 0,
                    egui::DragValue::new(&mut n).range(1..=count.max(1)),
                )
                .changed()
            {
                self.go_to_page(n);
            }
            ui.label(format!("of {count}"));
            if ui
                .add_enabled(self.page_number < count, egui::Button::new("▶"))
                .clicked()
            {
                self.go_to_page(self.page_number + 1);
            }
            let has_scan = self
                .session
                .as_ref()
                .and_then(|s| s.page(self.page_number))
                .is_some_and(|p| p.image.is_some());

            // Zoom belongs to the page image and to nothing else, so it is
            // here only while there is a page image on screen. It used to be
            // here always: four controls that looked operable, drove the panel
            // that was not drawn, and reported a percentage of nothing - which
            // is every `.dclg` and every archive with the panel closed, so the
            // usual state of the window was four dead controls. Found at
            // David's keyboard on 2026-09-04, running CHECKLIST item 7.
            //
            // Hidden rather than disabled. A greyed slider still asks to be
            // read, and the answer would be that it applies to a pane that is
            // not there; the pane comes and goes and its controls can too.
            if self.show_scan && has_scan {
                ui.separator();
                ui.label("Zoom");
                let mut z = self.zoom.unwrap_or(self.zoom_shown);
                if ui
                    .add(
                        egui::Slider::new(&mut z, 0.1..=4.0)
                            .logarithmic(true)
                            .show_value(false),
                    )
                    .changed()
                {
                    self.zoom = Some(z);
                }
                ui.label(format!("{:.0}%", self.zoom_shown * 100.0));
                if ui.small_button("Fit").clicked() {
                    self.zoom = None;
                }
                if ui.small_button("100%").clicked() {
                    self.zoom = Some(1.0);
                }
            }

            ui.separator();
            ui.add_enabled_ui(has_scan, |ui| {
                ui.toggle_value(&mut self.show_scan, "Page image")
                    .on_hover_text("Show the page image from the archive beside the document");
            });
            ui.separator();
            ui.toggle_value(&mut self.show_structure, "Structure")
                .on_hover_text("Show the structure pane (Ctrl+1)");
            ui.toggle_value(&mut self.show_element, "Element")
                .on_hover_text("Show the element pane (Ctrl+2)");
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        // Taken first and by reference, because this variant carries the
        // command rather than an id and the arms below match by value.
        if let Dialog::Flatten(command) = &self.dialog {
            let command = command.clone();
            let lost = flattening_target(&command)
                .map(|(target, _)| {
                    self.session
                        .as_ref()
                        .map(|s| s.formatting_in(target))
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let names: Vec<String> = lost.iter().map(|n| format!("<{n}>")).collect();
            // "it" for one and "them" for two is worth the line: the message
            // names the real tags, and naming them and then getting the number
            // wrong reads like a message nobody looked at.
            let (what, them) = match names.len() {
                0 => ("Formatting in this text".to_owned(), "it"),
                1 => (names[0].clone(), "it"),
                _ => (names.join(", "), "them"),
            };
            let mut next = None;
            egui::Modal::new(egui::Id::new("flatten")).show(ctx, |ui| {
                ui.heading("Keep this edit as plain text?");
                // "Keeping" rather than "saving": the flattening happens when
                // the edit is applied, and the document in the window is what
                // changes. A save writes whatever is there by then.
                ui.label(format!(
                    "{what} could not be matched to what you typed, so keeping this \
                     edit will write the line as plain text without {them}."
                ));
                ui.add_space(4.0);
                ui.weak(
                    "This happens when the formatted words themselves changed, or when \
                     they now appear more than once and there is no way to tell which \
                     was meant. Undo restores everything.",
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Keep as plain text").clicked()
                        || ui.input(|i| i.key_pressed(Key::Enter))
                    {
                        next = Some(true);
                    }
                    if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                        next = Some(false);
                    }
                });
            });
            if let Some(confirmed) = next {
                self.dialog = Dialog::None;
                if confirmed {
                    self.apply_now(command);
                } else {
                    // The element pane's buffers reload only when the
                    // selection changes, so a cancelled edit would otherwise
                    // sit in the Text box saying something the document does
                    // not - and fire the same refused command again the next
                    // time that box lost focus. `apply_now` clears them for
                    // the same reason; declining has to as well.
                    self.editor = Editor::default();
                    self.document.cancel_edit();
                    self.status = "Edit discarded; the formatting is unchanged".to_owned();
                }
            }
            return;
        }
        match self.dialog {
            // Handled above, by reference, because it carries a command.
            Dialog::Flatten(_) | Dialog::None => {}
            Dialog::Remove(id) => {
                let what = self
                    .session
                    .as_ref()
                    .and_then(|s| s.element(id))
                    .map(|v| format!("<{}> at {}", v.name, v.path))
                    .unwrap_or_else(|| "this element".to_owned());
                let mut next = None;
                egui::Modal::new(egui::Id::new("remove")).show(ctx, |ui| {
                    ui.heading("Remove element?");
                    ui.label(format!(
                        "{what} and everything inside it will be removed. Undo brings it back."
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Remove").clicked() || ui.input(|i| i.key_pressed(Key::Enter))
                        {
                            next = Some(true);
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                        {
                            next = Some(false);
                        }
                    });
                });
                if let Some(confirmed) = next {
                    self.dialog = Dialog::None;
                    if confirmed {
                        self.apply(Command::Remove { id });
                    }
                }
            }
            Dialog::Close => {
                let mut next = None;
                egui::Modal::new(egui::Id::new("close")).show(ctx, |ui| {
                    ui.heading("Save changes?");
                    ui.label("The document has unsaved changes.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save and close").clicked() {
                            next = Some(if self.save() { Some(true) } else { None });
                        }
                        if ui.button("Discard").clicked() {
                            next = Some(Some(false));
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                        {
                            next = Some(None);
                        }
                    });
                });
                if let Some(outcome) = next {
                    self.dialog = Dialog::None;
                    if outcome.is_some() {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.session.as_ref().is_some_and(Session::dirty)
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.dialog = Dialog::Close;
        }

        if matches!(self.dialog, Dialog::None) {
            self.shortcuts(&ctx);
        }

        // A document double-clicked in Finder, which arrives as an Apple Event
        // rather than as an argument. The same as choosing it in the dialog:
        // Cmd+O replaces what is open without asking either, and a document
        // that arrives while a modal is up waits for it, since `taken`
        // consumes and asking early would lose it rather than defer it.
        #[cfg(target_os = "macos")]
        if matches!(self.dialog, Dialog::None) {
            if let Some(path) = opened_document::taken() {
                self.open(&path);
            }
        }

        let title = self.title();
        if title != self.last_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        // egui 0.36 folded `TopBottomPanel` and `SidePanel` into one `Panel`.
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.weak(&self.status);
            });
        });

        let mut requests = Requests::default();
        let page_view = self.session.as_ref().and_then(|s| s.page(self.page_number));
        let selected = self.session.as_ref().and_then(Session::selected);
        let scroll = std::mem::take(&mut self.scroll_to_selection);

        let mut show_structure = self.show_structure;
        egui::Panel::left("structure")
            .default_size(300.0)
            .resizable(true)
            .show_collapsible(ui, &mut show_structure, |ui| match &page_view {
                Some(page) => inspector::structure(
                    ui,
                    page,
                    selected,
                    scroll,
                    &mut self.expanded,
                    &mut requests,
                ),
                None => {
                    ui.heading("Structure");
                    ui.weak("Open a DocLang document or archive.");
                }
            });

        let mut show_element = self.show_element;
        egui::Panel::right("element")
            .default_size(360.0)
            .resizable(true)
            .show_collapsible(ui, &mut show_element, |ui| {
                let view = self.session.as_ref().and_then(Session::selection);
                if let Some(session) = &self.session {
                    self.editor.show(
                        ui,
                        session,
                        view.as_ref(),
                        self.selected_cell,
                        &mut requests,
                    );
                }
            });

        egui::Panel::bottom("problems")
            .default_size(160.0)
            .resizable(true)
            .show(ui, |ui| {
                if let Some(session) = &mut self.session {
                    let findings = session.findings().to_vec();
                    inspector::problems(ui, session, &findings, &mut requests);
                }
            });

        let mut doc_actions = document::Actions::default();
        egui::CentralPanel::default().show(ui, |ui| match (&self.session, &page_view) {
            (Some(session), Some(page)) => {
                if self.show_scan && page.image.is_some() {
                    egui::Panel::right("scan")
                        .default_size(ui.available_width() * 0.45)
                        .min_size(160.0)
                        .resizable(true)
                        .show(ui, |ui| {
                            let (pick, used) =
                                self.pane.show(ui, session, page, self.zoom, selected);
                            self.zoom_shown = used;
                            if let Pick::Select(id) = pick {
                                requests.select = Some(id);
                                self.scroll_to_selection = true;
                            }
                        });
                }
                let blocks = session.blocks(page.number).unwrap_or_default();
                doc_actions = self.document.show(
                    ui,
                    session,
                    &blocks,
                    Selection {
                        element: selected,
                        cell: self.selected_cell,
                    },
                    scroll,
                );
            }
            _ => {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a DocLang document or archive (Ctrl+O).");
                });
            }
        });

        if let Some(id) = doc_actions.select {
            requests.select = Some(id);
            self.scroll_to_selection = true;
        }
        if let Some(cell) = doc_actions.select_cell {
            self.selected_cell = Some(cell);
        }
        requests.commands.extend(doc_actions.commands);

        if let Some(page) = requests.go_to_page {
            self.go_to_page(page);
        }
        if let Some(id) = requests.select {
            let scroll = self.scroll_to_selection || requests.go_to_page.is_some();
            self.select(id, scroll);
        }
        for command in requests.commands {
            self.apply(command);
        }
        if let Some(id) = requests.remove {
            self.dialog = Dialog::Remove(id);
        }

        self.dialogs(&ctx);
    }
}

fn describe(command: &Command) -> String {
    match command {
        Command::SetText { .. } => "Text changed".into(),
        Command::SetAttr {
            name,
            value: Some(v),
            ..
        } => format!("{name} set to {v}"),
        Command::SetAttr {
            name, value: None, ..
        } => format!("{name} removed"),
        Command::SetLabel { value: Some(v), .. } => format!("Label set to {v}"),
        Command::SetLabel { value: None, .. } => "Label removed".into(),
        Command::SetLayer { value, .. } => {
            format!("Layer set to {}", value.as_deref().unwrap_or("body"))
        }
        Command::SetBounds {
            bounds: Some(_), ..
        } => "Box changed".into(),
        Command::SetBounds { bounds: None, .. } => "Box removed".into(),
        Command::Rename { kind, .. } => format!("Changed to {}", kind.name()),
        Command::Move { .. } => "Moved".into(),
        Command::Insert { kind, .. } => format!("Inserted {}", kind.name()),
        Command::Remove { .. } => "Element removed".into(),
        Command::SetListItemText { item, .. } => format!("List item {} changed", item + 1),
        Command::SetCellText { row, col, .. } => {
            format!("Cell row {}, column {} changed", row + 1, col + 1)
        }
        Command::SetCellKind { row, col, kind, .. } => format!(
            "Cell row {}, column {} is now {}",
            row + 1,
            col + 1,
            document::cell_kind_name(*kind)
        ),
    }
}
