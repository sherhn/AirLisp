//! Tool windows: Project tree, Structure, REPL, Run, Problems, Find results, Git, Terminal.

use std::path::PathBuf;

use eframe::egui::{self, Color32, CornerRadius, Key, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use lisp_syntax::outline::{SymbolKind, definition_at};

use crate::app::App;
use crate::editor_view::Sev;
use crate::fonts;
use crate::i18n::{self, tr};
use crate::icons::{self, Icon};
use crate::state::*;
use crate::widgets::{self, PANEL_HEADER_H, ROW_H};

fn kind_color(app: &App, k: SymbolKind) -> Color32 {
    let p = &app.pal;
    match k {
        SymbolKind::Function | SymbolKind::Method | SymbolKind::Generic => p.definition,
        SymbolKind::Macro => p.keyword_form,
        SymbolKind::Variable | SymbolKind::Constant => p.special_var,
        SymbolKind::Class | SymbolKind::Struct | SymbolKind::Condition | SymbolKind::Type => p.builtin,
        SymbolKind::Package | SymbolKind::System => p.ok,
        SymbolKind::Other => p.text_dim,
    }
}

pub fn badge_color(app: &App, badge: &str) -> Color32 {
    let p = &app.pal;
    match badge {
        "f" | "M" | "g" => p.definition,
        "m" => p.keyword_form,
        "v" | "c" => p.special_var,
        "C" | "S" | "E" | "T" => p.builtin,
        "P" | "A" => p.ok,
        "F" | "B" => p.accent,
        _ => p.text_dim,
    }
}

impl App {
    // ------------------------------------------------------------------ panel header

    /// Header strip of a tool window; returns the rect so callers can add right-aligned actions.
    pub fn panel_header(&mut self, ui: &mut egui::Ui, title: &'static str) -> Rect {
        let pal = self.pal;
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), PANEL_HEADER_H), Sense::hover());
        ui.painter().text(Pos2::new(r.min.x + 16.0, r.center().y), egui::Align2::LEFT_CENTER, tr(title), fonts::semibold(13.5), pal.text);
        r
    }

    fn header_action(&mut self, ui: &mut egui::Ui, header: Rect, slot: usize, icon: Icon, tip: &'static str) -> bool {
        let pal = self.pal;
        let r = Rect::from_min_size(Pos2::new(header.max.x - 12.0 - 32.0 * (slot as f32 + 1.0), header.center().y - 14.0), Vec2::splat(28.0));
        let resp = ui.interact(r, ui.id().with(("hdr", tip)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, widgets::radius(), pal.hover);
        }
        icons::paint(ui.painter(), r.shrink(7.0), icon, pal.text_dim);
        resp.on_hover_text(tr(tip)).clicked()
    }

    // ------------------------------------------------------------------ project tree

    /// Rows of the project tree. They are cached until something changes (expand/collapse, refresh, save),
    /// because listing and ignore-matching a big project every frame would freeze the UI.
    fn take_tree_rows(&mut self) -> Vec<TreeRow> {
        let Some(p) = self.project.as_mut() else { return vec![] };
        if let Some((ver, rows)) = p.tree_cache.take()
            && ver == p.tree_ver
        {
            return rows;
        }
        let root = p.root.clone();
        fn walk(app: &mut App, dir: PathBuf, depth: usize, rows: &mut Vec<TreeRow>) {
            let entries = app.dir_entries(&dir);
            for e in entries {
                let (expanded, ignored) = match app.project.as_mut() {
                    Some(p) => (e.is_dir && p.expanded.contains(&e.path), p.ignore.is_ignored(&e.path, e.is_dir)),
                    None => (false, false),
                };
                rows.push(TreeRow { path: e.path.clone(), depth, is_dir: e.is_dir, expanded, name: e.name.clone(), ignored });
                if expanded && !ignored {
                    walk(app, e.path, depth + 1, rows);
                }
            }
        }
        let mut rows = Vec::new();
        let name = self.project.as_ref().map(|p| p.name.clone()).unwrap_or_default();
        let expanded = self.project.as_ref().is_some_and(|p| p.expanded.contains(&root));
        rows.push(TreeRow { path: root.clone(), depth: 0, is_dir: true, expanded, name, ignored: false });
        if expanded {
            walk(self, root, 1, &mut rows);
        }
        rows
    }

    fn put_tree_rows(&mut self, rows: Vec<TreeRow>) {
        if let Some(p) = self.project.as_mut() {
            p.tree_cache = Some((p.tree_ver, rows));
        }
    }

    pub fn draw_project_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        let header = self.panel_header(ui, "Project");
        if self.header_action(ui, header, 0, Icon::Refresh, "Refresh") {
            self.refresh_project();
        }
        if self.project.is_none() {
            ui.add_space(28.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(tr("No project is open")).color(pal.text_dim));
                ui.add_space(12.0);
                if widgets::primary_button(ui, &pal, tr("Open Project")).clicked() {
                    self.run_command(ctx, "file.open_project");
                }
                ui.add_space(8.0);
                if widgets::secondary_button(ui, &pal, tr("New Project")).clicked() {
                    self.run_command(ctx, "file.new_project");
                }
            });
            return;
        }
        let rows = self.take_tree_rows();
        let mut toggle: Option<PathBuf> = None;
        let mut open: Option<PathBuf> = None;
        let mut select: Option<PathBuf> = None;
        let mut action: Option<(String, PathBuf)> = None;
        let selected = self.project.as_ref().and_then(|p| p.selected.clone());
        let active_path = self.doc().and_then(|d| d.path.clone());
        let font = fonts::ui(13.5);
        egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &rows[range] {
                let w = ui.available_width().max(220.0);
                let (r, resp) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::click());
                let is_sel = selected.as_ref() == Some(&row.path);
                widgets::row_background(ui, &pal, r, is_sel, resp.hovered());
                let x = r.min.x + 14.0 + row.depth as f32 * 18.0;
                if row.is_dir {
                    icons::paint(ui.painter(), Rect::from_min_size(Pos2::new(x, r.center().y - 6.0), Vec2::splat(12.0)), if row.expanded { Icon::ChevronDown } else { Icon::ChevronRight }, pal.text_dim);
                }
                let ir = Rect::from_min_size(Pos2::new(x + 18.0, r.center().y - 8.0), Vec2::splat(16.0));
                let kind = self.git_change_of(&row.path);
                if row.is_dir {
                    icons::paint(ui.painter(), ir, if row.expanded { Icon::FolderOpen } else { Icon::Folder }, if row.ignored { pal.text_dim.gamma_multiply(0.7) } else { pal.builtin.gamma_multiply(0.85) });
                } else if lisp_syntax::dialect::is_lisp_file(&row.path) {
                    icons::paint(ui.painter(), ir, Icon::Lisp, if row.ignored { pal.text_dim.gamma_multiply(0.7) } else { pal.keyword_form });
                } else {
                    icons::paint(ui.painter(), ir, Icon::File, pal.text_dim);
                }
                let dirty_dir = row.is_dir && self.git.dirty_dirs.contains(&row.path) && row.depth > 0;
                let color = if row.ignored {
                    pal.text_dim.gamma_multiply(0.7)
                } else if let Some(k) = kind {
                    self.git_color(k)
                } else if dirty_dir {
                    pal.git_modified
                } else if active_path.as_ref() == Some(&row.path) {
                    pal.accent
                } else {
                    pal.text
                };
                ui.painter().text(Pos2::new(x + 42.0, r.center().y), egui::Align2::LEFT_CENTER, &row.name, font.clone(), color);
                if resp.clicked() {
                    select = Some(row.path.clone());
                    if row.is_dir {
                        toggle = Some(row.path.clone());
                    }
                }
                if resp.double_clicked() && !row.is_dir {
                    open = Some(row.path.clone());
                }
                resp.context_menu(|ui| {
                    let dir = if row.is_dir { row.path.clone() } else { row.path.parent().map(PathBuf::from).unwrap_or_default() };
                    if ui.button(tr("New File...")).clicked() {
                        action = Some(("new_file".into(), dir.clone()));
                        ui.close();
                    }
                    if ui.button(tr("New Directory...")).clicked() {
                        action = Some(("new_dir".into(), dir.clone()));
                        ui.close();
                    }
                    ui.separator();
                    if !row.is_dir && ui.button(tr("Open")).clicked() {
                        open = Some(row.path.clone());
                        ui.close();
                    }
                    if ui.button(tr("Rename...")).clicked() {
                        action = Some(("rename".into(), row.path.clone()));
                        ui.close();
                    }
                    if ui.button(tr("Delete...")).clicked() {
                        action = Some(("delete".into(), row.path.clone()));
                        ui.close();
                    }
                    if !row.is_dir && self.git.repo.is_some() {
                        ui.separator();
                        if ui.button(tr("Git: Show Diff")).clicked() {
                            action = Some(("diff".into(), row.path.clone()));
                            ui.close();
                        }
                        if ui.button(tr("Git: Add")).clicked() {
                            action = Some(("stage".into(), row.path.clone()));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button(tr("Copy Path")).clicked() {
                        ctx.copy_text(row.path.to_string_lossy().into_owned());
                        ui.close();
                    }
                    if ui.button(tr("Reveal in File Manager")).clicked() {
                        crate::input::reveal_in_file_manager(&row.path);
                        ui.close();
                    }
                    if ui.button(tr("Refresh")).clicked() {
                        action = Some(("refresh".into(), row.path.clone()));
                        ui.close();
                    }
                });
            }
        });
        self.put_tree_rows(rows);
        if let Some(p) = select
            && let Some(proj) = self.project.as_mut()
        {
            proj.selected = Some(p);
        }
        if let Some(p) = toggle
            && let Some(proj) = self.project.as_mut()
        {
            if !proj.expanded.remove(&p) {
                proj.expanded.insert(p);
            }
            proj.tree_ver += 1;
        }
        if let Some(p) = open {
            self.open_file(&p);
        }
        if let Some((a, path)) = action {
            match a.as_str() {
                "new_file" => self.popup = Popup::NewFile { dir: path, input: String::new(), is_dir: false, just_opened: true },
                "new_dir" => self.popup = Popup::NewFile { dir: path, input: String::new(), is_dir: true, just_opened: true },
                "rename" => {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    self.popup = Popup::RenameFile { path, input: name, just_opened: true };
                }
                "delete" => {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    self.popup = Popup::Confirm { message: i18n::fmt("Delete '{}'? This cannot be undone.", &[&name]), kind: ConfirmKind::Delete(path) };
                }
                "diff" => {
                    if let Some(rel) = self.git.repo.as_ref().and_then(|r| r.rel(&path)) {
                        self.git_show_diff(&rel);
                    }
                }
                "stage" => {
                    if let Some(rel) = self.git.repo.as_ref().and_then(|r| r.rel(&path)) {
                        self.git_run_sync("Add", move |r| r.stage(&[rel]).map(|_| String::new()));
                    }
                }
                "refresh" => self.refresh_project(),
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------------ structure

    pub fn draw_structure_panel(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        self.panel_header(ui, "Structure");
        if self.doc().is_none() {
            ui.add_space(12.0);
            ui.label(RichText::new(format!("   {}", tr("No file is open"))).color(pal.text_dim));
            return;
        }
        // take the cached outline out of the document while drawing (no clone of a possibly huge list)
        let (defs, caret) = {
            let d = self.docs.get_mut(self.active).expect("checked above");
            d.defs();
            (d.outline_cache.take().map(|c| c.1).unwrap_or_default(), d.ed.caret())
        };
        let current = definition_at(&defs, caret).map(|x| x.start);
        let mut goto: Option<usize> = None;
        if defs.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new(format!("   {}", tr("Nothing to show"))).color(pal.text_dim));
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, ROW_H, defs.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for def in &defs[range] {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
                widgets::row_background(ui, &pal, r, current == Some(def.start), resp.hovered());
                let c = kind_color(self, def.kind);
                let br = Rect::from_min_size(Pos2::new(r.min.x + 16.0, r.center().y - 9.0), Vec2::splat(18.0));
                ui.painter().rect_filled(br, CornerRadius::same(5), c.gamma_multiply(0.22));
                ui.painter().text(br.center(), egui::Align2::CENTER_CENTER, def.kind.badge(), fonts::semibold(11.0), c);
                ui.painter().text(Pos2::new(r.min.x + 44.0, r.center().y), egui::Align2::LEFT_CENTER, &def.name, fonts::ui(13.5), pal.text);
                if resp.clicked() {
                    goto = Some(def.name_start);
                }
            }
        });
        if let Some(d) = self.docs.get_mut(self.active) {
            let rev = d.ed.revision();
            d.outline_cache = Some((rev, defs));
        }
        if let Some(o) = goto
            && let Some(d) = self.doc_mut()
        {
            d.ed.set_caret(o, false);
            d.view.scroll_to_caret = true;
            self.editor_focus = true;
        }
    }

    // ------------------------------------------------------------------ bottom tool windows

    pub fn draw_bottom_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        let Some(tool) = self.bottom_tool else { return };
        let problems = self.all_problems();
        let err_count = problems.iter().filter(|p| p.sev == Sev::Error).count();
        let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), PANEL_HEADER_H), Sense::hover());
        let mut x = hr.min.x + 12.0;
        let tabs = [
            (BottomTool::Repl, tr("REPL").to_string()),
            (BottomTool::Run, tr("Run").to_string()),
            (BottomTool::Terminal, tr("Terminal").to_string()),
            (BottomTool::Problems, if problems.is_empty() { tr("Problems").to_string() } else { format!("{}  {}", tr("Problems"), problems.len()) }),
            (BottomTool::Git, "Git".to_string()),
            (BottomTool::Find, if self.find_results.hits.is_empty() { tr("Find").to_string() } else { format!("{}  {}", tr("Find"), self.find_results.hits.len()) }),
        ];
        let mut switch_to = None;
        for (t, label) in tabs {
            let active = t == tool;
            let font = if active { fonts::semibold(13.5) } else { fonts::ui(13.5) };
            let g = ui.painter().layout_no_wrap(label.clone(), font.clone(), pal.text);
            let w = g.size().x + 26.0;
            let r = Rect::from_min_size(Pos2::new(x, hr.min.y), Vec2::new(w, hr.height()));
            let resp = ui.interact(r, ui.id().with(("btab", label.clone())), Sense::click());
            if active {
                ui.painter().rect_filled(r.shrink2(Vec2::new(0.0, 8.0)), CornerRadius::same(12), pal.tab_active_bg);
            } else if resp.hovered() {
                ui.painter().rect_filled(r.shrink2(Vec2::new(0.0, 8.0)), CornerRadius::same(12), pal.hover);
            }
            let color = if t == BottomTool::Problems && err_count > 0 { pal.error } else if active { pal.text } else { pal.text_dim };
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, font, color);
            if resp.clicked() {
                switch_to = Some(t);
            }
            x += w + 2.0;
        }
        let mut close = false;
        if self.header_action(ui, hr, 0, Icon::Close, "Hide") {
            close = true;
        }
        let mut slot = 1;
        match tool {
            BottomTool::Repl => {
                if self.header_action(ui, hr, slot, Icon::Clear, "Clear output") {
                    self.repl.log.clear();
                }
                slot += 1;
                if self.header_action(ui, hr, slot, Icon::Restart, "Restart REPL (Ctrl+Shift+F5)") {
                    self.repl_restart();
                }
                slot += 1;
                if self.header_action(ui, hr, slot, Icon::Stop, "Interrupt (restarts the REPL)") {
                    self.run_command(ctx, "run.stop");
                }
            }
            BottomTool::Run => {
                if self.header_action(ui, hr, slot, Icon::Clear, "Clear output") {
                    self.run.log.clear();
                }
                slot += 1;
                if self.header_action(ui, hr, slot, Icon::Stop, "Stop (Ctrl+F2)") {
                    self.stop_run();
                }
                slot += 1;
                if self.header_action(ui, hr, slot, Icon::Play, "Rerun (Shift+F10)") {
                    self.run_selected();
                }
            }
            BottomTool::Terminal => {
                if self.header_action(ui, hr, slot, Icon::Clear, "Clear") {
                    self.term.log.clear();
                }
                slot += 1;
                if self.header_action(ui, hr, slot, Icon::Stop, "Stop running command") {
                    self.terminal_stop();
                }
                slot += 1;
                if self.header_action(ui, hr, slot, Icon::Script, "Run script...") {
                    self.terminal_pick_script();
                }
            }
            BottomTool::Problems => {
                if self.header_action(ui, hr, slot, Icon::Clear, "Clear compiler problems") {
                    self.compiler_problems.clear();
                    for d in self.docs.iter_mut() {
                        d.compiler_markers.clear();
                    }
                }
            }
            BottomTool::Find => {
                if self.header_action(ui, hr, slot, Icon::Clear, "Clear results") {
                    self.find_results = FindResults::default();
                }
            }
            BottomTool::Git => {
                if self.header_action(ui, hr, slot, Icon::Refresh, "Refresh") {
                    self.git.rx = None;
                    self.git_refresh();
                }
            }
        }
        if let Some(t) = switch_to {
            self.bottom_tool = Some(t);
            if t == BottomTool::Repl && self.repl.session.is_none() {
                self.ensure_repl();
            }
            if t == BottomTool::Terminal {
                self.terminal_start_dir();
                self.term.focus_input = true;
            }
        }
        if close {
            self.bottom_tool = None;
            return;
        }
        match self.bottom_tool {
            Some(BottomTool::Repl) => self.draw_repl(ui, ctx),
            Some(BottomTool::Run) => self.draw_run(ui),
            Some(BottomTool::Terminal) => self.draw_terminal(ui, ctx),
            Some(BottomTool::Problems) => self.draw_problems(ui, &problems),
            Some(BottomTool::Find) => self.draw_find_results(ui),
            Some(BottomTool::Git) => self.draw_git(ui, ctx),
            None => {}
        }
    }

    fn log_color(&self, k: LogKind) -> Color32 {
        let p = &self.pal;
        match k {
            LogKind::Input => p.text,
            LogKind::Output => p.plain,
            LogKind::Value => p.number,
            LogKind::Error => p.error,
            LogKind::Info => p.text_dim,
            LogKind::Trace => p.text_dim,
        }
    }

    fn draw_log(&self, ui: &mut egui::Ui, id: &str, lines: &[LogLine], stick: bool, prompt: &str) {
        let font = egui::FontId::monospace((self.settings.font_size - 1.0).max(9.0));
        let row_h = (ui.fonts_mut(|f| f.row_height(&font)) + 5.0).round();
        egui::ScrollArea::both().id_salt(id).auto_shrink([false, false]).stick_to_bottom(stick).show_rows(ui, row_h, lines.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for l in &lines[range] {
                let text = l.text.trim_end_matches('\n');
                let (prefix, color) = match l.kind {
                    LogKind::Input => (format!("{prompt} "), self.log_color(l.kind)),
                    LogKind::Value => ("=> ".to_string(), self.log_color(l.kind)),
                    LogKind::Trace => ("  ".to_string(), self.log_color(l.kind)),
                    _ => (String::new(), self.log_color(l.kind)),
                };
                let mut job = egui::text::LayoutJob::default();
                job.append("", 14.0, egui::TextFormat::simple(font.clone(), color));
                if !prefix.is_empty() {
                    job.append(&prefix, 0.0, egui::TextFormat::simple(font.clone(), if l.kind == LogKind::Input { self.pal.ok } else { color }));
                }
                job.append(text, 0.0, egui::TextFormat::simple(font.clone(), color));
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
                let g = ui.painter().layout_job(job);
                ui.painter().galley(Pos2::new(rect.min.x, rect.center().y - g.size().y / 2.0), g, color);
            }
        });
    }

    /// Rounded single/multi-line input row shared by REPL and terminal: the prompt and the first line of text
    /// share one baseline and are centred in a field of the standard height.
    fn prompt_input(&mut self, ui: &mut egui::Ui, id: egui::Id, prompt: &str, which: u8, hint: &str) -> egui::Response {
        let pal = self.pal;
        let font = egui::FontId::monospace(self.settings.font_size);
        let row_h = ui.fonts_mut(|f| f.row_height(&font));
        let vpad = ((widgets::FIELD_H - row_h) / 2.0).max(4.0) as i8;
        let mut resp = None;
        egui::Frame::new().fill(pal.input_bg).corner_radius(widgets::radius()).inner_margin(Margin::symmetric(14, vpad)).outer_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // the default minimum control height (30) would make a one-line input taller than its text row
                ui.spacing_mut().interact_size.y = row_h;
                ui.label(RichText::new(prompt).font(font.clone()).color(pal.ok));
                let buf = if which == 0 { &mut self.repl.input } else { &mut self.term.input };
                let te = egui::TextEdit::multiline(buf)
                    .id(id)
                    .font(font.clone())
                    .text_color(pal.text)
                    .desired_rows(1)
                    .desired_width(f32::INFINITY)
                    .hint_text(RichText::new(hint).font(font.clone()).color(pal.text_dim))
                    .frame(egui::Frame::NONE)
                    .margin(Margin::ZERO)
                    .lock_focus(true);
                resp = Some(ui.add(te));
            });
        });
        resp.unwrap()
    }

    fn draw_repl(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let (dot, st) = if self.repl.starting {
                (pal.warning, tr("starting...").to_string())
            } else if self.repl.ready {
                (pal.ok, i18n::fmt("{0}   package {1}", &[&self.repl.version, &self.repl.package]))
            } else if self.lisp.is_none() {
                (pal.error, tr("no Lisp implementation found").to_string())
            } else {
                (pal.text_dim, tr("not running").to_string())
            };
            let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, dot);
            ui.label(RichText::new(st).color(pal.text_dim).font(fonts::ui(12.0)));
            if self.repl.session.is_none() && self.lisp.is_some() && widgets::secondary_button(ui, &pal, tr("Start")).clicked() {
                self.ensure_repl();
            }
        });
        let input_h = 64.0 + 20.0 * (self.repl.input.matches('\n').count().min(5) as f32);
        let avail = ui.available_height() - input_h;
        ui.allocate_ui(Vec2::new(ui.available_width(), avail.max(40.0)), |ui| {
            let stick = self.repl.stick_to_bottom;
            self.draw_log(ui, "repl_log", &self.repl.log, stick, ">");
        });
        let id = egui::Id::new("repl_input");
        let mut submit = false;
        let mut hist: Option<isize> = None;
        if ctx.memory(|m| m.has_focus(id)) {
            let (enter, shift, up, down) = ctx.input(|i| (i.key_pressed(Key::Enter), i.modifiers.shift, i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
            let complete = {
                let p = lisp_syntax::parse(&self.repl.input, lisp_syntax::Dialect::CommonLisp);
                !p.diagnostics.iter().any(|d| d.message.starts_with("Unclosed") || d.message.starts_with("Unterminated"))
            };
            let ctrl = ctx.input(|i| i.modifiers.command);
            if enter && !shift && (complete || ctrl) && !self.repl.input.trim().is_empty() {
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, Key::Enter);
                    i.consume_key(egui::Modifiers::COMMAND, Key::Enter);
                });
                submit = true;
            }
            if up && !self.repl.input.contains('\n') {
                hist = Some(-1);
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
            }
            if down && !self.repl.input.contains('\n') {
                hist = Some(1);
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
            }
        }
        let resp = self.prompt_input(ui, id, ">", 0, tr("Evaluate Lisp. Enter to run, Shift+Enter for a new line"));
        if self.repl.focus_input {
            resp.request_focus();
            self.repl.focus_input = false;
        }
        if resp.clicked() || resp.gained_focus() {
            self.editor_focus = false;
        }
        if let Some(dir) = hist {
            let n = self.repl.history.len() as isize;
            if n > 0 {
                let cur = self.repl.hist_pos.map_or(n, |p| p as isize);
                let next = (cur + dir).clamp(0, n);
                self.repl.hist_pos = if next == n { None } else { Some(next as usize) };
                self.repl.input = if next == n { String::new() } else { self.repl.history[next as usize].clone() };
            }
        }
        if submit {
            let code = std::mem::take(&mut self.repl.input);
            if self.repl.history.last() != Some(&code) {
                self.repl.history.push(code.clone());
            }
            self.repl.hist_pos = None;
            self.repl_eval(&code, true);
            resp.request_focus();
        }
    }

    fn draw_terminal(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        self.terminal_start_dir();
        ui.add_space(6.0);
        let scripts = self.project_scripts();
        let (mut run, mut pick) = (None::<std::path::PathBuf>, false);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let shown = match self.wsl_target() {
                Some(t) => format!("{}:{}", t.distro, lisp_runtime::wsl::to_linux(&self.term.cwd, &t)),
                None => self.term.cwd.display().to_string(),
            };
            ui.label(RichText::new(shown).color(pal.text_dim).font(fonts::ui(12.0)));
            if let Some(r) = &self.term.script {
                ui.label(RichText::new(i18n::fmt("script {0}: {1}/{2}", &[&r.name, &r.done, &r.total])).color(pal.accent).font(fonts::ui(12.0)));
            } else if self.term.handle.is_some() {
                ui.label(RichText::new(tr("running...")).color(pal.warning).font(fonts::ui(12.0)));
            }
            widgets::right_row(ui, 30.0, |ui| {
                ui.add_space(8.0);
                let resp = widgets::Pill::new(tr("Scripts")).icon(Icon::Script, pal.text_dim).chevron().bordered().height(28.0).show(ui, &pal);
                egui::Popup::menu(&resp).width(320.0).show(|ui| {
                    if scripts.is_empty() {
                        ui.label(RichText::new(tr("No scripts in the project (.als, .bat, .cmd, .ps1, .sh)")).color(pal.text_dim).size(11.5));
                    }
                    for (rel, abs) in &scripts {
                        if ui.button(rel).clicked() {
                            run = Some(abs.clone());
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button(tr("Choose a file...")).clicked() {
                        pick = true;
                        ui.close();
                    }
                });
            });
        });
        if let Some(p) = run {
            self.terminal_run_script(&p);
        }
        if pick {
            self.terminal_pick_script();
        }
        let avail = ui.available_height() - 64.0;
        ui.allocate_ui(Vec2::new(ui.available_width(), avail.max(40.0)), |ui| {
            self.draw_log(ui, "term_log", &self.term.log, true, "$");
        });
        let id = egui::Id::new("term_input");
        let mut submit = false;
        let mut hist: Option<isize> = None;
        if ctx.memory(|m| m.has_focus(id)) {
            let (enter, up, down) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
            if enter && !self.term.input.trim().is_empty() {
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                submit = true;
            }
            if up {
                hist = Some(-1);
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
            }
            if down {
                hist = Some(1);
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
            }
        }
        let resp = self.prompt_input(ui, id, "$", 1, tr("Type a command and press Enter"));
        if self.term.focus_input {
            resp.request_focus();
            self.term.focus_input = false;
        }
        if resp.clicked() || resp.gained_focus() {
            self.editor_focus = false;
        }
        if let Some(dir) = hist {
            let n = self.term.history.len() as isize;
            if n > 0 {
                let cur = self.term.hist_pos.map_or(n, |p| p as isize);
                let next = (cur + dir).clamp(0, n);
                self.term.hist_pos = if next == n { None } else { Some(next as usize) };
                self.term.input = if next == n { String::new() } else { self.term.history[next as usize].clone() };
            }
        }
        if submit {
            let line = std::mem::take(&mut self.term.input);
            self.terminal_run(&line);
            resp.request_focus();
        }
    }

    fn draw_run(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let (txt, color) = if self.run.running { (tr("running...").to_string(), pal.warning) } else if self.run.exit_code.is_some() { (self.run.title.clone(), pal.text_dim) } else { (tr("Nothing has been run yet. Press Shift+F10 to run the selected configuration.").to_string(), pal.text_dim) };
            ui.label(RichText::new(txt).color(color).font(fonts::ui(12.0)));
        });
        if self.run.log.is_empty() && self.run.handle.is_none() {
            return;
        }
        self.draw_log(ui, "run_log", &self.run.log, true, "$");
    }

    pub fn empty_state(&self, ui: &mut egui::Ui, icon: Icon, color: Color32, text: &str) {
        let pal = self.pal;
        ui.add_space(26.0);
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            icons::paint(ui.painter(), r, icon, color);
            ui.add_space(8.0);
            ui.label(RichText::new(text).color(pal.text_dim));
        });
    }

    fn draw_problems(&mut self, ui: &mut egui::Ui, problems: &[ProblemRow]) {
        let pal = self.pal;
        if problems.is_empty() {
            self.empty_state(ui, Icon::Check, pal.ok, tr("No problems found in open files"));
            return;
        }
        let mut goto: Option<(Option<PathBuf>, usize, usize)> = None;
        let root = self.project_root().map(PathBuf::from);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for p in problems {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H + 4.0), Sense::click());
                widgets::row_background(ui, &pal, r, false, resp.hovered());
                let (icon, color) = match p.sev {
                    Sev::Error => (Icon::Error, pal.error),
                    Sev::Warning => (Icon::Warning, pal.warning),
                    Sev::Weak => (Icon::Info, pal.text_dim),
                };
                icons::paint(ui.painter(), Rect::from_min_size(Pos2::new(r.min.x + 18.0, r.center().y - 8.0), Vec2::splat(16.0)), icon, color);
                let g = ui.painter().layout_no_wrap(p.message.clone(), fonts::ui(13.5), pal.text);
                let gw = g.size().x;
                ui.painter().galley(Pos2::new(r.min.x + 44.0, r.center().y - g.size().y / 2.0), g, pal.text);
                let loc = match (&p.path, &root) {
                    (Some(path), Some(root)) => ide_core::workspace::relative(root, path),
                    (Some(path), None) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    _ => String::new(),
                };
                let line = self.docs.iter().find(|d| d.path == p.path).map(|d| d.ed.line_col(p.start.min(d.ed.text().len())).0 + 1);
                let loc = match line {
                    Some(l) if !loc.is_empty() => format!("{loc}:{l}"),
                    _ => loc,
                };
                ui.painter().text(Pos2::new(r.min.x + 58.0 + gw, r.center().y), egui::Align2::LEFT_CENTER, format!("{loc}   {}", p.source), fonts::ui(12.0), pal.text_dim);
                if resp.clicked() {
                    goto = Some((p.path.clone(), p.start, p.end));
                }
            }
        });
        if let Some((Some(path), s, e)) = goto {
            self.goto_range(&path, s, e, true);
        }
    }

    fn draw_find_results(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        if self.find_results.hits.is_empty() && self.find_rx.is_some() {
            self.empty_state(ui, Icon::Search, pal.accent, tr("Searching the project..."));
            return;
        }
        if self.find_results.hits.is_empty() {
            self.empty_state(ui, Icon::Search, pal.text_dim, tr("No results. Ctrl+Shift+F searches the project, Alt+F7 finds usages."));
            return;
        }
        let hits = self.find_results.hits.clone();
        let title = self.find_results.title.clone();
        let root = self.project_root().map(PathBuf::from);
        let mut goto: Option<(PathBuf, usize, usize)> = None;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(RichText::new(i18n::fmt("{0}   {1} match(es)", &[&title, &hits.len()])).color(pal.text_dim).font(fonts::ui(12.0)));
        });
        let mono = egui::FontId::monospace((self.settings.font_size - 1.0).max(9.0));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut last: Option<&PathBuf> = None;
            for h in &hits {
                if last != Some(&h.path) {
                    last = Some(&h.path);
                    let rel = root.as_ref().map(|r| ide_core::workspace::relative(r, &h.path)).unwrap_or_else(|| h.path.to_string_lossy().into_owned());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                        icons::paint(ui.painter(), r, Icon::Lisp, pal.keyword_form);
                        ui.label(RichText::new(rel).font(fonts::semibold(13.0)));
                    });
                }
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
                widgets::row_background(ui, &pal, r, false, resp.hovered());
                ui.painter().text(Pos2::new(r.min.x + 40.0, r.center().y), egui::Align2::LEFT_CENTER, format!("{:>4}", h.line + 1), mono.clone(), pal.text_dim);
                let text = h.line_text.trim_start();
                let lead = h.line_text.chars().count() - text.chars().count();
                let col = h.col.saturating_sub(lead);
                let pre: String = text.chars().take(col).collect();
                let mid: String = text.chars().skip(col).take(h.len).collect();
                let post: String = text.chars().skip(col + h.len).take(200).collect();
                let mut job = egui::text::LayoutJob::default();
                job.append(&pre, 0.0, egui::TextFormat::simple(mono.clone(), pal.plain));
                job.append(&mid, 0.0, egui::TextFormat { background: pal.find_match, ..egui::TextFormat::simple(mono.clone(), pal.text) });
                job.append(&post, 0.0, egui::TextFormat::simple(mono.clone(), pal.plain));
                let g = ui.painter().layout_job(job);
                ui.painter().galley(Pos2::new(r.min.x + 84.0, r.center().y - g.size().y / 2.0), g, pal.plain);
                if resp.clicked() {
                    let off = std::fs::read_to_string(&h.path).ok().map(|t| {
                        let ls = t.split_inclusive('\n').take(h.line).map(str::len).sum::<usize>();
                        let line = t[ls..].lines().next().unwrap_or("");
                        let b = line.char_indices().nth(h.col).map_or(line.len(), |(b, _)| b);
                        let e = line.char_indices().nth(h.col + h.len).map_or(line.len(), |(b, _)| b);
                        (ls + b, ls + e)
                    });
                    if let Some((s, e)) = off {
                        goto = Some((h.path.clone(), s, e));
                    }
                }
            }
        });
        if let Some((p, s, e)) = goto {
            self.goto_range(&p, s, e, true);
        }
    }

    // ------------------------------------------------------------------ git tool window

    fn draw_git(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let pal = self.pal;
        if self.project.is_none() {
            self.empty_state(ui, Icon::Branch, pal.text_dim, tr("Open a project to use version control"));
            return;
        }
        ui.add_space(8.0);
        let mut commit_local = false;
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let local_n = self.git.local.as_ref().map_or(0, |l| l.commits().len());
            let tabs = [(GitTab::Log, tr("Log").to_string()), (GitTab::Local, i18n::fmt("Local ({})", &[&local_n]))];
            for (t, label) in tabs {
                let active = self.git.tab == t;
                let g = ui.painter().layout_no_wrap(label.clone(), fonts::ui(13.0), pal.text);
                let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, 28.0), Sense::click());
                if active {
                    ui.painter().rect_filled(r, CornerRadius::same(7), pal.selected.gamma_multiply(0.6));
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(7), pal.hover);
                }
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, fonts::ui(13.0), if active { pal.text } else { pal.text_dim });
                if resp.clicked() {
                    self.git.tab = t;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                if widgets::secondary_button(ui, &pal, tr("Local Commit")).clicked() {
                    commit_local = true;
                }
            });
        });
        ui.add_space(6.0);
        if commit_local {
            self.popup = Popup::LocalCommit { input: String::new(), just_opened: true };
        }
        match self.git.tab {
            GitTab::Log => self.draw_git_log(ui),
            GitTab::Local => self.draw_local_commits(ui),
        }
    }

    fn draw_git_log(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        if self.git.repo.is_none() {
            self.empty_state(ui, Icon::History, pal.text_dim, tr("No git history: this project is not a Git repository"));
            return;
        }
        if self.git.log.is_empty() {
            self.empty_state(ui, Icon::History, pal.text_dim, tr("No commits yet"));
            return;
        }
        let log = self.git.log.clone();
        let graph = std::mem::take(&mut self.git.graph);
        let cols = graph.iter().map(|g| g.width).max().unwrap_or(1).clamp(1, MAX_LANES);
        let head = self.git.head.clone();
        let mut show: Option<usize> = None;
        let row_h = ROW_H + 2.0;
        egui::ScrollArea::vertical().id_salt("git_log").auto_shrink([false, false]).show_rows(ui, row_h, log.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in range {
                let c = &log[i];
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                widgets::row_background(ui, &pal, r, false, resp.hovered());
                let cy = r.center().y;
                let lane_x = |l: usize| r.min.x + 22.0 + l.min(MAX_LANES - 1) as f32 * LANE_W;
                let mut tx = r.min.x + 22.0 + cols as f32 * LANE_W + 8.0;
                if let Some(g) = graph.get(i) {
                    let p = ui.painter();
                    let stroke = |color: usize| Stroke::new(1.8, lane_color(color));
                    let node = Pos2::new(lane_x(g.lane), cy);
                    for &(l, col) in &g.through {
                        p.line_segment([Pos2::new(lane_x(l), r.min.y), Pos2::new(lane_x(l), r.max.y)], stroke(col));
                    }
                    for &(l, col) in &g.incoming {
                        lane_edge(p, Pos2::new(lane_x(l), r.min.y), node, stroke(col));
                    }
                    for &(l, col) in &g.outgoing {
                        lane_edge(p, node, Pos2::new(lane_x(l), r.max.y), stroke(col));
                    }
                    let is_head = head.as_deref() == Some(c.hash.as_str());
                    if c.parents.len() > 1 {
                        p.circle_filled(node, 5.0, pal.panel_bg);
                        p.circle_stroke(node, 4.0, Stroke::new(2.0, lane_color(g.color)));
                    } else {
                        p.circle_filled(node, 4.5, lane_color(g.color));
                    }
                    if is_head {
                        p.circle_stroke(node, 7.5, Stroke::new(1.5, pal.text));
                    }
                } else {
                    ui.painter().circle_filled(Pos2::new(lane_x(0), cy), 4.0, pal.text_dim);
                }
                ui.painter().text(Pos2::new(tx, cy), egui::Align2::LEFT_CENTER, &c.short, egui::FontId::monospace(12.5), pal.text_dim);
                tx += 70.0;
                let g = ui.painter().layout_no_wrap(c.subject.clone(), fonts::ui(13.5), pal.text);
                let gw = g.size().x.min((r.max.x - tx) * 0.5).max(40.0);
                let clip = ui.painter().with_clip_rect(Rect::from_min_max(Pos2::new(tx, r.min.y), Pos2::new(tx + gw, r.max.y)));
                clip.galley(Pos2::new(tx, cy - g.size().y / 2.0), g, pal.text);
                tx += gw + 10.0;
                for name in &c.refs {
                    tx = ref_chip(ui.painter(), &pal, Pos2::new(tx, cy), name) + 6.0;
                }
                ui.painter().text(Pos2::new(r.max.x - 12.0, cy), egui::Align2::RIGHT_CENTER, format!("{}   {}", c.author, c.when), fonts::ui(12.0), pal.text_dim);
                if resp.clicked() {
                    show = Some(i);
                }
            }
        });
        self.git.graph = graph;
        if let Some(i) = show {
            self.git_show_commit(i);
        }
    }
}


