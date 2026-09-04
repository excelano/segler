//! The document pane: the tree rendered as a reader would see it, and
//! edited in place.
//!
//! Every block the session hands over is drawn in its own shape: headings
//! by level, paragraphs from their runs, lists with their markers, tables as
//! a grid, pictures from their `src`, code and formulas set apart. A click
//! selects a block; a second click on a block whose text is plain opens it
//! for typing, in a field where the text was, and the field commits when it
//! is left. Nothing here touches the tree: the pane draws view-models and
//! hands back commands.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::HashMap;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align, Color32, FontId, Key, Sense, Stroke, StrokeKind, TextFormat, TextureHandle,
    TextureOptions, Vec2,
};
use segler_core::blocks::{Block, CellBlock, Run, Style};
use segler_core::doclang::Kind;
use segler_core::otsl::CellKind;
use segler_core::session::{Command, Session};
use segler_core::tree::ElementId;

use crate::page::color_for;

/// What the pane asked for this frame.
#[derive(Default)]
pub struct Actions {
    pub select: Option<Option<ElementId>>,
    pub select_cell: Option<(ElementId, usize, usize)>,
    pub commands: Vec<Command>,
}

/// What is being typed into, and what has been typed so far.
struct Editing {
    target: Target,
    buffer: String,
    /// The field takes focus on its first frame.
    fresh: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Element(ElementId),
    Cell(ElementId, usize, usize),
}

#[derive(Default)]
pub struct DocumentPane {
    editing: Option<Editing>,
    /// Decoded picture textures by `src` URI; `None` where decoding failed.
    pictures: HashMap<String, Option<TextureHandle>>,
}

/// The selection as the pane needs it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub element: Option<ElementId>,
    pub cell: Option<(ElementId, usize, usize)>,
}

impl DocumentPane {
    pub fn forget(&mut self) {
        self.editing = None;
        self.pictures.clear();
    }

    /// Stop any in-place edit without committing it.
    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        session: &Session,
        blocks: &[Block],
        selection: Selection,
        scroll_to_selection: bool,
    ) -> Actions {
        let mut actions = Actions::default();
        egui::ScrollArea::vertical()
            .id_salt("document")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(820.0));
                ui.add_space(12.0);
                let mut cx = Cx {
                    session,
                    selection,
                    scroll_to_selection,
                    actions: &mut actions,
                    editing: &mut self.editing,
                    pictures: &mut self.pictures,
                };
                for block in blocks {
                    cx.block(ui, block);
                    ui.add_space(8.0);
                }
                ui.add_space(40.0);
            });
        actions
    }
}

/// Everything a block needs while it draws.
struct Cx<'a> {
    session: &'a Session,
    selection: Selection,
    scroll_to_selection: bool,
    actions: &'a mut Actions,
    editing: &'a mut Option<Editing>,
    pictures: &'a mut HashMap<String, Option<TextureHandle>>,
}

