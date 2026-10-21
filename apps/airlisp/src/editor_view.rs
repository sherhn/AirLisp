//! The code editor widget: painting (highlighting, selection, caret, gutter, folds, change markers)
//! and mouse handling. Keyboard input is dispatched centrally by the app through the command registry.

use std::collections::HashSet;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2, text::LayoutJob};
use ide_core::diff::{Mark, MarkKind};
use ide_core::editor::Editor;
use lisp_syntax::highlight::{Span, highlight};

use crate::folding::{self, FoldKey, Region};
use crate::icons::{self, Icon};
use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sev {
    Error,
    Warning,
    Weak,
}

#[derive(Clone, Debug)]
pub struct Marker {
    pub start: usize,
    pub end: usize,
    pub sev: Sev,
    pub msg: String,
}

#[derive(Default)]
pub struct ViewState {
    pub scroll: Vec2,
    pub scroll_to_caret: bool,
    pub last_input_time: f64,
    spans: Option<(u64, Vec<Span>)>,
    regions: Option<(u64, Vec<Region>)>,
    max_cols: Option<(u64, usize)>,
    pub last_viewport_lines: usize,
    pub last_text_rect: Option<Rect>,
}

pub struct ViewCfg<'a> {
    pub pal: &'a Palette,
    pub font_size: f32,
    pub line_numbers: bool,
    pub whitespace: bool,
    pub rainbow: bool,
    pub focused: bool,
    pub markers: &'a [Marker],
    pub find: &'a [(usize, usize)],
    pub find_current: Option<usize>,
    pub git_marks: &'a [Mark],
}

#[derive(Default)]
pub struct ViewOut {
    pub clicked: bool,
    /// Ctrl/Cmd+click target offset (go to declaration).
    pub ctrl_click: Option<usize>,
    /// Screen position just below the caret, for popups.
    pub caret_screen: Option<Pos2>,
    pub hover_offset: Option<usize>,
    pub text_rect: Option<Rect>,
    /// A fold chevron was clicked.
    pub fold_toggle: Option<FoldKey>,
}

const GUTTER_PAD: f32 = 12.0;
const FOLD_W: f32 = 18.0;
const MARK_W: f32 = 10.0;
const STRIPE_W: f32 = 12.0;
const TEXT_PAD: f32 = 14.0;

pub fn line_height_for(font_size: f32) -> f32 {
    (font_size * 1.62).round()
}

