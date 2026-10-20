//! Reusable, consistently styled widgets (the design system): spacing, radii and states follow
//! the JetBrains "New UI" grid (4 px base, 8 px radii, 28-40 px rows).

use eframe::egui::{self, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::fonts;
use crate::icons::{self, Icon};
use crate::theme::Palette;

pub const MENU_H: f32 = 34.0;
pub const TOOLBAR_H: f32 = 52.0;
pub const STRIP_W: f32 = 48.0;
pub const TAB_H: f32 = 40.0;
pub const STATUS_H: f32 = 30.0;
pub const ROW_H: f32 = 28.0;
pub const CRUMB_H: f32 = 30.0;
pub const PANEL_HEADER_H: f32 = 40.0;
pub const RADIUS: u8 = 8;

pub fn radius() -> CornerRadius {
    CornerRadius::same(RADIUS)
}

/// A hoverable rounded button with optional leading icon, label and trailing chevron.
pub struct Pill<'a> {
    pub icon: Option<(Icon, Color32)>,
    pub label: &'a str,
    pub chevron: bool,
    pub bordered: bool,
    pub min_w: f32,
    pub height: f32,
    pub semibold: bool,
}

impl<'a> Pill<'a> {
    pub fn new(label: &'a str) -> Self {
        Pill { icon: None, label, chevron: false, bordered: false, min_w: 0.0, height: 32.0, semibold: false }
    }
    pub fn icon(mut self, i: Icon, c: Color32) -> Self {
        self.icon = Some((i, c));
        self
    }
    pub fn chevron(mut self) -> Self {
        self.chevron = true;
        self
    }
    pub fn bordered(mut self) -> Self {
        self.bordered = true;
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
        let mut w = 12.0 + galley.size().x + 12.0;
        if self.icon.is_some() {
            w += 22.0;
        }
        if self.chevron {
            w += 20.0;
        }
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w.max(self.min_w), self.height), Sense::click());
        let p = ui.painter();
        if resp.hovered() || resp.is_pointer_button_down_on() {
            p.rect_filled(rect, radius(), if resp.is_pointer_button_down_on() { pal.selected } else { pal.hover });
        }
        if self.bordered {
            p.rect_stroke(rect, radius(), Stroke::new(1.0, pal.border), StrokeKind::Inside);
        }
        let mut x = rect.min.x + 12.0;
        if let Some((icon, color)) = self.icon {
            icons::paint(p, Rect::from_min_size(Pos2::new(x, rect.center().y - 8.0), Vec2::splat(16.0)), icon, color);
            x += 22.0;
        }
        p.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, self.label, font, pal.text);
        if self.chevron {
            icons::paint(p, Rect::from_min_size(Pos2::new(rect.max.x - 22.0, rect.center().y - 6.0), Vec2::splat(12.0)), Icon::ChevronDown, pal.text_dim);
        }
        resp
    }
}

/// Square icon button with rounded hover background.
pub fn icon_button(ui: &mut egui::Ui, pal: &Palette, icon: Icon, size: f32, color: Color32, tooltip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, radius(), if resp.is_pointer_button_down_on() { pal.selected } else { pal.hover });
    }
    let inset = (size * 0.26).round();
    icons::paint(ui.painter(), rect.shrink(inset), icon, color);
    if tooltip.is_empty() { resp } else { resp.on_hover_text(tooltip) }
}

/// Filled accent button for the main action of a dialog.
pub fn primary_button(ui: &mut egui::Ui, pal: &Palette, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), fonts::semibold(13.5), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 32.0, 34.0), Sense::click());
    let fill = if resp.is_pointer_button_down_on() { pal.accent.gamma_multiply(0.8) } else if resp.hovered() { pal.accent.linear_multiply(1.0).gamma_multiply(1.12) } else { pal.accent };
    ui.painter().rect_filled(rect, radius(), fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, fonts::semibold(13.5), Color32::WHITE);
    resp
}

/// Outlined secondary button.
pub fn secondary_button(ui: &mut egui::Ui, pal: &Palette, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), fonts::ui(13.5), pal.text);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 32.0, 34.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, radius(), pal.hover);
    }
    ui.painter().rect_stroke(rect, radius(), Stroke::new(1.0, pal.border), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, fonts::ui(13.5), pal.text);
    resp
}

/// Rounded text input frame (with optional leading search icon). Returns the TextEdit response.
pub fn input_frame<R>(ui: &mut egui::Ui, pal: &Palette, leading: Option<Icon>, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = egui::Frame::new().fill(pal.input_bg).stroke(Stroke::new(1.0, pal.border)).corner_radius(radius()).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
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

/// Section caption (small, dim).
pub fn caption(ui: &mut egui::Ui, pal: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).font(fonts::semibold(11.5)).color(pal.text_dim));
}

/// Small rounded "chip" such as a keyboard hint.
pub fn chip(ui: &mut egui::Ui, pal: &Palette, text: &str) {
    let g = ui.painter().layout_no_wrap(text.to_string(), fonts::ui(11.5), pal.text_dim);
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 14.0, 20.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(5), pal.hover);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, fonts::ui(11.5), pal.text_dim);
}

/// A row background used by lists/trees: selection or hover, rounded with a small inset.
pub fn row_background(ui: &egui::Ui, pal: &Palette, rect: Rect, selected: bool, hovered: bool) {
    let r = rect.shrink2(Vec2::new(6.0, 1.0));
    if selected {
        ui.painter().rect_filled(r, CornerRadius::same(6), pal.selected);
    } else if hovered {
        ui.painter().rect_filled(r, CornerRadius::same(6), pal.hover);
    }
}

/// Thin horizontal divider.
pub fn divider(ui: &egui::Ui, pal: &Palette, y: f32, x0: f32, x1: f32) {
    ui.painter().line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(1.0, pal.border));
}