impl Cx<'_> {
    fn block(&mut self, ui: &mut egui::Ui, block: &Block) {
        match block {
            Block::Heading {
                id,
                level,
                runs,
                editable,
            } => {
                let size = match level {
                    1 => 26.0,
                    2 => 22.0,
                    3 => 19.0,
                    _ => 16.5,
                };
                let base = style(true, false);
                self.text_block(
                    ui,
                    *id,
                    runs,
                    FontId::proportional(size),
                    base,
                    None,
                    *editable,
                    Kind::Heading,
                );
            }
            Block::Paragraph {
                id,
                kind,
                runs,
                blocks,
                editable,
            } => {
                let (font, base, color) = match kind {
                    Kind::PageHeader | Kind::PageFooter | Kind::Footnote => (
                        FontId::proportional(12.5),
                        Style::default(),
                        Some(ui.visuals().weak_text_color()),
                    ),
                    Kind::Caption => {
                        let s = style(false, true);
                        (
                            FontId::proportional(13.5),
                            s,
                            Some(ui.visuals().weak_text_color()),
                        )
                    }
                    Kind::Key => {
                        let s = style(true, false);
                        (FontId::proportional(15.0), s, None)
                    }
                    _ => (FontId::proportional(15.0), Style::default(), None),
                };
                self.text_block(ui, *id, runs, font, base, color, *editable, *kind);
                if !blocks.is_empty() {
                    ui.indent(("para", id), |ui| {
                        for b in blocks {
                            self.block(ui, b);
                        }
                    });
                }
            }
            Block::List { id, ordered, items } => {
                let selected = self.selection.element == Some(*id);
                let (response, _) = framed(ui, *id, selected, color_for(Kind::List), |ui| {
                    for (n, item) in items.iter().enumerate() {
                        ui.horizontal_top(|ui| {
                            let marker = item.marker.clone().unwrap_or_else(|| {
                                if *ordered {
                                    format!("{}.", n + 1)
                                } else {
                                    "•".to_owned()
                                }
                            });
                            ui.add_sized(
                                [28.0, 18.0],
                                egui::Label::new(egui::RichText::new(marker).size(15.0)),
                            );
                            ui.vertical(|ui| {
                                if !item.runs.is_empty() {
                                    let job = layout(
                                        &item.runs,
                                        FontId::proportional(15.0),
                                        Style::default(),
                                        ui.visuals().text_color(),
                                        ui.available_width(),
                                    );
                                    ui.add(egui::Label::new(job).wrap());
                                }
                                for b in &item.blocks {
                                    self.block(ui, b);
                                }
                            });
                        });
                    }
                });
                self.select_on_click(ui, &response, *id);
            }
            Block::Table {
                id,
                kind,
                caption,
                rows,
                cols,
                cells,
            } => {
                let selected = self.selection.element == Some(*id);
                if let Some(c) = caption {
                    let s = style(false, true);
                    let job = layout(
                        c,
                        FontId::proportional(13.5),
                        s,
                        ui.visuals().weak_text_color(),
                        ui.available_width(),
                    );
                    ui.add(egui::Label::new(job).wrap());
                }
                let (response, _) = framed(ui, *id, selected, color_for(*kind), |ui| {
                    self.table(ui, *id, *rows, *cols, cells);
                });
                self.select_on_click(ui, &response, *id);
            }
            Block::Picture {
                id,
                src,
                caption,
                blocks,
                ..
            } => {
                let selected = self.selection.element == Some(*id);
                let (response, _) = framed(ui, *id, selected, color_for(Kind::Picture), |ui| {
                    match src.as_deref().and_then(|s| self.picture(ui.ctx(), s)) {
                        Some(tex) => {
                            let [w, h] = tex.size();
                            let width = ui.available_width().min(w as f32);
                            let size = Vec2::new(width, width * h as f32 / w.max(1) as f32);
                            ui.add(egui::Image::from_texture((tex.id(), size)));
                        }
                        None => {
                            let label = match src {
                                Some(s) => format!("picture: {s}"),
                                None => "picture".to_owned(),
                            };
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 80.0),
                                Sense::hover(),
                            );
                            ui.painter().rect_stroke(
                                rect,
                                4.0,
                                Stroke::new(1.0, ui.visuals().weak_text_color()),
                                StrokeKind::Inside,
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                label,
                                FontId::proportional(13.0),
                                ui.visuals().weak_text_color(),
                            );
                            // Without an image, the text the model found
                            // inside is all there is to show.
                            for b in blocks {
                                self.block(ui, b);
                            }
                        }
                    }
                    if let Some(c) = caption {
                        let s = style(false, true);
                        let job = layout(
                            c,
                            FontId::proportional(13.5),
                            s,
                            ui.visuals().weak_text_color(),
                            ui.available_width(),
                        );
                        ui.add(egui::Label::new(job).wrap());
                    }
                });
                self.select_on_click(ui, &response, *id);
            }
            Block::Code {
                id,
                language,
                text,
                editable,
            } => {
                let selected = self.selection.element == Some(*id);
                if let Some(l) = language {
                    ui.small(l);
                }
                let (response, label) = framed(ui, *id, selected, color_for(Kind::Code), |ui| {
                    self.mono_block(ui, *id, text, *editable, false)
                });
                self.select_on_click(ui, &response, *id);
                if label.is_some_and(|l| l.clicked()) {
                    self.actions.select = Some(Some(*id));
                }
            }
            Block::Formula { id, text, editable } => {
                let selected = self.selection.element == Some(*id);
                let (response, label) = framed(ui, *id, selected, color_for(Kind::Formula), |ui| {
                    self.mono_block(ui, *id, text, *editable, true)
                });
                self.select_on_click(ui, &response, *id);
                if label.is_some_and(|l| l.clicked()) {
                    self.actions.select = Some(Some(*id));
                }
            }
            Block::Container { id, kind, blocks } => {
                let selected = self.selection.element == Some(*id);
                let (response, _) = framed(ui, *id, selected, color_for(*kind), |ui| {
                    ui.small(kind.name());
                    for b in blocks {
                        self.block(ui, b);
                    }
                });
                self.select_on_click(ui, &response, *id);
            }
            Block::PageBreak => {
                ui.separator();
            }
            Block::Other { id, name, runs } => {
                let selected = self.selection.element == Some(*id);
                let (response, _) =
                    framed(ui, *id, selected, ui.visuals().weak_text_color(), |ui| {
                        ui.small(format!("<{name}>"));
                        let job = layout(
                            runs,
                            FontId::proportional(15.0),
                            Style::default(),
                            ui.visuals().text_color(),
                            ui.available_width(),
                        );
                        ui.add(egui::Label::new(job).wrap());
                    });
                self.select_on_click(ui, &response, *id);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn text_block(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        runs: &[Run],
        font: FontId,
        base: Style,
        color: Option<Color32>,
        editable: bool,
        kind: Kind,
    ) {
        let selected = self.selection.element == Some(id);
        let target = Target::Element(id);
        if self.editing.as_ref().is_some_and(|e| e.target == target) {
            self.editor(ui, target, font, |text| Command::SetText { id, text });
            return;
        }
        let color = color.unwrap_or(ui.visuals().text_color());
        let job = layout(runs, font, base, color, ui.available_width() - 12.0);
        let (response, label) = framed(ui, id, selected, color_for(kind), |ui| {
            let text = if runs.is_empty() {
                egui::WidgetText::from(egui::RichText::new("(empty)").weak().italics())
            } else {
                job.into()
            };
            ui.add(egui::Label::new(text).wrap().sense(Sense::click()))
        });
        self.select_on_click(ui, &response, id);
        if label.clicked() {
            self.actions.select = Some(Some(id));
            self.actions.select_cell = None;
        }
        if (response.double_clicked() || label.double_clicked()) && editable {
            let body = self
                .session
                .element(id)
                .map(|v| v.body_text.trim().to_owned())
                .unwrap_or_default();
            *self.editing = Some(Editing {
                target,
                buffer: body,
                fresh: true,
            });
        }
    }

    fn mono_block(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        text: &str,
        editable: bool,
        italic: bool,
    ) -> Option<egui::Response> {
        let target = Target::Element(id);
        if self.editing.as_ref().is_some_and(|e| e.target == target) {
            self.editor(ui, target, FontId::monospace(13.5), |text| {
                Command::SetText { id, text }
            });
            return None;
        }
        let mut rich = egui::RichText::new(text).monospace();
        if italic {
            rich = rich.italics();
        }
        let response = ui.add(egui::Label::new(rich).wrap().sense(Sense::click()));
        if response.double_clicked() && editable {
            *self.editing = Some(Editing {
                target,
                buffer: text.to_owned(),
                fresh: true,
            });
        }
        Some(response)
    }

    /// The in-place text field. Commits on losing focus, cancels on Escape.
    fn editor(
        &mut self,
        ui: &mut egui::Ui,
        target: Target,
        font: FontId,
        make: impl FnOnce(String) -> Command,
    ) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let response = ui.add(
            egui::TextEdit::multiline(&mut editing.buffer)
                .font(font)
                .desired_width(f32::INFINITY)
                .desired_rows(1),
        );
        if editing.fresh {
            response.request_focus();
            editing.fresh = false;
            return;
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            *self.editing = None;
            return;
        }
        if response.lost_focus() {
            let buffer = editing.buffer.clone();
            *self.editing = None;
            let unchanged = match target {
                Target::Element(id) => self
                    .session
                    .element(id)
                    .is_some_and(|v| v.body_text.trim() == buffer),
                Target::Cell(..) => false,
            };
            if !unchanged {
                self.actions.commands.push(make(buffer));
            }
        }
    }

    fn table(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        rows: usize,
        cols: usize,
        cells: &[CellBlock],
    ) {
        if rows == 0 || cols == 0 {
            ui.weak("(empty table)");
            return;
        }
        let col_w = (ui.available_width() / cols as f32).max(40.0);
        for r in 0..rows {
            // Draw the row's cells at fixed widths, then paint borders and
            // fills to the row's full height, so the grid holds its columns
            // whatever a cell contains.
            let mut drawn: Vec<(egui::Rect, Option<&CellBlock>)> = Vec::new();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let mut c = 0;
                while c < cols {
                    let cell = cells.iter().find(|cell| cell.row == r && cell.col == c);
                    let span = cell.map_or(1, |cell| cell.colspan.max(1));
                    let w = col_w * span as f32;
                    let inner = ui.allocate_ui_with_layout(
                        Vec2::new(w, 0.0),
                        egui::Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(w);
                            match cell {
                                Some(cell) => self.cell(ui, id, cell, w),
                                None => {
                                    ui.allocate_space(Vec2::new(w, 24.0));
                                }
                            }
                        },
                    );
                    drawn.push((inner.response.rect, cell));
                    c += span;
                }
            });
            let bottom = drawn
                .iter()
                .map(|(rect, _)| rect.bottom())
                .fold(f32::MIN, f32::max);
            let painter = ui.painter();
            for (rect, cell) in &drawn {
                let full = egui::Rect::from_min_max(rect.min, egui::Pos2::new(rect.max.x, bottom));
                let fill = match cell {
                    Some(cell) if self.selection.cell == Some((id, cell.row, cell.col)) => {
                        ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                    }
                    // The faint background is invisible against a dark panel;
                    // the inactive-widget fill reads in both themes.
                    Some(cell) if cell.kind.is_header() => ui.visuals().widgets.inactive.bg_fill,
                    Some(_) => Color32::TRANSPARENT,
                    None => ui.visuals().faint_bg_color.gamma_multiply(0.5),
                };
                if fill != Color32::TRANSPARENT {
                    painter.rect_filled(full, 0.0, fill);
                }
                painter.rect_stroke(
                    full,
                    0.0,
                    Stroke::new(1.0, ui.visuals().weak_text_color().gamma_multiply(0.5)),
                    StrokeKind::Inside,
                );
            }
        }
    }

    /// One cell's content, drawn at the cell's width. The text is the
    /// clickable thing, so it sits above the table's frame in the hit test.
    fn cell(&mut self, ui: &mut egui::Ui, table: ElementId, cell: &CellBlock, width: f32) {
        let target = Target::Cell(table, cell.row, cell.col);
        let pad = 4.0;
        ui.add_space(pad);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(width - 2.0 * pad);
                if self.editing.as_ref().is_some_and(|e| e.target == target) {
                    let (t, r, c) = (table, cell.row, cell.col);
                    self.editor(ui, target, FontId::proportional(14.0), move |text| {
                        Command::SetCellText {
                            id: t,
                            row: r,
                            col: c,
                            text,
                        }
                    });
                    return;
                }
                let base = style(cell.kind.is_header(), false);
                let job = layout(
                    &cell.runs,
                    FontId::proportional(14.0),
                    base,
                    ui.visuals().text_color(),
                    ui.available_width(),
                );
                let text: egui::WidgetText = if cell.runs.is_empty() && cell.blocks.is_empty() {
                    egui::RichText::new(" ").size(14.0).into()
                } else {
                    job.into()
                };
                let response = ui.add_sized(
                    Vec2::new(ui.available_width(), 16.0),
                    egui::Label::new(text).wrap().sense(Sense::click()),
                );
                if response.clicked() {
                    self.actions.select = Some(Some(table));
                    self.actions.select_cell = Some((table, cell.row, cell.col));
                }
                if response.double_clicked() && cell.editable {
                    let buffer = cell
                        .runs
                        .iter()
                        .map(|r| r.text.as_str())
                        .collect::<String>();
                    *self.editing = Some(Editing {
                        target,
                        buffer,
                        fresh: true,
                    });
                }
                for b in &cell.blocks {
                    self.block(ui, b);
                }
            });
        });
        ui.add_space(pad);
    }

    fn select_on_click(&mut self, ui: &egui::Ui, response: &egui::Response, id: ElementId) {
        if response.clicked() {
            self.actions.select = Some(Some(id));
            self.actions.select_cell = None;
        }
        // Only when the block is out of view; see the structure pane for
        // why asking to scroll at all costs a frame.
        if self.selection.element == Some(id)
            && self.selection.cell.is_none()
            && self.scroll_to_selection
            && !ui.clip_rect().contains_rect(response.rect)
        {
            response
                .scroll_to_me_animation(Some(Align::Center), egui::style::ScrollAnimation::none());
        }
    }

    fn picture(&mut self, ctx: &egui::Context, src: &str) -> Option<TextureHandle> {
        if let Some(cached) = self.pictures.get(src) {
            return cached.clone();
        }
        let bytes = if let Some(rest) = src.strip_prefix("data:") {
            // data:<mime>;base64,<payload>
            rest.split_once(";base64,").and_then(|(_, payload)| {
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD
                    .decode(payload.trim())
                    .ok()
            })
        } else {
            self.session.part(src).ok().flatten()
        };
        let handle = bytes
            .and_then(|b| image::load_from_memory(&b).ok())
            .map(|img| {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                ctx.load_texture(format!("picture-{src}"), color, TextureOptions::LINEAR)
            });
        self.pictures.insert(src.to_owned(), handle.clone());
        handle
    }
}

