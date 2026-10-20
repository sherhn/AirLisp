//! Tool windows: Project tree, Structure, REPL, Run, Problems, Find results.

use std::path::PathBuf;

use eframe::egui::{self, Align, Color32, CornerRadius, FontId, Key, Layout, Pos2, Rect, RichText, Sense, Vec2};
use lisp_syntax::outline::{SymbolKind, definition_at, outline};

use crate::app::App;
use crate::editor_view::Sev;
use crate::icons::{self, Icon};
use crate::state::*;

const ROW_H: f32 = 22.0;

struct TreeRow {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
    expanded: bool,
    name: String,
}

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
        "F" => p.accent,
        _ => p.text_dim,
    }
}


impl App {
    // ------------------------------------------------------------------ project tree

    fn collect_tree_rows(&mut self) -> Vec<TreeRow> {
        let Some(root) = self.project.as_ref().map(|p| p.root.clone()) else { return vec![] };
        let mut rows = Vec::new();
        let mut stack: Vec<(PathBuf, usize)> = vec![(root.clone(), 0)];
        // iterative DFS keeping directory order
        fn walk(app: &mut App, dir: PathBuf, depth: usize, rows: &mut Vec<TreeRow>) {
            let entries = app.dir_entries(&dir);
            for e in entries {
                let expanded = e.is_dir && app.project.as_ref().is_some_and(|p| p.expanded.contains(&e.path));
                rows.push(TreeRow { path: e.path.clone(), depth, is_dir: e.is_dir, expanded, name: e.name.clone() });
                if expanded {
                    walk(app, e.path, depth + 1, rows);
                }
            }
        }
        if let Some((root, _)) = stack.pop() {
            let name = self.project.as_ref().map(|p| p.name.clone()).unwrap_or_default();
            rows.push(TreeRow { path: root.clone(), depth: 0, is_dir: true, expanded: true, name });
            let expanded = self.project.as_ref().is_some_and(|p| p.expanded.contains(&root));
            if expanded {
                walk(self, root, 1, &mut rows);
            } else if let Some(r) = rows.first_mut() {
                r.expanded = false;
            }
        }
        rows
    }