const LANE_W: f32 = 16.0;
const MAX_LANES: usize = 8;

/// Branch line colours; the graph cycles through them.
fn lane_color(i: usize) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(0x5A, 0x86, 0xF5),
        Color32::from_rgb(0x4C, 0xB0, 0x7A),
        Color32::from_rgb(0xE0, 0xA0, 0x40),
        Color32::from_rgb(0xB0, 0x7C, 0xE8),
        Color32::from_rgb(0xE0, 0x6C, 0x75),
        Color32::from_rgb(0x3C, 0xB8, 0xC4),
        Color32::from_rgb(0xD6, 0x6F, 0xB0),
        Color32::from_rgb(0x8C, 0xA8, 0x4B),
    ];
    COLORS[i % COLORS.len()]
}

/// A graph edge between two points: straight when they share a lane, an S-curve otherwise.
fn lane_edge(p: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    if (from.x - to.x).abs() < 0.5 {
        p.line_segment([from, to], stroke);
    } else {
        let mid = (from.y + to.y) / 2.0;
        p.add(egui::epaint::CubicBezierShape::from_points_stroke([from, Pos2::new(from.x, mid), Pos2::new(to.x, mid), to], false, Color32::TRANSPARENT, stroke));
    }
}

/// A small label for a branch/tag/HEAD decoration; returns the x where it ends.
fn ref_chip(p: &egui::Painter, pal: &crate::theme::Palette, at: Pos2, raw: &str) -> f32 {
    let (text, color) = if let Some(cur) = raw.strip_prefix("HEAD -> ") {
        (cur.to_string(), pal.accent)
    } else if let Some(tag) = raw.strip_prefix("tag: ") {
        (tag.to_string(), pal.warning)
    } else if raw.contains('/') {
        (raw.to_string(), pal.text_dim)
    } else {
        (raw.to_string(), pal.ok)
    };
    let g = p.layout_no_wrap(text, fonts::semibold(11.5), color);
    let rect = Rect::from_min_size(Pos2::new(at.x, at.y - 9.0), Vec2::new(g.size().x + 14.0, 18.0));
    p.rect_filled(rect, CornerRadius::same(7), color.gamma_multiply(0.2));
    p.galley(Pos2::new(rect.min.x + 7.0, rect.center().y - g.size().y / 2.0), g, color);
    rect.max.x
}
