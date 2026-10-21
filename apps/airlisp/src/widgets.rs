//! The design system: a "frosted glass" look built from tonal layers of grey (no borders), floating
//! rounded cards over a soft gradient, translucent hover/selection states and generous spacing.

use eframe::egui::{self, Align2, Color32, CornerRadius, Margin, Mesh, Painter, Pos2, Rect, Sense, Shadow, Shape, Stroke, Vec2};

use crate::fonts;
use crate::icons::{self, Icon};
use crate::theme::Palette;

pub const MENU_H: f32 = 36.0;
pub const TOOLBAR_H: f32 = 54.0;
pub const STRIP_W: f32 = 56.0;
pub const TAB_H: f32 = 48.0;
pub const STATUS_H: f32 = 30.0;
pub const ROW_H: f32 = 30.0;
pub const CRUMB_H: f32 = 30.0;
pub const PANEL_HEADER_H: f32 = 44.0;
pub const RADIUS: u8 = 10;
pub const CARD_RADIUS: u8 = 18;
pub const GAP: i8 = 8;

pub fn radius() -> CornerRadius {
    CornerRadius::same(RADIUS)
}

// ---------------------------------------------------------------------- surfaces

/// The window background: a soft vertical gradient.
pub fn paint_base(painter: &Painter, rect: Rect, pal: &Palette) {
    let mut m = Mesh::default();
    m.colored_vertex(rect.left_top(), pal.base_top);
    m.colored_vertex(rect.right_top(), pal.base_top);
    m.colored_vertex(rect.right_bottom(), pal.base_bottom);
    m.colored_vertex(rect.left_bottom(), pal.base_bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(m));
}

/// A floating tool-window card: filled, rounded, softly shadowed, no border.
pub fn card_frame(pal: &Palette, fill: Color32) -> egui::Frame {
    egui::Frame::new().fill(fill).corner_radius(CornerRadius::same(CARD_RADIUS)).shadow(Shadow { offset: [0, 6], blur: 22, spread: 0, color: pal.shadow })
}

/// Translucent "glass" surface for popups and dialogs (no border; a soft sheen on the top edge instead).
pub fn glass_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::new().fill(pal.glass).corner_radius(CornerRadius::same(CARD_RADIUS + 2)).inner_margin(Margin::same(18)).shadow(Shadow { offset: [0, 14], blur: 48, spread: 2, color: pal.shadow })
}

/// Thin highlight along the top edge of a glass surface (a light reflection, not a border).
pub fn sheen(painter: &Painter, rect: Rect, pal: &Palette) {
    let y = rect.min.y + 0.5;
    painter.line_segment([Pos2::new(rect.min.x + 16.0, y), Pos2::new(rect.max.x - 16.0, y)], Stroke::new(1.0, pal.sheen));
}

/// Dim everything behind a modal dialog.
pub fn scrim(ctx: &egui::Context, pal: &Palette) {
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("scrim"));
    let alpha = if pal.dark { 110 } else { 60 };
    ctx.layer_painter(layer).rect_filled(ctx.content_rect(), CornerRadius::ZERO, Color32::from_black_alpha(alpha));
}

// ---------------------------------------------------------------------- buttons

/// A hoverable rounded button with optional leading icon, label and trailing chevron.
pub struct Pill<'a> {
    pub icon: Option<(Icon, Color32)>,
    pub label: &'a str,
    pub chevron: bool,
    pub filled: bool,
    pub min_w: f32,
    pub height: f32,
    pub semibold: bool,
}

impl<'a> Pill<'a> {
    pub fn new(label: &'a str) -> Self {
        Pill { icon: None, label, chevron: false, filled: false, min_w: 0.0, height: 32.0, semibold: false }
    }
    pub fn icon(mut self, i: Icon, c: Color32) -> Self {
        self.icon = Some((i, c));
        self
    }
    pub fn chevron(mut self) -> Self {
        self.chevron = true;
        self
    }
    /// Resting fill (a quiet tonal button) instead of transparent.
    pub fn bordered(mut self) -> Self {
        self.filled = true;
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    pub fn semibold(mut self) -> Self {
        self.semibold = true;
        self
    }
    pub fn show(self, ui: &mut egui::Ui, pal: &Palette) -> egui::Response {
        let font = if self.semibold { fonts::semibold(13.5) } else { fonts::ui(13.5) };
        let galley = ui.painter().layout_no_wrap(self.label.to_string(), font.clone(), pal.text);
        let mut w = 14.0 + galley.size().x + 14.0;
        if self.icon.is_some() {
            w += 24.0;
        }
        if self.chevron {
            w += 22.0;
        }
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w.max(self.min_w), self.height), Sense::click());
        let p = ui.painter();
        let r = CornerRadius::same((self.height / 2.0).min(14.0) as u8);
        if resp.is_pointer_button_down_on() {
            p.rect_filled(rect, r, pal.selected);
        } else if resp.hovered() {
            p.rect_filled(rect, r, if self.filled { pal.hover.gamma_multiply(1.5) } else { pal.hover });
        } else if self.filled {
            p.rect_filled(rect, r, pal.hover.gamma_multiply(0.55));
        }
        let mut x = rect.min.x + 14.0;
        if let Some((icon, color)) = self.icon {
            icons::paint(p, Rect::from_min_size(Pos2::new(x, rect.center().y - 8.0), Vec2::splat(16.0)), icon, color);
            x += 24.0;
        }
        p.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, self.label, font, pal.text);
        if self.chevron {
            icons::paint(p, Rect::from_min_size(Pos2::new(rect.max.x - 24.0, rect.center().y - 6.0), Vec2::splat(12.0)), Icon::ChevronDown, pal.text_dim);
        }
        resp
    }
}

