//! Segler, the desktop application. Presented to a person as **Segler**; the
//! crate and the binary are `segler-desktop`, so nothing on `PATH` collides
//! with the command-line tool.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![deny(unsafe_code)]

use std::path::{Path, PathBuf};

use eframe::egui;
use segler_core::{archive, summary::Summary};

/// Reverse-DNS on macOS, the desktop entry's basename on Linux, the window
/// class a Wayland compositor matches an icon against.
const APP_ID: &str = "segler-desktop";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_title("Segler")
            .with_inner_size([960.0, 720.0]),
        ..Default::default()
    };
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    eframe::run_native(
        "Segler",
        options,
        Box::new(move |_cc| Ok(Box::new(App::start(path)))),
    )
}

/// What the window knows about the document it has open.
struct Opened {
    path: PathBuf,
    loaded: archive::Loaded,
    summary: Summary,
}

#[derive(Default)]
struct App {
    opened: Option<Opened>,
    error: Option<String>,
}

impl App {
    fn start(path: Option<PathBuf>) -> Self {
        let mut app = Self::default();
        if let Some(path) = path {
            app.open(&path);
        }
        app
    }

    fn open(&mut self, path: &Path) {
        let result = archive::load(path)
            .map_err(|e| e.to_string())
            .and_then(|loaded| {
                Summary::of(&loaded.markup)
                    .map(|summary| Opened {
                        path: path.to_owned(),
                        loaded,
                        summary,
                    })
                    .map_err(|e| e.to_string())
            });
        match result {
            Ok(opened) => {
                self.opened = Some(opened);
                self.error = None;
            }
            Err(message) => self.error = Some(format!("{}: {message}", path.display())),
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
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // egui 0.36 folded `TopBottomPanel` and `SidePanel` into one `Panel`.
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open…").clicked() {
                    self.pick_file();
                }
                if let Some(opened) = &self.opened {
                    ui.label(opened.path.display().to_string());
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
                return;
            }
            let Some(opened) = &self.opened else {
                ui.label("Open a DocLang document or archive.");
                return;
            };
            summary_grid(ui, opened);
        });
    }
}

fn summary_grid(ui: &mut egui::Ui, opened: &Opened) {
    let s = &opened.summary;
    egui::Grid::new("summary").num_columns(2).show(ui, |ui| {
        ui.label("Kind");
        ui.label(match opened.loaded.kind {
            archive::Kind::Markup => "markup",
            archive::Kind::Archive => "archive",
        });
        ui.end_row();
        ui.label("Version");
        ui.label(&s.version);
        ui.end_row();
        ui.label("Pages");
        ui.label(s.pages.to_string());
        ui.end_row();
        if opened.loaded.kind == archive::Kind::Archive {
            ui.label("Page images");
            ui.label(opened.loaded.pages.len().to_string());
            ui.end_row();
            ui.label("Assets");
            ui.label(opened.loaded.assets.len().to_string());
            ui.end_row();
        }
        ui.label("Located elements");
        ui.label(s.located.to_string());
        ui.end_row();
    });

    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("elements")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for (name, count) in &s.elements {
                    ui.monospace(name);
                    ui.label(count.to_string());
                    ui.end_row();
                }
            });
    });
}
