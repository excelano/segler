//! The structure pane, the element editor, and the problems list. Each
//! reads a view-model from the session and hands back commands; none of
//! them touches the document.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use eframe::egui::{self, Align};
use segler_core::doclang::Kind;
use segler_core::otsl::{CellKind, Grid};
use segler_core::session::{Command, ElementView, PageView, Session};
use segler_core::tree::ElementId;
use segler_core::validate::Finding;

use crate::document::cell_kind_name;
use crate::page::color_for;

/// What a pane asked for this frame.
#[derive(Default)]
pub struct Requests {
    pub select: Option<Option<ElementId>>,
    pub commands: Vec<Command>,
    pub remove: Option<ElementId>,
    pub go_to_page: Option<usize>,
}

/// The structure tree of the current page.
pub fn structure(
    ui: &mut egui::Ui,
    page: &PageView,
    selected: Option<ElementId>,
    scroll_to_selection: bool,
    out: &mut Requests,
) {
    ui.heading("Structure");
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("structure")
        .show(ui, |ui| {
            if page.rows.is_empty() {
                ui.weak("Nothing on this page.");
            }
            for row in &page.rows {
                let is_selected = selected == Some(row.id);
                let mut tag = row.kind.name().to_owned();
                if let Some(d) = &row.detail {
                    tag.push_str(&format!(" {d}"));
                }
                let text = egui::RichText::new(&tag)
                    .color(color_for(row.kind))
                    .strong();
                ui.horizontal(|ui| {
                    ui.add_space(12.0 * row.depth as f32);
                    let response = ui.selectable_label(is_selected, text);
                    let excerpt = ui.add(
                        egui::Label::new(egui::RichText::new(&row.excerpt).weak())
                            .truncate()
                            .sense(egui::Sense::click()),
                    );
                    if response.clicked() || excerpt.clicked() {
                        out.select = Some(Some(row.id));
                    }
                    if is_selected
                        && scroll_to_selection
                        && !ui.clip_rect().contains_rect(response.rect)
                    {
                        // A jump, not an animation: the animated scroll renders the
                        // tree at fractional offsets for a few frames, and text drawn
                        // there comes out blurred, which reads as a flash on whichever
                        // row lands on the seam.
                        response.scroll_to_me_animation(
                            Some(Align::Center),
                            egui::style::ScrollAnimation::none(),
                        );
                    }
                });
            }
        });
}

/// Edit buffers for the selected element, reloaded when the selection
/// changes so that a half-typed value is not lost to a redraw.
#[derive(Default)]
pub struct Editor {
    for_element: Option<ElementId>,
    text: String,
    label: String,
    bounds: [u32; 4],
}

