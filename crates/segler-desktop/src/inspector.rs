//! The structure pane, the element editor, and the problems list. Each
//! reads a view-model from the session and hands back commands; none of
//! them touches the document.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::HashSet;

use eframe::egui::{self, Align};
use segler_core::doclang::Kind;
use segler_core::otsl::{CellKind, Grid};
use segler_core::session::{Command, ElementView, PageView, Session, TextTarget};
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
/// The structure tree of the current page. A picture's inner rows, the
/// words a model read inside a figure, fold under the picture by default:
/// the image already shows them, and forty one-word rows would push the
/// page's paragraphs out of sight. A picture opens itself when the
/// selection lands inside it.
pub fn structure(
    ui: &mut egui::Ui,
    page: &PageView,
    selected: Option<ElementId>,
    scroll_to_selection: bool,
    expanded: &mut HashSet<ElementId>,
    out: &mut Requests,
) {
    ui.heading("Structure");
    ui.add_space(4.0);

    // Which picture each row sits under, if any, so a selection inside a
    // folded picture can open it before the rows draw.
    let mut owner: Vec<Option<ElementId>> = Vec::with_capacity(page.rows.len());
    let mut open_picture: Option<(ElementId, usize)> = None;
    for row in &page.rows {
        if let Some((_, depth)) = open_picture {
            if row.depth <= depth {
                open_picture = None;
            }
        }
        owner.push(open_picture.map(|(id, _)| id));
        if row.kind == Kind::Picture {
            open_picture = Some((row.id, row.depth));
        }
    }
    if let Some(sel) = selected {
        if let Some(i) = page.rows.iter().position(|r| r.id == sel) {
            if let Some(pic) = owner[i] {
                expanded.insert(pic);
            }
        }
    }
    let has_children = |i: usize| {
        page.rows
            .get(i + 1)
            .is_some_and(|next| next.depth > page.rows[i].depth)
    };

    egui::ScrollArea::vertical()
        .id_salt("structure")
        .show(ui, |ui| {
            if page.rows.is_empty() {
                ui.weak("Nothing on this page.");
            }
            for (i, row) in page.rows.iter().enumerate() {
                if owner[i].is_some_and(|pic| !expanded.contains(&pic)) {
                    continue;
                }
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
                    if row.kind == Kind::Picture && has_children(i) {
                        let open = expanded.contains(&row.id);
                        // Drawn, not typed: the default font has no triangle glyph.
                        let (rect, response) =
                            ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::click());
                        let c = rect.center();
                        let points = if open {
                            vec![
                                c + egui::vec2(-4.0, -2.0),
                                c + egui::vec2(4.0, -2.0),
                                c + egui::vec2(0.0, 3.0),
                            ]
                        } else {
                            vec![
                                c + egui::vec2(-2.0, -4.0),
                                c + egui::vec2(3.0, 0.0),
                                c + egui::vec2(-2.0, 4.0),
                            ]
                        };
                        ui.painter().add(egui::Shape::convex_polygon(
                            points,
                            ui.visuals().weak_text_color(),
                            egui::Stroke::NONE,
                        ));
                        if response
                            .on_hover_text(if open {
                                "Fold the picture's inner elements"
                            } else {
                                "Show the picture's inner elements"
                            })
                            .clicked()
                        {
                            if open {
                                expanded.remove(&row.id);
                            } else {
                                expanded.insert(row.id);
                            }
                        }
                    }
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
    fn load(&mut self, session: &Session, view: &ElementView) {
        self.for_element = Some(view.id);
        // The markup where the body carries formatting that can be spelled as
        // tags, so this box and the document pane's in-place field hold the
        // same string and mean the same thing by it.
        self.text = session
            .inline_text(TextTarget::Body(view.id))
            .unwrap_or_else(|| view.body_text.trim().to_owned());
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
            self.load(session, view);
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
                    // Text made of parts used to be shown here read-only,
                    // under the heading "Text (from its parts)", and that was
                    // the only place the window said why a formatted
                    // paragraph would not open for typing. It is editable
                    // now; what is left to say is what the tags are, so that
                    // the confirm on committing is not the first anybody
                    // hears of them.
                    // What the box is holding, said once. Tags are shown where
                    // they can be read back exactly; where they cannot, the box
                    // holds the words and the commit is what says what it could
                    // not put back.
                    if !view.plain_text {
                        if session.inline_text(TextTarget::Body(id)).is_some() {
                            ui.weak(
                                "Formatting is shown as tags. Edit them like any \
                                 other text; malformed markup is refused rather \
                                 than written.",
                            );
                        } else {
                            ui.weak(
                                "Made of parts with no inline spelling. What can be \
                                 put back is; what cannot, the commit asks about \
                                 first.",
                            );
                        }
                    }
                    let started_from = session
                        .inline_text(TextTarget::Body(id))
                        .unwrap_or_else(|| view.body_text.trim().to_owned());
                    if r.lost_focus() && self.text != started_from {
                        out.commands.push(Command::SetText {
                            id,
                            text: self.text.clone(),
                        });
                    }
                } else if !view.body_text.trim().is_empty() {
                    // Structure rather than text: a list, a table, a group.
                    // Its parts are edited where they are, and since the
                    // first Windows walkthrough that includes a list's items,
                    // which are edited in the document pane.
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