    pub fn draw_project_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        self.panel_header(ui, "Project");
        if self.project.is_none() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No project is open").color(pal.text_dim));
                ui.add_space(8.0);
                if ui.button("Open Project...").clicked() {
                    self.run_command(ctx, "file.open_project");
                }
                if ui.button("New Project...").clicked() {
                    self.run_command(ctx, "file.new_project");
                }
            });
            return;
        }
        let rows = self.collect_tree_rows();
        let mut toggle: Option<PathBuf> = None;
        let mut open: Option<PathBuf> = None;
        let mut select: Option<PathBuf> = None;
        let mut action: Option<(String, PathBuf)> = None;
        let selected = self.project.as_ref().and_then(|p| p.selected.clone());
        let active_path = self.doc().and_then(|d| d.path.clone());
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            for row in &rows {
                let w = ui.available_width().max(200.0);
                let (r, resp) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::click());
                let is_sel = selected.as_ref() == Some(&row.path);
                if is_sel {
                    ui.painter().rect_filled(r, CornerRadius::same(4), pal.selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(4), pal.hover);
                }
                let x = r.min.x + 6.0 + row.depth as f32 * 16.0;
                if row.is_dir {
                    icons::paint(ui.painter(), Rect::from_min_size(Pos2::new(x, r.center().y - 6.0), Vec2::splat(12.0)), if row.expanded { Icon::ChevronDown } else { Icon::ChevronRight }, pal.text_dim);
                }
                let ir = Rect::from_min_size(Pos2::new(x + 16.0, r.center().y - 7.0), Vec2::splat(14.0));
                if row.is_dir {
                    icons::paint(ui.painter(), ir, if row.expanded { Icon::FolderOpen } else { Icon::Folder }, pal.builtin.gamma_multiply(0.85));
                } else if lisp_syntax::dialect::is_lisp_file(&row.path) {
                    icons::paint(ui.painter(), ir, Icon::Lisp, pal.keyword_form);
                } else {
                    icons::paint(ui.painter(), ir, Icon::File, pal.text_dim);
                }
                let is_active = active_path.as_ref() == Some(&row.path);
                ui.painter().text(Pos2::new(x + 36.0, r.center().y), egui::Align2::LEFT_CENTER, &row.name, FontId::proportional(13.0), if is_active { pal.accent } else { pal.text });
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
                    if ui.button("New File...").clicked() {
                        action = Some(("new_file".into(), dir.clone()));
                        ui.close();
                    }
                    if ui.button("New Directory...").clicked() {
                        action = Some(("new_dir".into(), dir.clone()));
                        ui.close();
                    }
                    ui.separator();
                    if !row.is_dir && ui.button("Open").clicked() {
                        open = Some(row.path.clone());
                        ui.close();
                    }
                    if ui.button("Rename...").clicked() {
                        action = Some(("rename".into(), row.path.clone()));
                        ui.close();
                    }
                    if ui.button("Delete...").clicked() {
                        action = Some(("delete".into(), row.path.clone()));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Copy Path").clicked() {
                        ctx.copy_text(row.path.to_string_lossy().into_owned());
                        ui.close();
                    }
                    if ui.button("Reveal in File Manager").clicked() {
                        crate::input::reveal_in_file_manager(&row.path);
                        ui.close();
                    }
                    if ui.button("Refresh").clicked() {
                        action = Some(("refresh".into(), row.path.clone()));
                        ui.close();
                    }
                });
            }
        });
        if let Some(p) = select
            && let Some(proj) = self.project.as_mut()
        {
            proj.selected = Some(p);
        }
        if let Some(p) = toggle
            && let Some(proj) = self.project.as_mut()
            && !proj.expanded.remove(&p)
        {
            proj.expanded.insert(p);
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
                    self.popup = Popup::Confirm { message: format!("Delete '{name}'? This cannot be undone."), kind: ConfirmKind::Delete(path) };
                }
                "refresh" => self.refresh_project(),
                _ => {}
            }
        }
    }

    pub fn panel_header(&mut self, ui: &mut egui::Ui, title: &str) {
        let pal = self.pal;
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
        ui.painter().text(Pos2::new(r.min.x + 12.0, r.center().y), egui::Align2::LEFT_CENTER, title, FontId::proportional(13.0), pal.text);
        ui.painter().line_segment([r.left_bottom(), r.right_bottom()], egui::Stroke::new(1.0, pal.border));
    }

    // ------------------------------------------------------------------ structure

    pub fn draw_structure_panel(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        self.panel_header(ui, "Structure");
        let Some(d) = self.doc() else {
            ui.add_space(12.0);
            ui.label(RichText::new("  No file is open").color(pal.text_dim));
            return;
        };
        let defs = outline(d.ed.text(), d.ed.dialect());
        let caret = d.ed.caret();
        let current = definition_at(&defs, caret).map(|x| x.start);
        let mut goto: Option<usize> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if defs.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("  Nothing to show").color(pal.text_dim));
            }
            for def in &defs {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
                if current == Some(def.start) {
                    ui.painter().rect_filled(r, CornerRadius::same(4), pal.selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(4), pal.hover);
                }
                let c = kind_color(self, def.kind);
                let br = Rect::from_min_size(Pos2::new(r.min.x + 8.0, r.center().y - 8.0), Vec2::splat(16.0));
                ui.painter().rect_filled(br, CornerRadius::same(4), c.gamma_multiply(0.25));
                ui.painter().text(br.center(), egui::Align2::CENTER_CENTER, def.kind.badge(), FontId::proportional(10.5), c);
                ui.painter().text(Pos2::new(r.min.x + 32.0, r.center().y), egui::Align2::LEFT_CENTER, &def.name, FontId::proportional(13.0), pal.text);
                if resp.clicked() {
                    goto = Some(def.name_start);
                }
            }
        });
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
        // header
        let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
        ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, pal.border));
        let mut x = hr.min.x + 8.0;
        let tabs = [
            (BottomTool::Repl, "REPL".to_string()),
            (BottomTool::Run, "Run".to_string()),
            (BottomTool::Problems, if problems.is_empty() { "Problems".to_string() } else { format!("Problems {}", problems.len()) }),
            (BottomTool::Find, if self.find_results.hits.is_empty() { "Find".to_string() } else { format!("Find {}", self.find_results.hits.len()) }),
        ];
        let mut switch_to = None;
        for (t, label) in tabs {
            let g = ui.painter().layout_no_wrap(label.clone(), FontId::proportional(13.0), pal.text);
            let w = g.size().x + 20.0;
            let r = Rect::from_min_size(Pos2::new(x, hr.min.y), Vec2::new(w, hr.height()));
            let resp = ui.interact(r, ui.id().with(("btab", label.clone())), Sense::click());
            let active = t == tool;
            if resp.hovered() && !active {
                ui.painter().rect_filled(r.shrink2(Vec2::new(0.0, 3.0)), CornerRadius::same(4), pal.hover);
            }
            let color = if t == BottomTool::Problems && err_count > 0 { pal.error } else if active { pal.text } else { pal.text_dim };
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(13.0), color);
            if active {
                ui.painter().rect_filled(Rect::from_min_size(Pos2::new(r.min.x + 6.0, r.max.y - 3.0), Vec2::new(w - 12.0, 3.0)), CornerRadius::same(1), pal.accent);
            }
            if resp.clicked() {
                switch_to = Some(t);
            }
            x += w + 2.0;
        }
        // right side actions
        let mut close = false;
        let mut hx = hr.max.x - 8.0 - 24.0;
        let cr = Rect::from_min_size(Pos2::new(hx, hr.center().y - 12.0), Vec2::splat(24.0));
        let resp = ui.interact(cr, ui.id().with("bclose"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(cr, CornerRadius::same(4), pal.hover);
        }
        icons::paint(ui.painter(), cr.shrink(6.0), Icon::Close, pal.text_dim);
        if resp.on_hover_text("Hide").clicked() {
            close = true;
        }
        hx -= 28.0;
        let header_btn = |ui: &mut egui::Ui, icon: Icon, tip: &str, color: Color32, hx: &mut f32| -> bool {
            let r = Rect::from_min_size(Pos2::new(*hx, hr.center().y - 12.0), Vec2::splat(24.0));
            *hx -= 28.0;
            let resp = ui.interact(r, ui.id().with(("bh", tip)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(4), pal.hover);
            }
            icons::paint(ui.painter(), r.shrink(5.0), icon, color);
            resp.on_hover_text(tip).clicked()
        };
        match tool {
            BottomTool::Repl => {
                if header_btn(ui, Icon::Clear, "Clear output", pal.text_dim, &mut hx) {
                    self.repl.log.clear();
                }
                if header_btn(ui, Icon::Restart, "Restart REPL (Ctrl+Shift+F5)", pal.ok, &mut hx) {
                    self.repl_restart();
                }
                if header_btn(ui, Icon::Stop, "Interrupt (restarts the REPL)", pal.error, &mut hx) {
                    self.run_command(ctx, "run.stop");
                }
            }
            BottomTool::Run => {
                if header_btn(ui, Icon::Clear, "Clear output", pal.text_dim, &mut hx) {
                    self.run.log.clear();
                }
                if header_btn(ui, Icon::Stop, "Stop (Ctrl+F2)", pal.error, &mut hx) {
                    self.stop_run();
                }
                if header_btn(ui, Icon::Play, "Rerun (Shift+F10)", pal.ok, &mut hx) {
                    self.run_current_file();
                }
            }
            BottomTool::Problems => {
                if header_btn(ui, Icon::Clear, "Clear compiler problems", pal.text_dim, &mut hx) {
                    self.compiler_problems.clear();
                    for d in self.docs.iter_mut() {
                        d.compiler_markers.clear();
                    }
                }
            }
            BottomTool::Find => {
                if header_btn(ui, Icon::Clear, "Clear results", pal.text_dim, &mut hx) {
                    self.find_results = FindResults::default();
                }
            }
        }
        if let Some(t) = switch_to {
            self.bottom_tool = Some(t);
            if t == BottomTool::Repl && self.repl.session.is_none() {
                self.ensure_repl();
            }
        }
        if close {
            self.bottom_tool = None;
            return;
        }
        match self.bottom_tool {
            Some(BottomTool::Repl) => self.draw_repl(ui, ctx),
            Some(BottomTool::Run) => self.draw_run(ui),
            Some(BottomTool::Problems) => self.draw_problems(ui, &problems),
            Some(BottomTool::Find) => self.draw_find_results(ui),
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
        let font = FontId::monospace((self.settings.font_size - 1.0).max(9.0));
        let row_h = ui.fonts_mut(|f| f.row_height(&font)) + 2.0;
        egui::ScrollArea::both().id_salt(id).auto_shrink([false, false]).stick_to_bottom(stick).show_rows(ui, row_h, lines.len(), |ui, range| {
            for l in &lines[range] {
                let text = l.text.trim_end_matches('\n');
                let (prefix, color) = match l.kind {
                    LogKind::Input => (format!("{prompt} "), self.log_color(l.kind)),
                    LogKind::Value => ("=> ".to_string(), self.log_color(l.kind)),
                    LogKind::Trace => ("  ".to_string(), self.log_color(l.kind)),
                    _ => (String::new(), self.log_color(l.kind)),
                };
                let mut job = egui::text::LayoutJob::default();
                if !prefix.is_empty() {
                    job.append(&prefix, 0.0, egui::TextFormat::simple(font.clone(), if l.kind == LogKind::Input { self.pal.ok } else { color }));
                }
                job.append(text, 0.0, egui::TextFormat::simple(font.clone(), color));
                ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
            }
        });
    }

    fn draw_repl(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        // status line
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let st = if self.repl.starting {
                "starting...".to_string()
            } else if self.repl.ready {
                format!("{}  -  package {}", self.repl.version, self.repl.package)
            } else if self.lisp.is_none() {
                "no Lisp implementation found".to_string()
            } else {
                "not running".to_string()
            };
            ui.label(RichText::new(st).color(pal.text_dim).size(12.0));
            if self.repl.session.is_none() && self.lisp.is_some() && ui.small_button("Start").clicked() {
                self.ensure_repl();
            }
        });
        let input_h = 30.0 + 18.0 * (self.repl.input.matches('\n').count().min(5) as f32);
        let avail = ui.available_height() - input_h - 10.0;
        ui.allocate_ui(Vec2::new(ui.available_width(), avail.max(40.0)), |ui| {
            let stick = self.repl.stick_to_bottom;
            self.draw_log(ui, "repl_log", &self.repl.log, stick, ">");
        });
        // input
        ui.separator();
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(14.0, 20.0), Sense::hover());
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, ">", FontId::monospace(14.0), pal.ok);
            let id = egui::Id::new("repl_input");
            let mut submit = false;
            let mut hist: Option<isize> = None;
            let focused = ctx.memory(|m| m.has_focus(id));
            if focused {
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
            let te = egui::TextEdit::multiline(&mut self.repl.input)
                .id(id)
                .font(FontId::monospace(self.settings.font_size))
                .desired_rows(1)
                .desired_width(f32::INFINITY)
                .hint_text("Evaluate Lisp. Enter to run, Shift+Enter for a new line")
                .frame(egui::Frame::NONE)
                .lock_focus(true);
            let resp = ui.add(te);
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
        });
    }

    fn draw_run(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (txt, color) = if self.run.running { ("running...".to_string(), pal.warning) } else if self.run.exit_code.is_some() { (self.run.title.to_string(), pal.text_dim) } else { ("Nothing has been run yet. Press Shift+F10 to run the current file.".to_string(), pal.text_dim) };
            ui.label(RichText::new(txt).color(color).size(12.0));
        });
        if self.run.log.is_empty() && self.run.handle.is_none() {
            return;
        }
        self.draw_log(ui, "run_log", &self.run.log, true, "$");
    }

    fn draw_problems(&mut self, ui: &mut egui::Ui, problems: &[ProblemRow]) {
        let pal = self.pal;
        if problems.is_empty() {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                icons::paint(ui.painter(), r, Icon::Check, pal.ok);
                ui.label(RichText::new("No problems found in open files").color(pal.text_dim));
            });
            return;
        }
        let mut goto: Option<(Option<PathBuf>, usize, usize)> = None;
        let root = self.project_root().map(PathBuf::from);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for p in problems {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H + 2.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(4), pal.hover);
                }
                let (icon, color) = match p.sev {
                    Sev::Error => (Icon::Error, pal.error),
                    Sev::Warning => (Icon::Warning, pal.warning),
                    Sev::Weak => (Icon::Info, pal.text_dim),
                };
                icons::paint(ui.painter(), Rect::from_min_size(Pos2::new(r.min.x + 10.0, r.center().y - 7.0), Vec2::splat(14.0)), icon, color);
                let g = ui.painter().layout_no_wrap(p.message.clone(), FontId::proportional(13.0), pal.text);
                let gw = g.size().x;
                ui.painter().galley(Pos2::new(r.min.x + 32.0, r.center().y - g.size().y / 2.0), g, pal.text);
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
                ui.painter().text(Pos2::new(r.min.x + 44.0 + gw, r.center().y), egui::Align2::LEFT_CENTER, format!("{loc}  [{}]", p.source), FontId::proportional(12.0), pal.text_dim);
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
        if self.find_results.hits.is_empty() {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new("No results. Use Ctrl+Shift+F to search the project or Alt+F7 for usages.").color(pal.text_dim));
            });
            return;
        }
        let hits = self.find_results.hits.clone();
        let title = self.find_results.title.clone();
        let root = self.project_root().map(PathBuf::from);
        let mut goto: Option<(PathBuf, usize, usize)> = None;
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(format!("{title}  -  {} match(es)", hits.len())).color(pal.text_dim).size(12.0));
        });
        let mono = FontId::monospace((self.settings.font_size - 1.0).max(9.0));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut last: Option<&PathBuf> = None;
            for h in &hits {
                if last != Some(&h.path) {
                    last = Some(&h.path);
                    let rel = root.as_ref().map(|r| ide_core::workspace::relative(r, &h.path)).unwrap_or_else(|| h.path.to_string_lossy().into_owned());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        icons::paint(ui.painter(), r, Icon::Lisp, pal.keyword_form);
                        ui.label(RichText::new(rel).strong().size(13.0));
                    });
                }
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(4), pal.hover);
                }
                ui.painter().text(Pos2::new(r.min.x + 32.0, r.center().y), egui::Align2::LEFT_CENTER, format!("{:>4}", h.line + 1), mono.clone(), pal.text_dim);
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
                ui.painter().galley(Pos2::new(r.min.x + 72.0, r.center().y - g.size().y / 2.0), g, pal.plain);
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
        let _ = Layout::top_down(Align::Min);
    }
}