/// Display column of char column `col` in `line` (tabs expand to 4 cells).
pub fn display_col(line: &str, col: usize) -> usize {
    line.chars().take(col).map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

fn char_col_from_display(line: &str, disp: usize) -> usize {
    let mut d = 0;
    for (i, c) in line.chars().enumerate() {
        let w = if c == '\t' { 4 } else { 1 };
        if d + w > disp {
            return i;
        }
        d += w;
    }
    line.chars().count()
}

fn wavy(painter: &egui::Painter, x0: f32, x1: f32, y: f32, color: Color32) {
    let mut pts = Vec::new();
    let mut x = x0;
    let mut up = false;
    while x < x1 {
        pts.push(Pos2::new(x, if up { y - 2.0 } else { y }));
        x += 2.0;
        up = !up;
    }
    pts.push(Pos2::new(x1, y));
    if pts.len() >= 2 {
        painter.add(Shape::line(pts, Stroke::new(1.0, color)));
    }
}

pub fn show(ui: &mut egui::Ui, ed: &mut Editor, state: &mut ViewState, folds: &mut HashSet<FoldKey>, cfg: &ViewCfg) -> ViewOut {
    let pal = cfg.pal;
    let font = FontId::monospace(cfg.font_size);
    let lh = line_height_for(cfg.font_size);
    let cw = ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
    let total_lines = ed.line_count();
    let digits = total_lines.to_string().len().max(3);
    let gutter_w = if cfg.line_numbers { cw * digits as f32 + GUTTER_PAD + FOLD_W + MARK_W } else { FOLD_W + MARK_W };

    // ---- caches
    let rev = ed.revision();
    if state.spans.as_ref().is_none_or(|(r, _)| *r != rev) {
        let p = ed.parsed();
        state.spans = Some((rev, highlight(ed.text(), &p)));
    }
    if state.regions.as_ref().is_none_or(|(r, _)| *r != rev) {
        state.regions = Some((rev, folding::regions(ed)));
    }
    let (spans_rev, spans) = state.spans.take().unwrap_or((0, Vec::new()));
    let (regions_rev, regions) = state.regions.take().unwrap_or((0, Vec::new()));

    // a caret inside a folded form unfolds it
    let (caret_line, caret_col) = ed.line_col(ed.caret());
    for line in [caret_line, ed.line_of(ed.anchor())] {
        if let Some(r) = folding::hidden_by(&regions, folds, line) {
            let key = r.key.clone();
            folds.remove(&key);
        }
    }
    let rows = folding::rows(total_lines, &regions, folds);
    let row_of_line = |line: usize| rows.row_of_line(line);
    let region_start: std::collections::HashMap<usize, &Region> = regions.iter().map(|r| (r.start_line, r)).collect();

    let full = ui.available_rect_before_wrap();
    let gutter_rect = Rect::from_min_size(full.min, Vec2::new(gutter_w, full.height()));
    let text_rect = Rect::from_min_max(Pos2::new(full.min.x + gutter_w, full.min.y), full.max);
    state.last_text_rect = Some(text_rect);
    let mut out = ViewOut { text_rect: Some(text_rect), ..Default::default() };

    let text = ed.text();
    let max_cols = match state.max_cols {
        Some((r, c)) if r == rev => c,
        _ => {
            let c = text.lines().map(|l| display_col(l, l.chars().count())).max().unwrap_or(0);
            state.max_cols = Some((rev, c));
            c
        }
    };
    let content_w = (max_cols as f32 + 10.0) * cw + STRIPE_W + TEXT_PAD;
    let content_h = rows.len() as f32 * lh + text_rect.height() * 0.45;

    let (sel_s, sel_e) = ed.selection();
    let caret = ed.caret();
    let bracket = ed.parsed().matching_bracket(caret);
    let now = ui.input(|i| i.time);
    let blink_on = ((now - state.last_input_time).max(0.0) * 1.7) as i64 % 2 == 0;

    #[derive(Clone, Copy)]
    enum Act {
        Caret { off: usize, extend: bool },
        Word(usize),
        Line(usize),
    }
    let mut acts: Vec<Act> = Vec::new();

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(egui::Layout::top_down(egui::Align::Min)));
    let scroll = egui::ScrollArea::both().auto_shrink([false, false]).scroll_offset(state.scroll).id_salt("editor_scroll");
    let mut hover_marker: Option<String> = None;
    let area = scroll.show_viewport(&mut child, |ui, viewport| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(content_w.max(viewport.width()), content_h.max(viewport.height())), Sense::click_and_drag());
        let painter = ui.painter_at(text_rect);
        let origin = rect.min + Vec2::new(TEXT_PAD, 0.0);
        let first = ((viewport.min.y / lh).floor().max(0.0)) as usize;
        let last = (((viewport.max.y / lh).ceil()) as usize + 1).min(rows.len());
        state.last_viewport_lines = ((text_rect.height() / lh) as usize).max(1);

        let mut caret_rect = None;
        for row in first..last {
            let line_no = rows.at(row);
            let ls = ed.line_start(line_no);
            let line = ed.line_text(line_no);
            let le = ls + line.len();
            let y = rect.min.y + row as f32 * lh;
            let x0 = origin.x;
            let ncols = line.chars().count();

            // current line highlight
            if line_no == caret_line && sel_s == sel_e {
                painter.rect_filled(Rect::from_min_size(Pos2::new(text_rect.min.x, y), Vec2::new(rect.width().max(text_rect.width()), lh)), CornerRadius::ZERO, pal.current_line);
            }
            // selection background
            if sel_s != sel_e && sel_s <= le && sel_e >= ls {
                let a = sel_s.max(ls) - ls;
                let b = sel_e.min(le) - ls;
                let xa = x0 + display_col(line, line[..a].chars().count()) as f32 * cw;
                let mut xb = x0 + display_col(line, line[..b].chars().count()) as f32 * cw;
                if sel_e > le {
                    xb += cw * 0.6;
                }
                if xb <= xa {
                    xb = xa + 2.0;
                }
                painter.rect_filled(Rect::from_min_max(Pos2::new(xa, y), Pos2::new(xb, y + lh)), CornerRadius::ZERO, pal.selection);
            }
            // find highlights
            for (i, &(fs, fe)) in cfg.find.iter().enumerate() {
                if fe < ls || fs > le {
                    continue;
                }
                let a = fs.max(ls) - ls;
                let b = fe.min(le) - ls;
                let xa = x0 + display_col(line, line[..a].chars().count()) as f32 * cw;
                let xb = x0 + display_col(line, line[..b].chars().count()) as f32 * cw;
                let col = if cfg.find_current == Some(i) { pal.find_current } else { pal.find_match };
                painter.rect_filled(Rect::from_min_max(Pos2::new(xa, y + 2.0), Pos2::new(xb.max(xa + 2.0), y + lh - 2.0)), CornerRadius::same(3), col);
            }
            // matching bracket
            if let Some((a, b)) = bracket {
                for off in [a, b] {
                    if off >= ls && off < le {
                        let xa = x0 + display_col(line, line[..off - ls].chars().count()) as f32 * cw;
                        painter.rect_filled(Rect::from_min_size(Pos2::new(xa - 1.0, y + 2.0), Vec2::new(cw + 2.0, lh - 4.0)), CornerRadius::same(3), pal.match_bracket);
                    }
                }
            }

            // text
            if !line.is_empty() {
                let mut job = LayoutJob::default();
                job.wrap.max_width = f32::INFINITY;
                let mut pos = ls;
                let mut si = spans.partition_point(|s| s.end <= ls);
                let push = |job: &mut LayoutJob, from: usize, to: usize, color: Color32| {
                    if to > from {
                        let t = &text[from..to];
                        let t = if t.contains('\t') { t.replace('\t', "    ") } else { t.to_string() };
                        job.append(&t, 0.0, egui::TextFormat::simple(font.clone(), color));
                    }
                };
                while pos < le {
                    if si < spans.len() && spans[si].start < le {
                        let s = &spans[si];
                        let a = s.start.max(pos);
                        if a > pos {
                            push(&mut job, pos, a, pal.plain);
                        }
                        let b = s.end.min(le);
                        push(&mut job, a, b, pal.syntax(s.class, s.depth, cfg.rainbow));
                        pos = b;
                        si += 1;
                    } else {
                        push(&mut job, pos, le, pal.plain);
                        pos = le;
                    }
                }
                let galley = painter.layout_job(job);
                let gy = y + (lh - galley.size().y) / 2.0;
                painter.galley(Pos2::new(x0, gy), galley, pal.plain);
            }

            // folded placeholder
            if let Some(r) = region_start.get(&line_no)
                && folds.contains(&r.key)
            {
                let xe = x0 + display_col(line, ncols) as f32 * cw + 8.0;
                let pill = Rect::from_min_size(Pos2::new(xe, y + 4.0), Vec2::new(32.0, lh - 8.0));
                painter.rect_filled(pill, CornerRadius::same(5), pal.hover);
                for k in 0..3 {
                    painter.circle_filled(Pos2::new(pill.min.x + 9.0 + k as f32 * 7.0, pill.center().y + 2.0), 1.4, pal.text_dim);
                }
            }

            if cfg.whitespace {
                for (i, ch) in line.chars().enumerate() {
                    if ch == ' ' {
                        let x = x0 + display_col(line, i) as f32 * cw + cw * 0.5;
                        painter.circle_filled(Pos2::new(x, y + lh * 0.5), 1.0, pal.whitespace);
                    }
                }
            }

            // problems
            for m in cfg.markers {
                if m.end < ls || m.start > le {
                    continue;
                }
                let a = m.start.max(ls).min(le) - ls;
                let b = m.end.min(le).max(m.start.max(ls).min(le)) - ls;
                let ca = line[..a].chars().count();
                let cb = line[..b].chars().count().max(ca + 1);
                let xa = x0 + display_col(line, ca) as f32 * cw;
                let xb = x0 + display_col(line, cb) as f32 * cw;
                let color = match m.sev {
                    Sev::Error => pal.error,
                    Sev::Warning => pal.warning,
                    Sev::Weak => pal.weak_warning,
                };
                wavy(&painter, xa, xb.max(xa + cw * 0.6), y + lh - 4.0, color);
            }

            if line_no == caret_line {
                let x = x0 + display_col(line, caret_col) as f32 * cw;
                caret_rect = Some(Rect::from_min_size(Pos2::new(x, y + 2.0), Vec2::new(2.0, lh - 4.0)));
            }
        }
        let caret_full = caret_rect.unwrap_or_else(|| {
            let line = ed.line_text(caret_line);
            Rect::from_min_size(Pos2::new(origin.x + display_col(line, caret_col) as f32 * cw, rect.min.y + row_of_line(caret_line) as f32 * lh + 2.0), Vec2::new(2.0, lh - 4.0))
        });
        if cfg.focused && blink_on && caret_rect.is_some() {
            painter.rect_filled(caret_full, CornerRadius::same(1), pal.caret);
        }
        if state.scroll_to_caret {
            ui.scroll_to_rect(caret_full.expand2(Vec2::new(cw * 6.0, lh * 2.0)), None);
            state.scroll_to_caret = false;
        }
        out.caret_screen = Some(Pos2::new(caret_full.min.x, caret_full.max.y + 2.0));

        // ---------------------------------------------------------------- mouse
        let pos_to_offset = |p: Pos2, ed: &Editor| -> usize {
            let rel = p - origin;
            let row = ((p.y - rect.min.y) / lh).floor().clamp(0.0, (rows.len().max(1) - 1) as f32) as usize;
            let line_no = rows.get(row).unwrap_or(0);
            let line = ed.line_text(line_no);
            let disp = (rel.x / cw + 0.5).floor().max(0.0) as usize;
            ed.offset_of(line_no, char_col_from_display(line, disp))
        };
        if resp.hovered() {
            let ctrl = ui.input(|i| i.modifiers.command);
            if let Some(hp) = ui.input(|i| i.pointer.hover_pos())
                && text_rect.contains(hp)
            {
                let off = pos_to_offset(hp, ed);
                out.hover_offset = Some(off);
                ui.set_cursor_icon(if ctrl { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Text });
                if let Some(m) = cfg.markers.iter().find(|m| m.start <= off && off <= m.end.max(m.start + 1) && !m.msg.is_empty()) {
                    hover_marker = Some(m.msg.clone());
                }
            }
        }
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        if let Some(p) = ui.input(|i| i.pointer.interact_pos())
            && pressed
            && resp.hovered()
            && text_rect.contains(p)
        {
            let off = pos_to_offset(p, ed);
            let (shift, cmd) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
            out.clicked = true;
            state.last_input_time = now;
            if cmd && !shift {
                out.ctrl_click = Some(off);
            } else {
                acts.push(Act::Caret { off, extend: shift });
            }
        }
        if resp.double_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            acts.push(Act::Word(pos_to_offset(p, ed)));
        }
        if resp.triple_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            acts.push(Act::Line(pos_to_offset(p, ed)));
        }
        if resp.dragged()
            && !resp.double_clicked()
            && !resp.triple_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            acts.push(Act::Caret { off: pos_to_offset(p, ed), extend: true });
            ui.scroll_to_rect(Rect::from_center_size(p, Vec2::splat(4.0)), None);
        }
        if let Some(msg) = hover_marker.take() {
            resp.on_hover_ui_at_pointer(|ui| {
                ui.set_max_width(460.0);
                ui.label(egui::RichText::new(msg).size(12.5));
            });
        }
    });
    state.scroll = area.state.clamped_offset();
    state.spans = Some((spans_rev, spans));
    for a in acts {
        match a {
            Act::Caret { off, extend } => ed.set_caret(off, extend),
            Act::Word(o) => ed.select_word_at(o),
            Act::Line(o) => ed.select_line_at(o),
        }
    }

    // ---------------------------------------------------------------- gutter
    let gp = ui.painter_at(gutter_rect);
    let gresp = ui.interact(gutter_rect, ui.id().with("gutter"), Sense::click());
    let hover_gutter = gresp.hovered();
    let first = ((state.scroll.y / lh).floor().max(0.0)) as usize;
    let last = ((((state.scroll.y + gutter_rect.height()) / lh).ceil()) as usize + 1).min(rows.len());
    let num_right = gutter_rect.max.x - FOLD_W - MARK_W + 2.0;
    for row in first..last {
        let line_no = rows.at(row);
        let y = gutter_rect.min.y + row as f32 * lh - state.scroll.y;
        if cfg.line_numbers {
            let color = if line_no == caret_line { pal.line_number_active } else { pal.line_number };
            gp.text(Pos2::new(num_right, y + lh * 0.5), Align2::RIGHT_CENTER, line_no + 1, font.clone(), color);
        }
        if let Some(r) = region_start.get(&line_no) {
            let folded = folds.contains(&r.key);
            if folded || hover_gutter {
                let cr = Rect::from_center_size(Pos2::new(gutter_rect.max.x - FOLD_W * 0.5 - 2.0, y + lh * 0.5), Vec2::splat(12.0));
                icons::paint(&gp, cr, if folded { Icon::ChevronRight } else { Icon::ChevronDown }, pal.line_number_active);
            }
        }
    }
    for m in cfg.git_marks {
        let row = row_of_line(m.line);
        if rows.get(row) != Some(m.line) {
            continue;
        }
        let y = gutter_rect.min.y + row as f32 * lh - state.scroll.y;
        if y + lh < gutter_rect.min.y || y > gutter_rect.max.y {
            continue;
        }
        let x = gutter_rect.max.x - FOLD_W - 5.0;
        match m.kind {
            MarkKind::Added => {
                gp.rect_filled(Rect::from_min_size(Pos2::new(x, y + 1.0), Vec2::new(3.0, lh - 2.0)), CornerRadius::same(1), pal.git_added);
            }
            MarkKind::Modified => {
                gp.rect_filled(Rect::from_min_size(Pos2::new(x, y + 1.0), Vec2::new(3.0, lh - 2.0)), CornerRadius::same(1), pal.git_modified);
            }
            MarkKind::Deleted => {
                gp.add(Shape::convex_polygon(vec![Pos2::new(x, y - 3.0), Pos2::new(x + 6.0, y), Pos2::new(x, y + 3.0)], pal.error.gamma_multiply(0.9), Stroke::NONE));
            }
        }
    }
    if gresp.clicked()
        && let Some(p) = gresp.interact_pointer_pos()
        && p.x > gutter_rect.max.x - FOLD_W - 2.0
    {
        let row = (((p.y - gutter_rect.min.y + state.scroll.y) / lh).floor().max(0.0)) as usize;
        if let Some(line) = rows.get(row)
            && let Some(r) = region_start.get(&line)
        {
            out.fold_toggle = Some(r.key.clone());
        }
    }

    // ---------------------------------------------------------------- error stripe
    let stripe = Rect::from_min_max(Pos2::new(text_rect.max.x - STRIPE_W, text_rect.min.y), text_rect.max);
    let sp = ui.painter_at(stripe);
    let h = stripe.height();
    let to_y = |off: usize| stripe.min.y + (ed.line_of(off.min(ed.text().len())) as f32 / total_lines.max(1) as f32) * h;
    for &(fs, _) in cfg.find {
        sp.rect_filled(Rect::from_min_size(Pos2::new(stripe.min.x + 3.0, to_y(fs)), Vec2::new(STRIPE_W - 6.0, 2.0)), CornerRadius::ZERO, pal.find_current);
    }
    for m in cfg.markers {
        let color = match m.sev {
            Sev::Error => pal.error,
            Sev::Warning => pal.warning,
            Sev::Weak => pal.weak_warning,
        };
        sp.rect_filled(Rect::from_min_size(Pos2::new(stripe.min.x + 3.0, to_y(m.start)), Vec2::new(STRIPE_W - 6.0, 3.0)), CornerRadius::same(1), color);
    }
    state.regions = Some((regions_rev, regions));
    ui.advance_cursor_after_rect(full);
    out
}
