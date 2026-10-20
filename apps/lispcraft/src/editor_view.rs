//! The code editor widget: painting (highlighting, selection, caret, gutter) and mouse handling.
//! Keyboard input is dispatched centrally by the app through the command registry.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2, text::LayoutJob};
use ide_core::editor::Editor;
use lisp_syntax::highlight::{Span, highlight};

use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sev {
    Error,
    Warning,
    Weak,
}

#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub start: usize,
    pub end: usize,
    pub sev: Sev,
}

#[derive(Default)]
pub struct ViewState {
    pub scroll: Vec2,
    pub scroll_to_caret: bool,
    pub last_input_time: f64,
    spans: Option<(u64, Vec<Span>)>,
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
}

#[derive(Default)]
pub struct ViewOut {
    pub clicked: bool,
    /// Ctrl/Cmd+click target offset (go to declaration).
    pub ctrl_click: Option<usize>,
    /// Screen position just below the caret, for popups.
    pub caret_screen: Option<Pos2>,
    pub line_height: f32,
    pub hover_offset: Option<usize>,
    pub text_rect: Option<Rect>,
}

const GUTTER_PAD: f32 = 10.0;
const STRIPE_W: f32 = 12.0;

fn line_height(font: &FontId, ui: &egui::Ui) -> f32 {
    (ui.fonts_mut(|f| f.row_height(font)) + 3.0).round()
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

fn spans_for(state: &mut ViewState, ed: &Editor) -> usize {
    let rev = ed.revision();
    if state.spans.as_ref().is_none_or(|(r, _)| *r != rev) {
        let p = ed.parsed();
        state.spans = Some((rev, highlight(ed.text(), &p)));
    }
    state.spans.as_ref().map_or(0, |s| s.1.len())
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

pub fn show(ui: &mut egui::Ui, ed: &mut Editor, state: &mut ViewState, cfg: &ViewCfg) -> ViewOut {
    let pal = cfg.pal;
    let font = FontId::monospace(cfg.font_size);
    let lh = line_height(&font, ui);
    let cw = ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
    let total_lines = ed.line_count();
    let digits = total_lines.to_string().len().max(3);
    let gutter_w = if cfg.line_numbers { cw * digits as f32 + GUTTER_PAD * 2.0 + 6.0 } else { 6.0 };

    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, CornerRadius::ZERO, pal.editor_bg);
    let gutter_rect = Rect::from_min_size(full.min, Vec2::new(gutter_w, full.height()));
    let text_rect = Rect::from_min_max(Pos2::new(full.min.x + gutter_w, full.min.y), full.max);
    state.last_text_rect = Some(text_rect);
    let mut out = ViewOut { line_height: lh, text_rect: Some(text_rect), ..Default::default() };

    spans_for(state, ed);
    let (spans_rev, spans) = state.spans.take().unwrap_or((0, Vec::new()));
    let spans: &[Span] = &spans;
    let text = ed.text();
    // mouse interactions are collected inside the scroll closure and applied afterwards
    #[derive(Clone, Copy)]
    enum Act {
        Caret { off: usize, extend: bool },
        Word(usize),
        Line(usize),
    }
    let mut acts: Vec<Act> = Vec::new();
    let max_cols = text.lines().map(|l| display_col(l, l.chars().count())).max().unwrap_or(0);
    let content_w = (max_cols as f32 + 8.0) * cw + STRIPE_W;
    let content_h = total_lines as f32 * lh + text_rect.height() * 0.5;

    let (sel_s, sel_e) = ed.selection();
    let caret = ed.caret();
    let (caret_line, caret_col) = ed.line_col(caret);
    let bracket = ed.parsed().matching_bracket(caret);
    let parsed = ed.parsed();
    let now = ui.input(|i| i.time);
    let blink_on = ((now - state.last_input_time).max(0.0) * 1.7) as i64 % 2 == 0;

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(egui::Layout::top_down(egui::Align::Min)));
    let scroll = egui::ScrollArea::both().auto_shrink([false, false]).scroll_offset(state.scroll).id_salt("editor_scroll");
    let area = scroll.show_viewport(&mut child, |ui, viewport| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(content_w.max(viewport.width()), content_h.max(viewport.height())), Sense::click_and_drag());
        let painter = ui.painter_at(text_rect);
        let origin = rect.min + Vec2::new(8.0, 0.0);
        let first = ((viewport.min.y / lh).floor().max(0.0)) as usize;
        let last = (((viewport.max.y / lh).ceil()) as usize + 1).min(total_lines);
        state.last_viewport_lines = ((text_rect.height() / lh) as usize).max(1);

        // current line highlight
        if caret_line >= first && caret_line < last && sel_s == sel_e {
            let y = rect.min.y + caret_line as f32 * lh;
            painter.rect_filled(Rect::from_min_size(Pos2::new(text_rect.min.x, y), Vec2::new(text_rect.width() + viewport.min.x, lh)).intersect(Rect::everything_right_of(text_rect.min.x)), CornerRadius::ZERO, pal.current_line);
        }

        let mut caret_rect = None;
        for line_no in first..last {
            let ls = ed.line_start(line_no);
            let line = ed.line_text(line_no);
            let le = ls + line.len();
            let y = rect.min.y + line_no as f32 * lh;
            let x0 = origin.x;
            let ncols = line.chars().count();

            // selection background
            if sel_s != sel_e && sel_s <= le && sel_e >= ls {
                let a = sel_s.max(ls) - ls;
                let b = sel_e.min(le) - ls;
                let ca = line[..a].chars().count();
                let cb = line[..b].chars().count();
                let mut xa = x0 + display_col(line, ca) as f32 * cw;
                let mut xb = x0 + display_col(line, cb) as f32 * cw;
                if sel_e > le {
                    xb += cw * 0.6; // show the selected line break
                }
                if xb <= xa {
                    xb = xa + 2.0;
                }
                xa = xa.max(x0);
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
                painter.rect_filled(Rect::from_min_max(Pos2::new(xa, y + 1.0), Pos2::new(xb.max(xa + 2.0), y + lh - 1.0)), CornerRadius::same(2), col);
            }
            // matching bracket
            if let Some((a, b)) = bracket {
                for off in [a, b] {
                    if off >= ls && off < le.max(ls + 1) && off < le {
                        let c = line[..off - ls].chars().count();
                        let xa = x0 + display_col(line, c) as f32 * cw;
                        painter.rect_filled(Rect::from_min_size(Pos2::new(xa, y), Vec2::new(cw, lh)), CornerRadius::same(2), pal.match_bracket);
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
                painter.galley(Pos2::new(x0, y + 1.0), galley, pal.plain);
            }

            // whitespace markers
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
                let cb = line[..b].chars().count().max(ca + 1).min(ncols.max(ca + 1));
                let xa = x0 + display_col(line, ca) as f32 * cw;
                let xb = x0 + display_col(line, cb.max(ca + 1)) as f32 * cw;
                let color = match m.sev {
                    Sev::Error => pal.error,
                    Sev::Warning => pal.warning,
                    Sev::Weak => pal.weak_warning,
                };
                wavy(&painter, xa, xb.max(xa + cw * 0.6), y + lh - 3.0, color);
            }

            // caret geometry
            if line_no == caret_line {
                let x = x0 + display_col(line, caret_col) as f32 * cw;
                caret_rect = Some(Rect::from_min_size(Pos2::new(x, y), Vec2::new(2.0, lh)));
            }
        }
        // caret may be outside the painted range when scrolling to it
        let caret_full = caret_rect.unwrap_or_else(|| {
            let line = ed.line_text(caret_line);
            Rect::from_min_size(Pos2::new(origin.x + display_col(line, caret_col) as f32 * cw, rect.min.y + caret_line as f32 * lh), Vec2::new(2.0, lh))
        });
        if cfg.focused && blink_on && caret_rect.is_some() {
            painter.rect_filled(caret_full, CornerRadius::ZERO, pal.caret);
        }
        if state.scroll_to_caret {
            let target = caret_full.expand2(Vec2::new(cw * 6.0, lh * 2.0));
            ui.scroll_to_rect(target, None);
            state.scroll_to_caret = false;
        }
        out.caret_screen = Some(Pos2::new(caret_full.min.x, caret_full.max.y));

        // ---------------------------------------------------------------- mouse
        let pos_to_offset = |p: Pos2, ed: &Editor| -> usize {
            let rel = p - origin;
            let line_no = ((p.y - rect.min.y) / lh).floor().clamp(0.0, (total_lines - 1) as f32) as usize;
            let line = ed.line_text(line_no);
            let disp = (rel.x / cw + 0.5).floor().max(0.0) as usize;
            let col = char_col_from_display(line, disp);
            ed.offset_of(line_no, col)
        };
        if resp.hovered() {
            let ctrl = ui.input(|i| i.modifiers.command);
            let over_text = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| text_rect.contains(p));
            if over_text {
                out.hover_offset = ui.input(|i| i.pointer.hover_pos()).map(|p| pos_to_offset(p, ed));
                ui.set_cursor_icon(if ctrl { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Text });
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
            // keep the pointer inside the text area for autoscroll
            acts.push(Act::Caret { off: pos_to_offset(p, ed), extend: true });
            let r = Rect::from_center_size(p, Vec2::splat(4.0));
            ui.scroll_to_rect(r, None);
        }
    });
    state.scroll = area.state.clamped_offset();
    state.spans = Some((spans_rev, spans.to_vec()));
    for a in acts {
        match a {
            Act::Caret { off, extend } => ed.set_caret(off, extend),
            Act::Word(o) => ed.select_word_at(o),
            Act::Line(o) => ed.select_line_at(o),
        }
    }

    // ---------------------------------------------------------------- gutter
    let gp = ui.painter_at(gutter_rect);
    gp.rect_filled(gutter_rect, CornerRadius::ZERO, pal.gutter_bg);
    if cfg.line_numbers {
        let first = ((state.scroll.y / lh).floor().max(0.0)) as usize;
        let last = ((((state.scroll.y + gutter_rect.height()) / lh).ceil()) as usize + 1).min(total_lines);
        for line_no in first..last {
            let y = gutter_rect.min.y + line_no as f32 * lh - state.scroll.y;
            let color = if line_no == caret_line { pal.line_number_active } else { pal.line_number };
            gp.text(Pos2::new(gutter_rect.max.x - GUTTER_PAD, y + lh * 0.5), Align2::RIGHT_CENTER, line_no + 1, font.clone(), color);
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
        sp.rect_filled(Rect::from_min_size(Pos2::new(stripe.min.x + 2.0, to_y(m.start)), Vec2::new(STRIPE_W - 4.0, 3.0)), CornerRadius::ZERO, color);
    }
    let _ = parsed;
    let _ = out.hover_offset;
    ui.advance_cursor_after_rect(full);
    out
}