fn style(bold: bool, italic: bool) -> Style {
    Style {
        bold,
        italic,
        ..Default::default()
    }
}

/// A block's frame: a hairline in the element's colour when selected,
/// nothing otherwise, and a click anywhere on it.
///
/// The click sense is registered before the children draw, over the area
/// the frame covered last frame, so that anything drawn inside it this
/// frame sits on top and takes its own clicks; a sense registered after the
/// children would be the topmost widget and swallow them. The true rect is
/// remembered under a hover-only sense, which intercepts nothing.
fn framed<R>(
    ui: &mut egui::Ui,
    id: ElementId,
    selected: bool,
    color: Color32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, R) {
    let id = ui.make_persistent_id(("block", id));
    let early = ui
        .ctx()
        .read_response(id.with("rect"))
        .map(|last| ui.interact(last.rect, id, Sense::click()));
    let frame = egui::Frame::new()
        .inner_margin(6.0)
        .corner_radius(3.0)
        .stroke(if selected {
            Stroke::new(1.5, color)
        } else {
            Stroke::NONE
        })
        .fill(if selected {
            color.gamma_multiply(0.08)
        } else {
            Color32::TRANSPARENT
        });
    let inner = frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        add(ui)
    });
    ui.interact(inner.response.rect, id.with("rect"), Sense::hover());
    (early.unwrap_or(inner.response), inner.inner)
}