impl Editor {
    fn load(&mut self, view: &ElementView) {
        self.for_element = Some(view.id);
        self.text = view.body_text.trim().to_owned();
        self.label = view.label.clone().unwrap_or_default();
        self.bounds = view.bounds.unwrap_or([0, 0, 0, 0]);
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        session: &Session,
        view: Option<&ElementView>,
        cell: Option<(ElementId, usize, usize)>,
        out: &mut Requests,
    ) {
        ui.heading("Element");
        ui.add_space(4.0);
        let Some(view) = view else {
            self.for_element = None;
            ui.weak("Select an element on the page or in the structure.");
            return;
        };
        if self.for_element != Some(view.id) {
            self.load(view);
        }
        let id = view.id;
        let (width, height) = session.view().resolution;

        egui::ScrollArea::vertical()
            .id_salt("editor")
            .show(ui, |ui| {
                egui::Grid::new("element-grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Kind");
                        match view.kind {
                            Some(k) => {
                                ui.label(
                                    egui::RichText::new(k.name()).color(color_for(k)).strong(),
                                );
                            }
                            None => {
                                ui.label(&view.name);
                            }
                        }
                        ui.end_row();

                        ui.label("Path");
                        ui.monospace(&view.path);
                        ui.end_row();

                        if matches!(view.kind, Some(Kind::Heading | Kind::FieldHeading)) {
                            ui.label("Level");
                            let mut level: u32 = view
                                .attrs
                                .iter()
                                .find(|(n, _)| n == "level")
                                .and_then(|(_, v)| v.parse().ok())
                                .unwrap_or(1);
                            let before = level;
                            let r =
                                ui.add(egui::DragValue::new(&mut level).range(1..=6).speed(0.1));
                            // The range clamps a typed 0 back to 1: a refusal,
                            // not a new value, and it must not reach the document.
                            if r.changed() && level != before {
                                out.commands.push(Command::SetAttr {
                                    id,
                                    name: "level".into(),
                                    value: Some(level.to_string()),
                                });
                            }
                            ui.end_row();
                        }

                        let class_values: Option<&[&str]> = match view.kind {
                            Some(Kind::List) => Some(&["unordered", "ordered"]),
                            Some(Kind::Picture) => Some(&["undefined", "chart"]),
                            Some(Kind::Value) => Some(&["read_only", "fillable"]),
                            _ => None,
                        };
                        if let Some(values) = class_values {
                            ui.label("Class");
                            let current = view
                                .attrs
                                .iter()
                                .find(|(n, _)| n == "class")
                                .map(|(_, v)| v.as_str())
                                .unwrap_or(values[0]);
                            egui::ComboBox::from_id_salt("class")
                                .selected_text(current)
                                .show_ui(ui, |ui| {
                                    for v in values {
                                        if ui.selectable_label(current == *v, *v).clicked()
                                            && current != *v
                                        {
                                            out.commands.push(Command::SetAttr {
                                                id,
                                                name: "class".into(),
                                                value: Some((*v).to_owned()),
                                            });
                                        }
                                    }
                                });
                            ui.end_row();
                        }

                        if view.kind.is_some_and(Kind::is_semantic) {
                            ui.label("Label");
                            let r = ui
                                .add(egui::TextEdit::singleline(&mut self.label).hint_text("none"));
                            if r.lost_focus()
                                && self.label.trim() != view.label.as_deref().unwrap_or("")
                            {
                                let value = self.label.trim();
                                out.commands.push(Command::SetLabel {
                                    id,
                                    value: (!value.is_empty()).then(|| value.to_owned()),
                                });
                            }
                            ui.end_row();

                            ui.label("Layer");
                            egui::ComboBox::from_id_salt("layer")
                                .selected_text(&view.layer)
                                .show_ui(ui, |ui| {
                                    for v in ["body", "background", "furniture"] {
                                        if ui.selectable_label(view.layer == v, v).clicked()
                                            && view.layer != v
                                        {
                                            out.commands.push(Command::SetLayer {
                                                id,
                                                value: (v != "body").then(|| v.to_owned()),
                                            });
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("Box");
                            ui.horizontal(|ui| {
                                let mut changed = false;
                                for (i, v) in self.bounds.iter_mut().enumerate() {
                                    let limit = if i % 2 == 0 { width } else { height };
                                    let r = ui.add(
                                        egui::DragValue::new(v).range(0..=limit.saturating_sub(1)),
                                    );
                                    // A typed value arrives when the field loses
                                    // focus, a dragged one when the drag stops;
                                    // `changed()` fires on neither frame.
                                    changed |= r.drag_stopped() || r.lost_focus();
                                }
                                if changed && view.bounds != Some(self.bounds) {
                                    out.commands.push(Command::SetBounds {
                                        id,
                                        bounds: Some(self.bounds),
                                    });
                                }
                                if view.bounds.is_some() {
                                    if ui.small_button("Clear").clicked() {
                                        out.commands.push(Command::SetBounds { id, bounds: None });
                                    }
                                } else if ui.small_button("Add").clicked() {
                                    out.commands.push(Command::SetBounds {
                                        id,
                                        bounds: Some([
                                            0,
                                            0,
                                            width.saturating_sub(1),
                                            height.saturating_sub(1),
                                        ]),
                                    });
                                }
                            });
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                if view.editable_text {
                    ui.label("Text");
                    let r = ui.add(
                        egui::TextEdit::multiline(&mut self.text)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    if r.lost_focus() && self.text != view.body_text.trim() {
                        out.commands.push(Command::SetText {
                            id,
                            text: self.text.clone(),
                        });
                    }
                } else if !view.body_text.trim().is_empty() {
                    ui.label("Text (from its parts)");
                    let collapsed = view
                        .body_text
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    ui.add(egui::Label::new(egui::RichText::new(collapsed).weak()).wrap());
                }

                if let Some((table, row, col)) = cell.filter(|(t, ..)| *t == view.id) {
                    let current = session
                        .document()
                        .find(table)
                        .and_then(|el| Grid::parse(el).at(row, col).map(|c| c.kind));
                    if let Some(current) = current {
                        ui.add_space(8.0);
                        ui.label(format!("Cell row {}, column {}", row + 1, col + 1));
                        egui::ComboBox::from_id_salt("cell-kind")
                            .selected_text(cell_kind_name(current))
                            .show_ui(ui, |ui| {
                                for k in CellKind::ALL {
                                    if ui
                                        .selectable_label(current == k, cell_kind_name(k))
                                        .clicked()
                                        && current != k
                                    {
                                        out.commands.push(Command::SetCellKind {
                                            id: table,
                                            row,
                                            col,
                                            kind: k,
                                        });
                                    }
                                }
                            });
                    }
                }

                ui.add_space(8.0);
                if ui.button("Remove element…").clicked() {
                    out.remove = Some(id);
                }

                ui.add_space(12.0);
                ui.label("Markup");
                let mut markup = view.markup.as_str();
                ui.add(
                    egui::TextEdit::multiline(&mut markup)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
            });
    }
}

/// The problems list: every finding, and a click goes to its element.
pub fn problems(ui: &mut egui::Ui, session: &Session, findings: &[Finding], out: &mut Requests) {
    ui.horizontal(|ui| {
        ui.heading("Problems");
        if findings.is_empty() {
            ui.weak("none; the document is valid");
        } else {
            ui.weak(format!("{}", findings.len()));
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("problems")
        .max_height(140.0)
        .show(ui, |ui| {
            for f in findings {
                let line = format!("{} {}  {}", f.layer, f.rule, f.path);
                let r = ui.add(
                    egui::Label::new(egui::RichText::new(&line).monospace())
                        .sense(egui::Sense::click()),
                );
                ui.add(egui::Label::new(egui::RichText::new(&f.message).weak()).wrap());
                if r.clicked() {
                    out.select = Some(Some(f.element));
                    out.go_to_page = session.page_of(f.element);
                }
                ui.add_space(4.0);
            }
        });
}
