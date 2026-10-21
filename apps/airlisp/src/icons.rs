//! Vector icons drawn with the painter, so they never depend on font glyph coverage.

use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Folder,
    FolderOpen,
    File,
    Lisp,
    Play,
    Stop,
    Restart,
    Search,
    Warning,
    Error,
    Check,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Close,
    Plus,
    Gear,
    List,
    Terminal,
    Trash,
    Project,
    Branch,
    Dot,
    Clear,
    Info,
    ArrowUp,
    ArrowDown,
    Commit,
    Refresh,
    History,
    Diff,
    Menu,
    More,
    Eye,
}

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    Pos2::new(r.min.x + r.width() * x, r.min.y + r.height() * y)
}

pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let r = Rect::from_center_size(rect.center(), Vec2::splat(rect.width().min(rect.height())));
    let s = Stroke::new((r.width() / 10.0).max(1.4), color);
    match icon {
        Icon::Folder | Icon::FolderOpen => {
            let body = Rect::from_min_max(p(r, 0.08, 0.28), p(r, 0.92, 0.82));
            painter.rect_filled(body, CornerRadius::same(2), color);
            let tab = Rect::from_min_max(p(r, 0.08, 0.18), p(r, 0.42, 0.34));
            painter.rect_filled(tab, CornerRadius::same(1), color);
            if icon == Icon::FolderOpen {
                painter.rect_filled(Rect::from_min_max(p(r, 0.2, 0.5), p(r, 0.98, 0.82)), CornerRadius::same(2), color.gamma_multiply(0.7));
            }
        }
        Icon::File | Icon::Lisp => {
            let body = Rect::from_min_max(p(r, 0.2, 0.08), p(r, 0.8, 0.92));
            painter.rect_filled(body, CornerRadius::same(2), color.gamma_multiply(if icon == Icon::Lisp { 0.28 } else { 0.18 }));
            painter.rect_stroke(body, CornerRadius::same(2), s, StrokeKind::Inside);
            if icon == Icon::Lisp {
                // a lambda: two strokes
                painter.line_segment([p(r, 0.36, 0.28), p(r, 0.66, 0.78)], s);
                painter.line_segment([p(r, 0.5, 0.52), p(r, 0.34, 0.78)], s);
            } else {
                for y in [0.34, 0.5, 0.66] {
                    painter.line_segment([p(r, 0.34, y), p(r, 0.66, y)], Stroke::new(1.0, color));
                }
            }
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(vec![p(r, 0.26, 0.14), p(r, 0.26, 0.86), p(r, 0.86, 0.5)], color, Stroke::NONE));
        }
        Icon::Stop => {
            painter.rect_filled(Rect::from_min_max(p(r, 0.2, 0.2), p(r, 0.8, 0.8)), CornerRadius::same(2), color);
        }
        Icon::Restart => {
            let c = r.center();
            let rad = r.width() * 0.34;
            let pts: Vec<Pos2> = (0..22).map(|i| {
                let a = -0.6 + (i as f32 / 21.0) * 5.0;
                c + Vec2::new(a.cos(), a.sin()) * rad
            }).collect();
            painter.add(Shape::line(pts.clone(), s));
            let end = *pts.last().unwrap();
            painter.add(Shape::convex_polygon(vec![end + Vec2::new(-3.0, -4.0), end + Vec2::new(4.0, -2.0), end + Vec2::new(-1.0, 4.0)], color, Stroke::NONE));
        }
        Icon::Search => {
            painter.circle_stroke(p(r, 0.42, 0.42), r.width() * 0.26, s);
            painter.line_segment([p(r, 0.62, 0.62), p(r, 0.9, 0.9)], Stroke::new(s.width * 1.3, color));
        }
        Icon::Warning => {
            painter.add(Shape::convex_polygon(vec![p(r, 0.5, 0.1), p(r, 0.94, 0.86), p(r, 0.06, 0.86)], color, Stroke::NONE));
            painter.line_segment([p(r, 0.5, 0.38), p(r, 0.5, 0.62)], Stroke::new(1.6, Color32::BLACK));
            painter.circle_filled(p(r, 0.5, 0.74), 1.1, Color32::BLACK);
        }
        Icon::Error => {
            painter.circle_filled(r.center(), r.width() * 0.44, color);
            painter.line_segment([p(r, 0.34, 0.34), p(r, 0.66, 0.66)], Stroke::new(1.6, Color32::WHITE));
            painter.line_segment([p(r, 0.66, 0.34), p(r, 0.34, 0.66)], Stroke::new(1.6, Color32::WHITE));
        }
        Icon::Info => {
            painter.circle_filled(r.center(), r.width() * 0.44, color);
            painter.line_segment([p(r, 0.5, 0.45), p(r, 0.5, 0.72)], Stroke::new(1.6, Color32::WHITE));
            painter.circle_filled(p(r, 0.5, 0.3), 1.1, Color32::WHITE);
        }
        Icon::Check => {
            painter.add(Shape::line(vec![p(r, 0.2, 0.52), p(r, 0.42, 0.74), p(r, 0.82, 0.28)], Stroke::new(s.width * 1.2, color)));
        }
        Icon::ChevronRight => {
            painter.add(Shape::line(vec![p(r, 0.36, 0.2), p(r, 0.68, 0.5), p(r, 0.36, 0.8)], s));
        }
        Icon::ChevronDown => {
            painter.add(Shape::line(vec![p(r, 0.2, 0.36), p(r, 0.5, 0.68), p(r, 0.8, 0.36)], s));
        }
        Icon::ChevronUp => {
            painter.add(Shape::line(vec![p(r, 0.2, 0.64), p(r, 0.5, 0.32), p(r, 0.8, 0.64)], s));
        }
        Icon::Close => {
            painter.line_segment([p(r, 0.28, 0.28), p(r, 0.72, 0.72)], s);
            painter.line_segment([p(r, 0.72, 0.28), p(r, 0.28, 0.72)], s);
        }
        Icon::Plus => {
            painter.line_segment([p(r, 0.2, 0.5), p(r, 0.8, 0.5)], s);
            painter.line_segment([p(r, 0.5, 0.2), p(r, 0.5, 0.8)], s);
        }
        Icon::Gear => {
            let c = r.center();
            painter.circle_stroke(c, r.width() * 0.2, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                painter.line_segment([c + d * r.width() * 0.3, c + d * r.width() * 0.43], Stroke::new(s.width * 1.5, color));
            }
        }
        Icon::List => {
            for y in [0.25, 0.5, 0.75] {
                painter.circle_filled(p(r, 0.2, y), 1.3, color);
                painter.line_segment([p(r, 0.36, y), p(r, 0.85, y)], s);
            }
        }
        Icon::Terminal => {
            painter.rect_stroke(Rect::from_min_max(p(r, 0.06, 0.16), p(r, 0.94, 0.84)), CornerRadius::same(2), s, StrokeKind::Inside);
            painter.add(Shape::line(vec![p(r, 0.24, 0.36), p(r, 0.42, 0.5), p(r, 0.24, 0.64)], s));
            painter.line_segment([p(r, 0.5, 0.66), p(r, 0.74, 0.66)], s);
        }
        Icon::Trash => {
            painter.rect_stroke(Rect::from_min_max(p(r, 0.24, 0.3), p(r, 0.76, 0.9)), CornerRadius::same(1), s, StrokeKind::Inside);
            painter.line_segment([p(r, 0.14, 0.26), p(r, 0.86, 0.26)], s);
            painter.line_segment([p(r, 0.38, 0.16), p(r, 0.62, 0.16)], s);
        }
        Icon::Clear => {
            painter.circle_stroke(r.center(), r.width() * 0.38, s);
            painter.line_segment([p(r, 0.24, 0.76), p(r, 0.76, 0.24)], s);
        }
        Icon::Project => {
            let body = Rect::from_min_max(p(r, 0.1, 0.2), p(r, 0.9, 0.84));
            painter.rect_stroke(body, CornerRadius::same(2), s, StrokeKind::Inside);
            painter.line_segment([p(r, 0.1, 0.38), p(r, 0.9, 0.38)], s);
            painter.line_segment([p(r, 0.36, 0.38), p(r, 0.36, 0.84)], s);
        }
        Icon::Branch => {
            painter.circle_stroke(p(r, 0.3, 0.22), 2.0, s);
            painter.circle_stroke(p(r, 0.3, 0.78), 2.0, s);
            painter.circle_stroke(p(r, 0.72, 0.4), 2.0, s);
            painter.line_segment([p(r, 0.3, 0.3), p(r, 0.3, 0.7)], s);
            painter.add(Shape::line(vec![p(r, 0.72, 0.48), p(r, 0.72, 0.56), p(r, 0.3, 0.64)], s));
        }
        Icon::ArrowUp => {
            painter.line_segment([p(r, 0.5, 0.86), p(r, 0.5, 0.18)], s);
            painter.add(Shape::line(vec![p(r, 0.24, 0.42), p(r, 0.5, 0.16), p(r, 0.76, 0.42)], s));
        }
        Icon::ArrowDown => {
            painter.line_segment([p(r, 0.5, 0.14), p(r, 0.5, 0.82)], s);
            painter.add(Shape::line(vec![p(r, 0.24, 0.58), p(r, 0.5, 0.84), p(r, 0.76, 0.58)], s));
        }
        Icon::Commit => {
            painter.line_segment([p(r, 0.06, 0.5), p(r, 0.32, 0.5)], s);
            painter.line_segment([p(r, 0.68, 0.5), p(r, 0.94, 0.5)], s);
            painter.circle_stroke(r.center(), r.width() * 0.19, s);
        }
        Icon::Refresh => {
            let c = r.center();
            let rad = r.width() * 0.36;
            let pts: Vec<Pos2> = (0..24).map(|i| {
                let a = 0.5 + (i as f32 / 23.0) * 5.0;
                c + Vec2::new(a.cos(), a.sin()) * rad
            }).collect();
            let end = pts[0];
            painter.add(Shape::line(pts, s));
            painter.add(Shape::convex_polygon(vec![end + Vec2::new(-4.0, -1.0), end + Vec2::new(3.0, -4.5), end + Vec2::new(2.5, 3.0)], color, Stroke::NONE));
        }
        Icon::History => {
            painter.circle_stroke(r.center(), r.width() * 0.38, s);
            painter.add(Shape::line(vec![p(r, 0.5, 0.28), p(r, 0.5, 0.52), p(r, 0.66, 0.62)], s));
        }
        Icon::Diff => {
            painter.rect_stroke(Rect::from_min_max(p(r, 0.1, 0.14), p(r, 0.9, 0.86)), CornerRadius::same(2), s, StrokeKind::Inside);
            painter.line_segment([p(r, 0.5, 0.14), p(r, 0.5, 0.86)], s);
            painter.line_segment([p(r, 0.2, 0.4), p(r, 0.4, 0.4)], s);
            painter.line_segment([p(r, 0.6, 0.6), p(r, 0.8, 0.6)], s);
            painter.line_segment([p(r, 0.7, 0.5), p(r, 0.7, 0.7)], s);
        }
        Icon::Menu => {
            for y in [0.28, 0.5, 0.72] {
                painter.line_segment([p(r, 0.16, y), p(r, 0.84, y)], s);
            }
        }
        Icon::More => {
            for x in [0.22, 0.5, 0.78] {
                painter.circle_filled(p(r, x, 0.5), 1.6, color);
            }
        }
        Icon::Eye => {
            painter.add(Shape::line(vec![p(r, 0.06, 0.5), p(r, 0.3, 0.3), p(r, 0.5, 0.26), p(r, 0.7, 0.3), p(r, 0.94, 0.5), p(r, 0.7, 0.7), p(r, 0.5, 0.74), p(r, 0.3, 0.7), p(r, 0.06, 0.5)], s));
            painter.circle_filled(r.center(), r.width() * 0.1, color);
        }
        Icon::Dot => {
            painter.circle_filled(r.center(), r.width() * 0.22, color);
        }
    }
}

/// Allocate a square, paint the icon, and return the response.
pub fn button(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32, hover_bg: Color32, tooltip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), hover_bg);
    }
    paint(ui.painter(), rect.shrink(size * 0.2), icon, color);
    if tooltip.is_empty() { resp } else { resp.on_hover_text(tooltip) }
}