/// Lay runs out in one job, styles applied per run.
fn layout(runs: &[Run], font: FontId, base: Style, color: Color32, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width.max(50.0);
    for run in runs {
        let mut style = run.style;
        style.bold |= base.bold;
        style.italic |= base.italic;
        let mut font_id = font.clone();
        if style.preformatted {
            font_id = FontId::monospace(font.size * 0.9);
        }
        if style.superscript || style.subscript {
            font_id.size *= 0.75;
        }
        let mut format = TextFormat {
            font_id,
            color: if style.bold {
                color.gamma_multiply(1.15)
            } else {
                color
            },
            italics: style.italic || style.handwriting,
            ..Default::default()
        };
        if style.underline {
            format.underline = Stroke::new(1.0, color);
        }
        if style.strikethrough {
            format.strikethrough = Stroke::new(1.0, color);
        }
        if style.superscript {
            format.valign = Align::TOP;
        } else if style.subscript {
            format.valign = Align::BOTTOM;
        }
        // egui has no bold face by default; weight is shown by a slightly
        // brighter colour and, for headings, by size.
        job.append(&run.text, 0.0, format);
    }
    job
}

/// The cell kinds a person may choose in the element pane.
pub fn cell_kind_name(k: CellKind) -> &'static str {
    match k {
        CellKind::Full => "cell",
        CellKind::Empty => "empty",
        CellKind::ColumnHeader => "column header",
        CellKind::RowHeader => "row header",
        CellKind::Corner => "corner",
        CellKind::SectionRow => "section row",
    }
}
