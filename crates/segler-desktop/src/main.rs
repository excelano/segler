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

#![deny(unsafe_code)]

mod document;
mod inspector;
mod page;
mod system_theme;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Modifiers, ViewportCommand};
use segler_core::session::{Command, Session};
use segler_core::tree::ElementId;

use document::{DocumentPane, Selection};
use inspector::{Editor, Requests};
use page::{PagePane, Pick};

/// Reverse-DNS on macOS, the desktop entry's basename on Linux, the window
/// class a Wayland compositor matches an icon against.
const APP_ID: &str = "segler-desktop";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_title("Segler")
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    eframe::run_native(
        "Segler",
        options,
        Box::new(move |cc| {
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

    fn apply(&mut self, command: Command) {
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
            ui.separator();
            let has_scan = self
                .session
                .as_ref()
                .and_then(|s| s.page(self.page_number))
                .is_some_and(|p| p.image.is_some());
            ui.add_enabled_ui(has_scan, |ui| {
                ui.toggle_value(&mut self.show_scan, "Scan")
                    .on_hover_text("Show the page scan beside the document");
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        match self.dialog {
            Dialog::None => {}
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

        egui::Panel::left("structure")
            .default_size(300.0)
            .resizable(true)
            .show(ui, |ui| match &page_view {
                Some(page) => inspector::structure(ui, page, selected, scroll, &mut requests),
                None => {
                    ui.heading("Structure");
                    ui.weak("Open a DocLang document or archive.");
                }
            });

        egui::Panel::right("element")
            .default_size(360.0)
            .resizable(true)
            .show(ui, |ui| {
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
