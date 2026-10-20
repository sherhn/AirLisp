//! Window chrome: menu bar, toolbar, tool strip, editor tabs, breadcrumbs, status bar, toasts.

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use ide_core::commands;

use crate::app::{App, RunConfig};
use crate::editor_view::Sev;
use crate::fonts;
use crate::icons::{self, Icon};
use crate::state::*;
use crate::widgets::{self, Pill, CRUMB_H, STATUS_H, TAB_H};

impl App {
    fn menu_item(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: &str) {
        let Some(c) = commands::by_id(id) else { return };
        let sc = c.shortcuts.first().map(|s| commands::display_shortcut(s)).unwrap_or_default();
        let mut btn = egui::Button::new(c.title);
        if !sc.is_empty() {
            btn = btn.shortcut_text(RichText::new(sc).color(self.pal.text_dim));
        }
        if ui.add(btn).clicked() {
            ui.close();
            self.run_command(ctx, id);
        }
    }

    fn menu_items(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, ids: &[&str]) {
        for id in ids {
            if *id == "-" {
                ui.separator();
            } else {
                self.menu_item(ui, ctx, id);
            }
        }
    }

    pub fn draw_menu_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.set_min_height(widgets::MENU_H - 6.0);
            let v = ui.visuals_mut();
            v.override_text_color = Some(pal.text);
            for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
                w.fg_stroke = Stroke::new(1.0, pal.text);
            }
            ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.menu_button("File", |ui| {
                self.menu_items(ui, ctx, &["file.new", "file.new_project", "file.open_project", "file.open_file", "file.reopen_last"]);
                let recent = self.settings.recent_projects.clone();
                ui.menu_button("Recent Projects", |ui| {
                    if recent.is_empty() {
                        ui.label(RichText::new("No recent projects").color(pal.text_dim));
                    }
                    for p in recent {
                        if ui.button(&p).clicked() {
                            ui.close();
                            self.open_project(std::path::Path::new(&p));
                        }
                    }
                });
                self.menu_items(ui, ctx, &["-", "file.save", "file.reload", "file.close_tab", "file.close_all", "-", "file.copy_path", "file.reveal", "-", "app.settings", "app.exit"]);
            });
            ui.menu_button("Edit", |ui| {
                self.menu_items(
                    ui,
                    ctx,
                    &["edit.undo", "edit.redo", "-", "edit.cut", "edit.copy", "edit.paste", "edit.select_all", "-", "edit.extend_selection", "edit.shrink_selection", "edit.duplicate", "edit.delete_line", "code.comment_line", "edit.join_lines", "edit.toggle_case", "-", "edit.fold", "edit.unfold", "edit.fold_all", "edit.unfold_all", "-", "find.find", "find.replace", "find.next", "find.prev", "find.in_files", "find.replace_in_files", "find.usages"],
                );
            });
            ui.menu_button("View", |ui| {
                self.menu_items(
                    ui,
                    ctx,
                    &["view.project", "view.structure", "view.git", "view.repl", "view.run", "view.problems", "view.terminal", "-", "view.zoom_in", "view.zoom_out", "view.zoom_reset", "-", "view.toggle_theme", "view.line_numbers", "view.whitespace", "view.rainbow", "view.hide_all"],
                );
            });
            ui.menu_button("Navigate", |ui| {
                self.menu_items(
                    ui,
                    ctx,
                    &["nav.search_everywhere", "nav.goto_file", "nav.goto_symbol", "nav.goto_line", "nav.recent_files", "nav.file_structure", "-", "nav.declaration", "nav.back", "nav.forward", "nav.matching_bracket", "nav.next_problem", "nav.prev_problem", "nav.last_edit"],
                );
            });
            ui.menu_button("Code", |ui| {
                self.menu_items(ui, ctx, &["code.complete", "code.quick_doc", "code.param_info", "code.expand_macro", "-", "code.reformat", "code.auto_indent"]);
                ui.separator();
                ui.menu_button("Structural Editing", |ui| {
                    for c in commands::COMMANDS.iter().filter(|c| c.id.starts_with("lisp.")) {
                        self.menu_item(ui, ctx, c.id);
                    }
                });
            });
            ui.menu_button("Refactor", |ui| self.menu_items(ui, ctx, &["refactor.rename"]));
            ui.menu_button("Git", |ui| {
                self.menu_items(ui, ctx, &["git.commit", "git.push", "git.pull", "-", "git.branches", "git.new_branch", "-", "git.diff", "git.rollback", "git.stage_file", "-", "git.refresh", "git.init"]);
            });
            ui.menu_button("Run", |ui| self.menu_items(ui, ctx, &["run.run_file", "run.compile_file", "run.stop", "-", "run.load_system", "run.test_system"]));
            ui.menu_button("Tools", |ui| self.menu_items(ui, ctx, &["repl.eval", "repl.eval_last", "repl.load_file", "repl.restart", "repl.clear", "repl.focus", "-", "view.terminal"]));
            ui.menu_button("Window", |ui| self.menu_items(ui, ctx, &["file.next_tab", "file.prev_tab", "file.close_others", "view.hide_all"]));
            ui.menu_button("Help", |ui| self.menu_items(ui, ctx, &["nav.find_action", "help.keymap", "help.about"]));
        });
    }

    // ------------------------------------------------------------------ toolbar

    pub fn draw_toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        ui.horizontal_centered(|ui| {
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 6.0;

            // ---- project widget
            let name = self.project.as_ref().map_or("No project".to_string(), |p| p.name.clone());
            let initials: String = name.chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
            let g = ui.painter().layout_no_wrap(name.clone(), fonts::semibold(13.5), pal.text);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 76.0, 38.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(rect, widgets::radius(), pal.hover);
            }
            let av = Rect::from_min_size(Pos2::new(rect.min.x + 6.0, rect.center().y - 14.0), Vec2::splat(28.0));
            ui.painter().rect_filled(av, CornerRadius::same(8), pal.accent);
            ui.painter().text(av.center(), Align2::CENTER_CENTER, if initials.is_empty() { "?".into() } else { initials }, fonts::semibold(11.5), Color32::WHITE);
            ui.painter().text(Pos2::new(av.max.x + 10.0, rect.center().y), Align2::LEFT_CENTER, &name, fonts::semibold(13.5), pal.text);
            icons::paint(ui.painter(), Rect::from_min_size(Pos2::new(rect.max.x - 22.0, rect.center().y - 6.0), Vec2::splat(12.0)), Icon::ChevronDown, pal.text_dim);
            let recents = self.settings.recent_projects.clone();
            let mut open_other: Option<String> = None;
            let mut cmd: Option<&str> = None;
            egui::Popup::menu(&resp).width(260.0).show(|ui| {
                ui.label(RichText::new("Recent projects").color(pal.text_dim).size(11.5));
                for p in recents.iter().take(8) {
                    let n = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                    if ui.button(n).on_hover_text(p).clicked() {
                        open_other = Some(p.clone());
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Open Project...").clicked() {
                    cmd = Some("file.open_project");
                    ui.close();
                }
                if ui.button("New Project...").clicked() {
                    cmd = Some("file.new_project");
                    ui.close();
                }
            });
            if let Some(p) = open_other {
                self.open_project(std::path::Path::new(&p));
            }
            if let Some(c) = cmd {
                self.run_command(ctx, c);
            }

            // ---- branch widget and VCS actions
            if let Some(branch) = self.git.branch.clone() {
                let resp = Pill::new(&branch).icon(Icon::Branch, pal.text_dim).chevron().height(34.0).show(ui, &pal);
                let branches = self.git.branches.clone();
                let mut checkout: Option<String> = None;
                let mut new_branch = false;
                egui::Popup::menu(&resp).width(280.0).show(|ui| {
                    if ui.button("New Branch...").clicked() {
                        new_branch = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.label(RichText::new("Local").color(pal.text_dim).size(11.5));
                    for b in branches.iter().filter(|b| !b.remote) {
                        let label = if b.current { format!("{}   (current)", b.name) } else { b.name.clone() };
                        if ui.add_enabled(!b.current, egui::Button::new(label)).clicked() {
                            checkout = Some(b.name.clone());
                            ui.close();
                        }
                    }
                    if branches.iter().any(|b| b.remote) {
                        ui.separator();
                        ui.label(RichText::new("Remote").color(pal.text_dim).size(11.5));
                        for b in branches.iter().filter(|b| b.remote) {
                            if ui.button(&b.name).clicked() {
                                checkout = Some(b.name.clone());
                                ui.close();
                            }
                        }
                    }
                });
                if new_branch {
                    self.popup = Popup::NewBranch { input: String::new(), just_opened: true };
                }
                if let Some(b) = checkout {
                    self.git_checkout(&b);
                }
                if self.git.repo.is_some() {
                    ui.add_space(4.0);
                    if widgets::icon_button(ui, &pal, Icon::ArrowDown, 34.0, pal.text_dim, "Update Project (Ctrl+T)").clicked() {
                        self.run_command(ctx, "git.pull");
                    }
                    let n = self.git.changes.len();
                    let r = widgets::icon_button(ui, &pal, Icon::Commit, 34.0, if n > 0 { pal.accent } else { pal.text_dim }, "Commit (Ctrl+K)");
                    if n > 0 {
                        ui.painter().circle_filled(r.rect.right_top() + Vec2::new(-8.0, 8.0), 3.5, pal.accent);
                    }
                    if r.clicked() {
                        self.run_command(ctx, "git.commit");
                    }
                    if widgets::icon_button(ui, &pal, Icon::ArrowUp, 34.0, pal.text_dim, "Push (Ctrl+Shift+K)").clicked() {
                        self.run_command(ctx, "git.push");
                    }
                    if let Some(b) = &self.git.busy {
                        ui.label(RichText::new(format!("{b}...")).color(pal.warning).size(12.0));
                    }
                }
            }

            // ---- right side
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                ui.spacing_mut().item_spacing.x = 4.0;
                if widgets::icon_button(ui, &pal, Icon::Gear, 36.0, pal.text_dim, "Settings (Ctrl+Alt+S)").clicked() {
                    self.run_command(ctx, "app.settings");
                }
                if widgets::icon_button(ui, &pal, Icon::Search, 36.0, pal.text_dim, "Search Everywhere (Double Shift)").clicked() {
                    self.run_command(ctx, "nav.search_everywhere");
                }
                ui.add_space(10.0);
                let running = self.run.running || self.compile.is_some();
                if widgets::icon_button(ui, &pal, Icon::Stop, 36.0, if running { pal.error } else { pal.text_dim.gamma_multiply(0.5) }, "Stop (Ctrl+F2)").clicked() {
                    self.run_command(ctx, "run.stop");
                }
                if widgets::icon_button(ui, &pal, Icon::Check, 36.0, pal.text_dim, "Compile current file (Ctrl+F9)").clicked() {
                    self.run_command(ctx, "run.compile_file");
                }
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, widgets::radius(), pal.hover);
                }
                icons::paint(ui.painter(), r.shrink(10.0), Icon::Play, pal.ok);
                if resp.on_hover_text("Run (Shift+F10)").clicked() {
                    self.run_command(ctx, "run.run_file");
                }
                let label = self.run_config_label();
                let resp = Pill::new(&label).icon(Icon::Lisp, pal.keyword_form).chevron().bordered().height(34.0).show(ui, &pal);
                let systems = self.asd_systems();
                let mut chosen: Option<RunConfig> = None;
                egui::Popup::menu(&resp).width(240.0).show(|ui| {
                    if ui.button("Current File").clicked() {
                        chosen = Some(RunConfig::CurrentFile);
                        ui.close();
                    }
                    for s in &systems {
                        if ui.button(format!("Load system '{s}'")).clicked() {
                            chosen = Some(RunConfig::Load(s.clone()));
                            ui.close();
                        }
                        if ui.button(format!("Test system '{s}'")).clicked() {
                            chosen = Some(RunConfig::Test(s.clone()));
                            ui.close();
                        }
                    }
                });
                if let Some(c) = chosen {
                    self.run_config = c;
                }
            });
        });
    }

    // ------------------------------------------------------------------ tool strip

    fn strip_button(&mut self, ui: &mut egui::Ui, icon: Icon, active: bool, tip: &str, badge: Option<Color32>) -> bool {
        let pal = self.pal;
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
        if active {
            ui.painter().rect_filled(r, CornerRadius::same(10), pal.selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(10), pal.hover);
        }
        icons::paint(ui.painter(), r.shrink(9.0), icon, if active { pal.text } else { pal.text_dim });
        if let Some(c) = badge {
            ui.painter().circle_filled(r.right_top() + Vec2::new(-8.0, 8.0), 4.0, c);
        }
        resp.on_hover_text(tip).clicked()
    }

    pub fn draw_tool_strip(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 6.0;
            for (icon, tool, tip) in [(Icon::Project, LeftTool::Project, "Project (Alt+1)"), (Icon::List, LeftTool::Structure, "Structure (Alt+7)")] {
                if self.strip_button(ui, icon, self.left_tool == Some(tool), tip, None) {
                    self.toggle_left(tool);
                }
            }
        });
        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 6.0;
            let errors = self.all_problems().iter().filter(|p| p.sev == Sev::Error).count();
            let pal = self.pal;
            let git_badge = (!self.git.changes.is_empty()).then_some(pal.accent);
            let tools = [
                (Icon::Search, BottomTool::Find, "Find (Ctrl+Shift+F)", None),
                (Icon::Warning, BottomTool::Problems, "Problems (Alt+6)", (errors > 0).then_some(pal.error)),
                (Icon::Terminal, BottomTool::Terminal, "Terminal (Alt+F12)", None),
                (Icon::Play, BottomTool::Run, "Run (Alt+4)", None),
                (Icon::Eye, BottomTool::Repl, "REPL (Alt+8)", self.repl.ready.then_some(pal.ok)),
                (Icon::Commit, BottomTool::Git, "Git (Alt+9)", git_badge),
            ];
            for (icon, tool, tip, badge) in tools {
                if self.strip_button(ui, icon, self.bottom_tool == Some(tool), tip, badge) {
                    self.toggle_bottom(tool);
                }
            }
        });
    }

    // ------------------------------------------------------------------ tabs

    pub fn draw_tabs(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), TAB_H), Sense::hover());
        ui.painter().rect_filled(strip, CornerRadius::ZERO, pal.panel_bg);
        widgets::divider(ui, &pal, strip.max.y - 0.5, strip.min.x, strip.max.x);
        let font = fonts::ui(13.5);
        let mut x = strip.min.x + 8.0;
        let mut to_close: Option<usize> = None;
        let mut to_activate: Option<usize> = None;
        let names: Vec<(String, bool, bool)> = self.docs.iter().map(|d| (d.name(), d.ed.is_modified(), d.conflict)).collect();
        let problems = self.all_problems();
        for (i, (name, modified, conflict)) in names.iter().enumerate() {
            let galley = ui.painter().layout_no_wrap(name.clone(), font.clone(), pal.text);
            let w = 14.0 + 18.0 + 8.0 + galley.size().x + 10.0 + 22.0 + 8.0;
            let r = Rect::from_min_size(Pos2::new(x, strip.min.y), Vec2::new(w, TAB_H));
            let resp = ui.interact(r, ui.id().with(("tab", i)), Sense::click());
            let active = i == self.active;
            let hit = r.shrink2(Vec2::new(0.0, 5.0));
            if active {
                ui.painter().rect_filled(hit, CornerRadius::same(8), pal.selected.gamma_multiply(0.45));
                ui.painter().rect_filled(Rect::from_min_size(Pos2::new(r.min.x + 12.0, r.max.y - 3.0), Vec2::new(w - 24.0, 3.0)), CornerRadius::same(2), pal.accent);
            } else if resp.hovered() {
                ui.painter().rect_filled(hit, CornerRadius::same(8), pal.hover);
            }
            let ir = Rect::from_min_size(Pos2::new(r.min.x + 14.0, r.center().y - 8.0), Vec2::splat(16.0));
            let is_lisp = self.docs[i].path.as_deref().is_none_or(lisp_syntax::dialect::is_lisp_file);
            icons::paint(ui.painter(), ir, if is_lisp { Icon::Lisp } else { Icon::File }, if is_lisp { pal.keyword_form } else { pal.text_dim });
            let has_err = problems.iter().any(|p| p.path == self.docs[i].path && p.path.is_some() && p.sev == Sev::Error);
            let git_color = self.docs[i].path.as_ref().and_then(|p| self.git_change_of(p)).map(|k| self.git_color(k));
            let text_color = if has_err { pal.error } else if let Some(c) = git_color { c } else if active { pal.text } else { pal.text_dim };
            ui.painter().text(Pos2::new(r.min.x + 40.0, r.center().y), Align2::LEFT_CENTER, name, font.clone(), text_color);
            let cr = Rect::from_center_size(Pos2::new(r.max.x - 20.0, r.center().y), Vec2::splat(20.0));
            let close_resp = ui.interact(cr, ui.id().with(("tabclose", i)), Sense::click());
            if *modified || *conflict {
                if close_resp.hovered() {
                    ui.painter().rect_filled(cr, CornerRadius::same(5), pal.hover);
                    icons::paint(ui.painter(), cr.shrink(5.0), Icon::Close, pal.text);
                } else {
                    ui.painter().circle_filled(cr.center(), 4.0, if *conflict { pal.warning } else { pal.modified });
                }
            } else if resp.hovered() || active || close_resp.hovered() {
                if close_resp.hovered() {
                    ui.painter().rect_filled(cr, CornerRadius::same(5), pal.selected);
                }
                icons::paint(ui.painter(), cr.shrink(5.0), Icon::Close, if close_resp.hovered() { pal.text } else { pal.text_dim });
            }
            if close_resp.clicked() || resp.middle_clicked() {
                to_close = Some(i);
            } else if resp.clicked() {
                to_activate = Some(i);
            }
            resp.context_menu(|ui| {
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
            x += w + 2.0;
            if x > strip.max.x {
                break;
            }
        }
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

    pub fn git_color(&self, k: ide_core::git::ChangeKind) -> Color32 {
        use ide_core::git::ChangeKind::*;
        let p = &self.pal;
        match k {
            Modified | Renamed => p.git_modified,
            Added => p.git_added,
            Untracked => p.git_untracked,
            Deleted => p.text_dim,
            Conflicted => p.error,
        }
    }

    // ------------------------------------------------------------------ breadcrumbs

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
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), CRUMB_H), Sense::hover());
        ui.painter().rect_filled(r, CornerRadius::ZERO, pal.editor_bg);
        let mut x = r.min.x + 16.0;
        let font = fonts::ui(12.0);
        let n = parts.len();
        for (i, part) in parts.iter().enumerate() {
            let last = i + 1 == n;
            let g = ui.painter().layout_no_wrap(part.clone(), font.clone(), if last { pal.text } else { pal.text_dim });
            ui.painter().galley(Pos2::new(x, r.center().y - g.size().y / 2.0), g.clone(), pal.text);
            x += g.size().x;
            if !last {
                icons::paint(ui.painter(), Rect::from_min_size(Pos2::new(x + 4.0, r.center().y - 5.0), Vec2::splat(10.0)), Icon::ChevronRight, pal.text_dim);
                x += 20.0;
            }
        }
    }

    // ------------------------------------------------------------------ status bar

    pub fn draw_status_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            if let Some(b) = self.git.branch.clone() {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                icons::paint(ui.painter(), r, Icon::Branch, pal.text_dim);
                ui.label(RichText::new(b).color(pal.text_dim).font(fonts::ui(12.0)));
                ui.add_space(8.0);
            }
            if !self.status.is_empty() {
                ui.label(RichText::new(self.status.clone()).color(pal.text_dim).font(fonts::ui(12.0)));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing.x = 18.0;
                let small = |t: String| RichText::new(t).color(pal.text_dim).font(fonts::ui(12.0));
                let info = self.doc().map(|d| {
                    let (line, col) = d.ed.line_col(d.ed.caret());
                    let sel = if d.ed.has_selection() { format!(" ({} chars)", d.ed.selected_text().chars().count()) } else { String::new() };
                    (format!("{}:{}{}", line + 1, col + 1, sel), d.ed.dialect().name().to_string(), d.ed.line_ending.label().to_string())
                });
                if let Some((pos, dialect, eol)) = info {
                    if ui.add(egui::Label::new(small(pos)).sense(Sense::click())).on_hover_text("Go to Line (Ctrl+G)").clicked() {
                        self.run_command(ctx, "nav.goto_line");
                    }
                    ui.label(small(dialect));
                    ui.label(small("2 spaces".into()));
                    ui.label(small("UTF-8".into()));
                    ui.label(small(eol));
                }
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

    // ------------------------------------------------------------------ toasts

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
            let font = fonts::ui(13.5);
            let text = elide(&t.text, 80);
            let g = ctx.fonts_mut(|f| f.layout_no_wrap(text.clone(), font.clone(), pal.text));
            let w = (g.size().x + 68.0).min(620.0);
            let h = 48.0;
            let rect = Rect::from_min_size(Pos2::new(screen.max.x - w - 20.0, y - h), Vec2::new(w, h));
            egui::Area::new(egui::Id::new(("toast", i))).fixed_pos(rect.min).order(egui::Order::Tooltip).interactable(false).show(ctx, |ui| {
                let p = ui.painter();
                p.rect_filled(rect.translate(Vec2::new(0.0, 3.0)).expand(1.0), CornerRadius::same(12), Color32::from_black_alpha(70));
                p.rect_filled(rect, CornerRadius::same(10), pal.popup_bg);
                p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, pal.border), StrokeKind::Inside);
                p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(0.0, 10.0), Vec2::new(3.0, h - 20.0)), CornerRadius::same(2), color);
                icons::paint(p, Rect::from_min_size(rect.min + Vec2::new(18.0, 16.0), Vec2::splat(16.0)), icon, color);
                p.text(Pos2::new(rect.min.x + 46.0, rect.center().y), Align2::LEFT_CENTER, text, font, pal.text);
                ui.allocate_rect(rect, Sense::hover());
            });
            y -= h + 10.0;
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
