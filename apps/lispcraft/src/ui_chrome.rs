//! Window chrome: menu bar, toolbar, tool strip, editor tabs, breadcrumbs, status bar, toasts.

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use ide_core::commands;

use crate::app::App;
use crate::editor_view::Sev;
use crate::icons::{self, Icon};
use crate::state::*;

const MENU_BAR_H: f32 = 30.0;
pub const TOOLBAR_H: f32 = 40.0;
pub const STATUS_H: f32 = 26.0;
pub const TAB_H: f32 = 36.0;

impl App {
    fn menu_item(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: &str) {
        let Some(c) = commands::by_id(id) else { return };
        let sc = c.shortcuts.first().map(|s| commands::display_shortcut(s)).unwrap_or_default();
        let mut btn = egui::Button::new(c.title);
        if !sc.is_empty() {
            btn = btn.shortcut_text(sc);
        }
        if ui.add(btn).clicked() {
            ui.close();
            self.run_command(ctx, id);
        }
    }

    pub fn draw_menu_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.set_min_height(MENU_BAR_H - 6.0);
            {
                let text = self.pal.text;
                let v = ui.visuals_mut();
                v.override_text_color = Some(text);
                for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
                    w.fg_stroke = Stroke::new(1.0, text);
                }
            }
            ui.menu_button("File", |ui| {
                for id in ["file.new", "file.new_project", "file.open_project", "file.open_file", "file.reopen_last"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                let recent = self.settings.recent_projects.clone();
                ui.menu_button("Recent Projects", |ui| {
                    if recent.is_empty() {
                        ui.label(RichText::new("No recent projects").color(self.pal.text_dim));
                    }
                    for p in recent {
                        if ui.button(&p).clicked() {
                            ui.close();
                            self.open_project(std::path::Path::new(&p));
                        }
                    }
                });
                ui.separator();
                for id in ["file.save", "file.reload", "file.close_tab", "file.close_all"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["file.copy_path", "file.reveal"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["app.settings", "app.exit"] {
                    self.menu_item(ui, ctx, id);
                }
            });
            ui.menu_button("Edit", |ui| {
                for id in ["edit.undo", "edit.redo"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["edit.cut", "edit.copy", "edit.paste", "edit.select_all"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["edit.extend_selection", "edit.shrink_selection", "edit.duplicate", "edit.delete_line", "code.comment_line", "edit.join_lines", "edit.toggle_case"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["find.find", "find.replace", "find.next", "find.prev", "find.in_files", "find.replace_in_files", "find.usages"] {
                    self.menu_item(ui, ctx, id);
                }
            });
            ui.menu_button("View", |ui| {
                for id in ["view.project", "view.structure", "view.repl", "view.run", "view.problems"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["view.zoom_in", "view.zoom_out", "view.zoom_reset"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["view.toggle_theme", "view.line_numbers", "view.whitespace", "view.rainbow", "view.hide_all"] {
                    self.menu_item(ui, ctx, id);
                }
            });
            ui.menu_button("Navigate", |ui| {
                for id in [
                    "nav.search_everywhere", "nav.goto_file", "nav.goto_symbol", "nav.goto_line", "nav.recent_files", "nav.file_structure",
                ] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["nav.declaration", "nav.back", "nav.forward", "nav.matching_bracket", "nav.next_problem", "nav.prev_problem", "nav.last_edit"] {
                    self.menu_item(ui, ctx, id);
                }
            });
            ui.menu_button("Code", |ui| {
                for id in ["code.complete", "code.quick_doc", "code.param_info", "code.expand_macro"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                for id in ["code.reformat", "code.auto_indent"] {
                    self.menu_item(ui, ctx, id);
                }
                ui.separator();
                ui.menu_button("Structural Editing", |ui| {
                    for c in commands::COMMANDS.iter().filter(|c| c.id.starts_with("lisp.")) {
                        self.menu_item(ui, ctx, c.id);
                    }
                });
            });
            ui.menu_button("Refactor", |ui| {
                self.menu_item(ui, ctx, "refactor.rename");
            });
            ui.menu_button("Run", |ui| {
                for id in ["run.run_file", "run.compile_file", "run.stop"] {
                    self.menu_item(ui, ctx, id);
                }
            });
            ui.menu_button("Tools", |ui| {
                for id in ["repl.eval", "repl.eval_last", "repl.load_file", "repl.restart", "repl.clear", "repl.focus"] {
                    self.menu_item(ui, ctx, id);
                }
            });
            ui.menu_button("Window", |ui| {
                for id in ["file.next_tab", "file.prev_tab", "file.close_others", "view.hide_all"] {
                    self.menu_item(ui, ctx, id);
                }
            });
            ui.menu_button("Help", |ui| {
                for id in ["nav.find_action", "help.keymap", "help.about"] {
                    self.menu_item(ui, ctx, id);
                }
            });
        });
    }

    pub fn draw_toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            // project widget
            let (name, branch) = match &self.project {
                Some(p) => (p.name.clone(), p.branch.clone()),
                None => ("No project".to_string(), None),
            };
            let initials: String = name.chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
            let (r, resp) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::same(6), pal.accent);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, if initials.is_empty() { "?".into() } else { initials }, FontId::proportional(11.0), Color32::WHITE);
            if resp.on_hover_text("Open Project (Ctrl+Shift+O)").clicked() {
                self.run_command(ctx, "file.open_project");
            }
            ui.label(RichText::new(&name).strong());
            if let Some(b) = branch {
                ui.add_space(8.0);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                icons::paint(ui.painter(), r, Icon::Branch, pal.text_dim);
                ui.label(RichText::new(b).color(pal.text_dim));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if icons::button(ui, Icon::Gear, 28.0, pal.text_dim, pal.hover, "Settings (Ctrl+Alt+S)").clicked() {
                    self.run_command(ctx, "app.settings");
                }
                if icons::button(ui, Icon::Search, 28.0, pal.text_dim, pal.hover, "Search Everywhere (Double Shift)").clicked() {
                    self.run_command(ctx, "nav.search_everywhere");
                }
                ui.add_space(10.0);
                let running = self.run.running || self.compile.is_some();
                if icons::button(ui, Icon::Stop, 28.0, if running { pal.error } else { pal.text_dim.gamma_multiply(0.6) }, pal.hover, "Stop (Ctrl+F2)").clicked() {
                    self.run_command(ctx, "run.stop");
                }
                if icons::button(ui, Icon::Check, 28.0, pal.text_dim, pal.hover, "Compile current file (Ctrl+F9)").clicked() {
                    self.run_command(ctx, "run.compile_file");
                }
                if icons::button(ui, Icon::Play, 28.0, pal.ok, pal.hover, "Run current file (Shift+F10)").clicked() {
                    self.run_command(ctx, "run.run_file");
                }
                // run configuration label
                let label = self.doc().map_or("Current File".to_string(), |d| d.name());
                let (r, resp) = ui.allocate_exact_size(Vec2::new(150.0, 26.0), Sense::click());
                ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, pal.border), StrokeKind::Inside);
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(6), pal.hover);
                }
                let (ir, _) = (Rect::from_min_size(r.min + Vec2::new(8.0, 6.0), Vec2::splat(14.0)), ());
                icons::paint(ui.painter(), ir, Icon::Lisp, pal.keyword_form);
                ui.painter().text(Pos2::new(r.min.x + 28.0, r.center().y), Align2::LEFT_CENTER, elide(&label, 16), FontId::proportional(13.0), pal.text);
                resp.on_hover_text("Run configuration: current file");
            });
        });
    }

    pub fn draw_tool_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        ui.vertical_centered(|ui| {
            ui.add_space(6.0);
            let tools = [(Icon::Project, LeftTool::Project, "Project (Alt+1)"), (Icon::List, LeftTool::Structure, "Structure (Alt+7)")];
            for (icon, tool, tip) in tools {
                let active = self.left_tool == Some(tool);
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                if active {
                    ui.painter().rect_filled(r, CornerRadius::same(6), pal.selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(6), pal.hover);
                }
                icons::paint(ui.painter(), r.shrink(8.0), icon, if active { pal.text } else { pal.text_dim });
                if resp.on_hover_text(tip).clicked() {
                    self.toggle_left(tool);
                }
            }
        });
        // bottom group
        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.add_space(6.0);
            let problems = self.all_problems().iter().filter(|p| p.sev == Sev::Error).count();
            let tools = [
                (Icon::Search, BottomTool::Find, "Find (Ctrl+Shift+F)", 0usize),
                (Icon::Warning, BottomTool::Problems, "Problems (Alt+6)", problems),
                (Icon::Play, BottomTool::Run, "Run (Alt+4)", 0),
                (Icon::Terminal, BottomTool::Repl, "REPL (Alt+8)", 0),
            ];
            for (icon, tool, tip, badge) in tools {
                let active = self.bottom_tool == Some(tool);
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                if active {
                    ui.painter().rect_filled(r, CornerRadius::same(6), pal.selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(6), pal.hover);
                }
                icons::paint(ui.painter(), r.shrink(8.0), icon, if active { pal.text } else { pal.text_dim });
                if badge > 0 {
                    ui.painter().circle_filled(r.right_top() + Vec2::new(-7.0, 7.0), 4.5, pal.error);
                }
                if resp.on_hover_text(tip).clicked() {
                    self.toggle_bottom(tool);
                    let _ = ctx;
                }
            }
        });
    }

    pub fn draw_tabs(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), TAB_H), Sense::hover());
        ui.painter().rect_filled(strip, CornerRadius::ZERO, pal.panel_bg);
        ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, pal.border));
        let font = FontId::proportional(13.0);
        let mut x = strip.min.x + 4.0;
        let mut to_close: Option<usize> = None;
        let mut to_activate: Option<usize> = None;
        let mut menu_for: Option<usize> = None;
        let names: Vec<(String, bool, bool, bool)> = self.docs.iter().map(|d| (d.name(), d.ed.is_modified(), d.conflict, d.path.is_some())).collect();
        let problems = self.all_problems();
        for (i, (name, modified, conflict, _)) in names.iter().enumerate() {
            let galley = ui.painter().layout_no_wrap(name.clone(), font.clone(), pal.text);
            let w = galley.size().x + 28.0 + 26.0;
            let r = Rect::from_min_size(Pos2::new(x, strip.min.y), Vec2::new(w, TAB_H));
            let resp = ui.interact(r, ui.id().with(("tab", i)), Sense::click());
            let active = i == self.active;
            if active {
                ui.painter().rect_filled(r, CornerRadius::ZERO, pal.tab_active_bg);
                ui.painter().rect_filled(Rect::from_min_size(Pos2::new(r.min.x, r.max.y - 3.0), Vec2::new(r.width(), 3.0)), CornerRadius::ZERO, pal.accent);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::ZERO, pal.hover);
            }
            let ir = Rect::from_min_size(Pos2::new(r.min.x + 8.0, r.center().y - 7.0), Vec2::splat(14.0));
            let is_lisp = self.docs[i].path.as_deref().is_none_or(lisp_syntax::dialect::is_lisp_file);
            icons::paint(ui.painter(), ir, if is_lisp { Icon::Lisp } else { Icon::File }, pal.keyword_form);
            let has_err = problems.iter().any(|p| p.path == self.docs[i].path && p.path.is_some() && p.sev == Sev::Error);
            let text_color = if has_err { pal.error } else if active { pal.text } else { pal.text_dim };
            ui.painter().galley(Pos2::new(r.min.x + 28.0, r.center().y - galley.size().y / 2.0), ui.painter().layout_no_wrap(name.clone(), font.clone(), text_color), text_color);
            // close / modified indicator
            let cr = Rect::from_center_size(Pos2::new(r.max.x - 14.0, r.center().y), Vec2::splat(16.0));
            let close_resp = ui.interact(cr, ui.id().with(("tabclose", i)), Sense::click());
            if *modified || *conflict {
                if close_resp.hovered() {
                    ui.painter().rect_filled(cr, CornerRadius::same(3), pal.hover);
                    icons::paint(ui.painter(), cr.shrink(3.0), Icon::Close, pal.text);
                } else {
                    ui.painter().circle_filled(cr.center(), 3.5, if *conflict { pal.warning } else { pal.modified });
                }
            } else if resp.hovered() || active || close_resp.hovered() {
                if close_resp.hovered() {
                    ui.painter().rect_filled(cr, CornerRadius::same(3), pal.hover);
                }
                icons::paint(ui.painter(), cr.shrink(3.0), Icon::Close, if close_resp.hovered() { pal.text } else { pal.text_dim });
            }
            if close_resp.clicked() || resp.middle_clicked() {
                to_close = Some(i);
            } else if resp.clicked() {
                to_activate = Some(i);
            }
            resp.context_menu(|ui| {
                menu_for = Some(i);
                if ui.button("Close").clicked() {
                    to_close = Some(i);
                    ui.close();
                }
                if ui.button("Close Other Tabs").clicked() {
                    to_activate = Some(i);
                    self.pending_close_others = true;
                    ui.close();
                }
                if ui.button("Copy Path").clicked() {
                    if let Some(p) = &self.docs[i].path {
                        ctx.copy_text(p.to_string_lossy().into_owned());
                    }
                    ui.close();
                }
                if ui.button("Reveal in File Manager").clicked() {
                    if let Some(p) = self.docs[i].path.clone() {
                        crate::input::reveal_in_file_manager(&p);
                    }
                    ui.close();
                }
            });
            x += w + 1.0;
            if x > strip.max.x {
                break;
            }
        }
        let _ = menu_for;
        if let Some(i) = to_activate {
            self.active = i;
            self.editor_focus = true;
        }
        if self.pending_close_others {
            self.pending_close_others = false;
            self.run_command(ctx, "file.close_others");
        }
        if let Some(i) = to_close {
            self.request_close_doc(i);
        }
    }

    pub fn draw_breadcrumbs(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        let Some(d) = self.doc() else { return };
        let mut parts: Vec<String> = Vec::new();
        match (&d.path, self.project_root()) {
            (Some(p), Some(root)) if p.starts_with(root) => {
                parts.push(self.project.as_ref().map(|x| x.name.clone()).unwrap_or_default());
                parts.extend(ide_core::workspace::relative(root, p).split('/').map(str::to_string));
            }
            (Some(p), _) => parts.push(p.to_string_lossy().into_owned()),
            (None, _) => parts.push(d.name()),
        }
        let defs = lisp_syntax::outline::outline(d.ed.text(), d.ed.dialect());
        if let Some(def) = lisp_syntax::outline::definition_at(&defs, d.ed.caret()) {
            parts.push(format!("{} {}", def.operator, def.name));
        }
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
        ui.painter().rect_filled(r, CornerRadius::ZERO, pal.editor_bg);
        let mut x = r.min.x + 12.0;
        let font = FontId::proportional(12.0);
        let n = parts.len();
        for (i, part) in parts.iter().enumerate() {
            let last = i + 1 == n;
            let g = ui.painter().layout_no_wrap(part.clone(), font.clone(), if last { pal.text } else { pal.text_dim });
            ui.painter().galley(Pos2::new(x, r.center().y - g.size().y / 2.0), g.clone(), pal.text);
            x += g.size().x;
            if !last {
                icons::paint(ui.painter(), Rect::from_min_size(Pos2::new(x + 2.0, r.center().y - 5.0), Vec2::splat(10.0)), Icon::ChevronRight, pal.text_dim);
                x += 16.0;
            }
        }
    }

    pub fn draw_status_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        ui.horizontal_centered(|ui| {
            ui.add_space(10.0);
            let msg = if !self.status.is_empty() { self.status.clone() } else { String::new() };
            ui.label(RichText::new(msg).color(pal.text_dim).size(12.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                let small = |t: String| RichText::new(t).color(pal.text_dim).size(12.0);
                let info = self.doc().map(|d| {
                    let (line, col) = d.ed.line_col(d.ed.caret());
                    let sel = if d.ed.has_selection() { format!(" ({} chars)", d.ed.selected_text().chars().count()) } else { String::new() };
                    (format!("{}:{}{}", line + 1, col + 1, sel), d.ed.dialect().name().to_string(), d.ed.line_ending.label().to_string())
                });
                if let Some((pos, dialect, eol)) = info {
                    if ui.add(egui::Label::new(small(pos)).sense(Sense::click())).on_hover_text("Go to Line (Ctrl+G)").clicked() {
                        self.run_command(ctx, "nav.goto_line");
                    }
                    ui.add_space(12.0);
                    ui.label(small(dialect));
                    ui.add_space(12.0);
                    ui.label(small("2 spaces".into()));
                    ui.add_space(12.0);
                    ui.label(small("UTF-8".into()));
                    ui.add_space(12.0);
                    ui.label(small(eol));
                }
                ui.add_space(14.0);
                // REPL indicator
                let (txt, color) = if self.repl.starting {
                    ("REPL starting...".to_string(), pal.warning)
                } else if self.repl.ready {
                    let busy = if self.repl.busy.is_empty() { "" } else { " (busy)" };
                    (format!("{}{}", self.repl.package.to_lowercase(), busy), pal.ok)
                } else if self.lisp.is_none() {
                    ("No Lisp found".to_string(), pal.error)
                } else {
                    ("REPL off".to_string(), pal.text_dim)
                };
                let resp = ui.add(egui::Label::new(small(txt)).sense(Sense::click())).on_hover_text("Toggle REPL (Alt+8)");
                let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, color);
                if resp.clicked() {
                    self.run_command(ctx, "view.repl");
                }
            });
        });
    }

    pub fn draw_toasts(&mut self, ctx: &egui::Context) {
        let pal = self.pal;
        let screen = ctx.content_rect();
        let mut y = screen.max.y - STATUS_H - 16.0;
        for (i, t) in self.toasts.iter().enumerate().rev() {
            let (icon, color) = match t.sev {
                Sev::Error => (Icon::Error, pal.error),
                Sev::Warning => (Icon::Warning, pal.warning),
                Sev::Weak => (Icon::Info, pal.accent),
            };
            let font = FontId::proportional(13.0);
            let g = ctx.fonts_mut(|f| f.layout_no_wrap(t.text.clone(), font.clone(), pal.text));
            let w = g.size().x + 56.0;
            let h = 40.0;
            let rect = Rect::from_min_size(Pos2::new(screen.max.x - w - 16.0, y - h), Vec2::new(w, h));
            egui::Area::new(egui::Id::new(("toast", i))).fixed_pos(rect.min).order(egui::Order::Tooltip).interactable(false).show(ctx, |ui| {
                let p = ui.painter();
                p.rect_filled(rect.translate(Vec2::new(0.0, 2.0)), CornerRadius::same(8), Color32::from_black_alpha(60));
                p.rect_filled(rect, CornerRadius::same(8), pal.popup_bg);
                p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, pal.border), StrokeKind::Inside);
                icons::paint(p, Rect::from_min_size(rect.min + Vec2::new(12.0, 12.0), Vec2::splat(16.0)), icon, color);
                p.text(Pos2::new(rect.min.x + 38.0, rect.center().y), Align2::LEFT_CENTER, &t.text, font, pal.text);
                ui.allocate_rect(rect, Sense::hover());
            });
            y -= h + 8.0;
        }
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

pub fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elide_keeps_short_and_trims_long() {
        assert_eq!(elide("abc", 5), "abc");
        assert_eq!(elide("abcdefghij", 5), "abcd...");
        assert_eq!(elide("жукжукжук", 4), "жук...");
    }
}