/// Square icon button with a soft rounded hover.
pub fn icon_button(ui: &mut egui::Ui, pal: &Palette, icon: Icon, size: f32, color: Color32, tooltip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, CornerRadius::same((size / 3.0) as u8), pal.selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same((size / 3.0) as u8), pal.hover);
    }
    let inset = (size * 0.27).round();
    icons::paint(ui.painter(), rect.shrink(inset), icon, color);
    if tooltip.is_empty() { resp } else { resp.on_hover_text(tooltip) }
}

/// Filled accent button for the main action of a dialog.
pub fn primary_button(ui: &mut egui::Ui, pal: &Palette, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), fonts::semibold(13.5), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 36.0, 36.0), Sense::click());
    let fill = if resp.is_pointer_button_down_on() { pal.accent.gamma_multiply(0.8) } else if resp.hovered() { pal.accent.gamma_multiply(1.15) } else { pal.accent };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, fonts::semibold(13.5), Color32::WHITE);
    resp
}

/// Quiet tonal secondary button.
pub fn secondary_button(ui: &mut egui::Ui, pal: &Palette, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), fonts::ui(13.5), pal.text);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 36.0, 36.0), Sense::click());
    let fill = if resp.is_pointer_button_down_on() { pal.selected } else if resp.hovered() { pal.hover.gamma_multiply(1.6) } else { pal.hover };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, fonts::ui(13.5), pal.text);
    resp
}

/// Rounded text input surface (tonal fill, no border) with an optional leading icon.
pub fn input_frame<R>(ui: &mut egui::Ui, pal: &Palette, leading: Option<Icon>, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = egui::Frame::new().fill(pal.input_bg).corner_radius(CornerRadius::same(12)).inner_margin(Margin::symmetric(12, 5)).show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(i) = leading {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                icons::paint(ui.painter(), r, i, pal.text_dim);
            }
            add(ui)
        })
        .inner
    });
    inner.inner
}

/// iOS-style switch.
pub fn toggle(ui: &mut egui::Ui, pal: &Palette, on: &mut bool) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(40.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.12);
    let track = if *on { pal.accent } else { pal.hover.gamma_multiply(1.8) };
    ui.painter().rect_filled(rect, CornerRadius::same(11), track);
    let x = rect.min.x + 11.0 + t * 18.0;
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), 8.0, Color32::WHITE);
    resp
}

/// Segmented control; returns the index chosen this frame.
pub fn segmented(ui: &mut egui::Ui, pal: &Palette, options: &[&str], selected: usize) -> Option<usize> {
    let mut chosen = None;
    let font = fonts::ui(13.0);
    let widths: Vec<f32> = options.iter().map(|o| ui.painter().layout_no_wrap(o.to_string(), font.clone(), pal.text).size().x + 28.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 34.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(12), pal.input_bg);
    let mut x = rect.min.x + 4.0;
    for (i, o) in options.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, rect.min.y + 4.0), Vec2::new(widths[i], 26.0));
        let resp = ui.interact(r, ui.id().with(("seg", i, *o)), Sense::click());
        if i == selected {
            ui.painter().rect_filled(r, CornerRadius::same(9), pal.tab_active_bg);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(9), pal.hover.gamma_multiply(0.6));
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, o, if i == selected { fonts::semibold(13.0) } else { font.clone() }, if i == selected { pal.text } else { pal.text_dim });
        if resp.clicked() {
            chosen = Some(i);
        }
        x += widths[i];
    }
    chosen
}

// ---------------------------------------------------------------------- small pieces

/// Section caption (small, dim).
pub fn caption(ui: &mut egui::Ui, pal: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).font(fonts::semibold(11.5)).color(pal.text_dim));
}

/// Small rounded "chip" such as a keyboard hint.
pub fn chip(ui: &mut egui::Ui, pal: &Palette, text: &str) {
    let g = ui.painter().layout_no_wrap(text.to_string(), fonts::ui(11.5), pal.text_dim);
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 14.0, 22.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(7), pal.hover);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, fonts::ui(11.5), pal.text_dim);
}

/// A row background used by lists/trees: translucent selection or hover, rounded with an inset.
pub fn row_background(ui: &egui::Ui, pal: &Palette, rect: Rect, selected: bool, hovered: bool) {
    let r = rect.shrink2(Vec2::new(8.0, 1.0));
    if selected {
        ui.painter().rect_filled(r, CornerRadius::same(9), pal.selected);
    } else if hovered {
        ui.painter().rect_filled(r, CornerRadius::same(9), pal.hover);
    }
}

/// Barely visible hairline (used only where a separation is really needed).
pub fn divider(ui: &egui::Ui, pal: &Palette, y: f32, x0: f32, x1: f32) {
    ui.painter().line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(1.0, pal.border));
}

/// Thin determinate progress bar.
pub fn progress_bar(ui: &mut egui::Ui, pal: &Palette, fraction: f32, width: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(4), pal.input_bg);
    let mut f = r;
    f.set_width((r.width() * fraction.clamp(0.0, 1.0)).max(8.0));
    ui.painter().rect_filled(f, CornerRadius::same(4), pal.accent);
}

#[cfg(test)]
mod tests {
    #[test]
    fn grid_constants_are_consistent() {
        assert!(super::TAB_H > super::ROW_H);
        assert!(super::CARD_RADIUS > super::RADIUS);
    }
}
