//! The page pane: the page image with a rectangle over every located
//! element, zoomable, and a click on a rectangle selects its element.
//!
//! Images become textures on first sight and the cache keeps the current
//! page and its neighbours, because an archive at a megabyte a page is not a
//! texture set to hold whole.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

use std::collections::HashMap;

use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use segler_core::doclang::Kind;
use segler_core::session::{PageView, Session};
use segler_core::tree::ElementId;

/// The page pane's own state: decoded textures by page number.
#[derive(Default)]
pub struct PagePane {
    textures: HashMap<usize, TextureHandle>,
    /// Pages whose image failed to decode, so the failure is reported once.
    failed: HashMap<usize, String>,
}

/// What the pane wants the application to do after a frame.
pub enum Pick {
    Nothing,
    Select(Option<ElementId>),
}

impl PagePane {
    pub fn forget(&mut self) {
        self.textures.clear();
        self.failed.clear();
    }

    /// Draw the page. `zoom` is a factor over the image's own size, or
    /// `None` to fit the pane's width; the factor used comes back with the
    /// pick so the toolbar can show it.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        session: &Session,
        page: &PageView,
        zoom: Option<f32>,
        selected: Option<ElementId>,
    ) -> (Pick, f32) {
        let texture = self.texture(ui.ctx(), session, page);
        self.evict(page.number);

        // The page's size in points at zoom 1: the image's size, or a
        // letter-shaped blank when there is no image.
        let base = match &texture {
            Some(t) => {
                let [w, h] = t.size();
                Vec2::new(w as f32, h as f32)
            }
            None => {
                let (w, h) = session.view().resolution;
                let aspect = w as f32 / h.max(1) as f32;
                Vec2::new(800.0 * aspect, 800.0)
            }
        };
        let zoom = zoom.unwrap_or_else(|| {
            // Leave room for the vertical scroll bar.
            ((ui.available_width() - 16.0) / base.x).clamp(0.05, 8.0)
        });
        let size = base * zoom;

        // The pane claims the width it was given. Left to shrink to its
        // content, it and a resizable panel around it size each other down
        // a little every frame until the panel is a sliver.
        ui.set_min_width(ui.available_width());
        let mut pick = Pick::Nothing;
        egui::ScrollArea::both()
            .id_salt("page-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let response = match &texture {
                    Some(t) => {
                        ui.add(egui::Image::from_texture((t.id(), size)).sense(Sense::click()))
                    }
                    None => {
                        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                        ui.painter()
                            .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(1.0, ui.visuals().weak_text_color()),
                            StrokeKind::Inside,
                        );
                        if let Some(why) = self.failed.get(&page.number) {
                            ui.painter().text(
                                rect.left_top() + Vec2::splat(12.0),
                                egui::Align2::LEFT_TOP,
                                why,
                                egui::FontId::proportional(14.0),
                                ui.visuals().error_fg_color,
                            );
                        }
                        response
                    }
                };
                let area = response.rect;
                let painter = ui.painter();

                for b in &page.boxes {
                    let rect = to_screen(area, b.rect);
                    let color = color_for(b.kind);
                    let is_selected = selected == Some(b.id);
                    if is_selected {
                        painter.rect_filled(rect, 0.0, color.gamma_multiply(0.18));
                    }
                    painter.rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(if is_selected { 3.0 } else { 1.0 }, color),
                        StrokeKind::Outside,
                    );
                }

                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        pick = Pick::Select(hit(area, page, pos));
                    }
                }
            });
        (pick, zoom)
    }

    fn texture(
        &mut self,
        ctx: &egui::Context,
        session: &Session,
        page: &PageView,
    ) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(&page.number) {
            return Some(t.clone());
        }
        if self.failed.contains_key(&page.number) {
            return None;
        }
        let name = page.image.as_ref()?;
        let bytes = match session.part(name) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                self.failed
                    .insert(page.number, format!("{name} is missing from the archive"));
                return None;
            }
            Err(e) => {
                self.failed.insert(page.number, format!("{name}: {e}"));
                return None;
            }
        };
        let decoded = match image::load_from_memory(&bytes) {
            Ok(img) => img.to_rgba8(),
            Err(e) => {
                self.failed.insert(page.number, format!("{name}: {e}"));
                return None;
            }
        };
        let (w, h) = decoded.dimensions();
        let color =
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], decoded.as_raw());
        let handle = ctx.load_texture(
            format!("page-{}", page.number),
            color,
            TextureOptions::LINEAR,
        );
        self.textures.insert(page.number, handle.clone());
        Some(handle)
    }

    /// Keep the current page and one either side; drop the rest.
    fn evict(&mut self, current: usize) {
        self.textures.retain(|n, _| n.abs_diff(current) <= 1);
    }
}

fn to_screen(area: Rect, frac: [f64; 4]) -> Rect {
    let [x0, y0, x1, y1] = frac;
    Rect::from_min_max(
        Pos2::new(
            area.left() + area.width() * x0 as f32,
            area.top() + area.height() * y0 as f32,
        ),
        Pos2::new(
            area.left() + area.width() * x1 as f32,
            area.top() + area.height() * y1 as f32,
        ),
    )
}

/// The smallest box under `pos`, so a caption inside a picture wins over
/// the picture.
fn hit(area: Rect, page: &PageView, pos: Pos2) -> Option<ElementId> {
    page.boxes
        .iter()
        .filter(|b| to_screen(area, b.rect).contains(pos))
        .min_by(|a, b| {
            let area_of = |r: [f64; 4]| (r[2] - r[0]) * (r[3] - r[1]);
            area_of(a.rect).total_cmp(&area_of(b.rect))
        })
        .map(|b| b.id)
}

/// One colour per family of element. Selection is shown by stroke weight
/// and fill, never by colour alone.
pub fn color_for(kind: Kind) -> Color32 {
    match kind {
        Kind::Heading | Kind::FieldHeading => Color32::from_rgb(0xf5, 0x9e, 0x0b),
        Kind::Table | Kind::Index => Color32::from_rgb(0x8b, 0x5c, 0xf6),
        Kind::Picture => Color32::from_rgb(0x14, 0xb8, 0xa6),
        Kind::List | Kind::Marker => Color32::from_rgb(0xec, 0x48, 0x99),
        Kind::PageHeader | Kind::PageFooter | Kind::Footnote => Color32::from_rgb(0x64, 0x74, 0x8b),
        _ => Color32::from_rgb(0x3b, 0x82, 0xf6),
    }
}
