//! Popups and dialogs: Search Everywhere & co, input dialogs, commit, diff, settings, completion,
//! find bar and the welcome screen.

use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Key, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use ide_core::commands;
use ide_core::diff::LineKind;
use ide_core::settings::Theme;

use crate::app::App;
use crate::editor_view::Sev;
use crate::fonts;
use crate::icons::{self, Icon};
use crate::state::*;
use crate::ui_panels::badge_color;
use crate::widgets;

fn popup_frame(pal: &crate::theme::Palette) -> egui::Frame {
    widgets::glass_frame(pal)
}

fn highlighted(text: &str, hits: &[usize], font: FontId, base: Color32, hit: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut buf = String::new();
    let mut cur = false;
    for (i, ch) in text.chars().enumerate() {
        let is_hit = hits.contains(&i);
        if is_hit != cur && !buf.is_empty() {
            job.append(&buf, 0.0, egui::TextFormat::simple(font.clone(), if cur { hit } else { base }));
            buf.clear();
        }
        cur = is_hit;
        buf.push(ch);
    }
    if !buf.is_empty() {
        job.append(&buf, 0.0, egui::TextFormat::simple(font, if cur { hit } else { base }));
    }
    job
}

enum PaletteAction {
    None,
    Close,
    Run(Item),
}

impl App {
    pub fn draw_popups(&mut self, ctx: &egui::Context) {
        if self.popup.is_open() && !matches!(self.popup, Popup::Palette(_)) {
            widgets::scrim(ctx, &self.pal);
        }
        self.draw_palette(ctx);
        self.draw_clone(ctx);
        self.draw_input_dialogs(ctx);
        self.draw_commit_dialog(ctx);
        self.draw_diff(ctx);
        self.draw_settings(ctx);
        self.draw_info_dialogs(ctx);
        self.draw_completion(ctx);
        self.draw_hint(ctx);
    }

    // ------------------------------------------------------------------ palette

    fn draw_palette(&mut self, ctx: &egui::Context) {
        if !matches!(self.popup, Popup::Palette(_)) {
            return;
        }
        self.tick_palette();
        let Popup::Palette(mut st) = std::mem::replace(&mut self.popup, Popup::None) else { return };
        let pal = self.pal;
        let mut action = PaletteAction::None;
        let screen = ctx.content_rect();
        let width = (screen.width() * 0.5).clamp(560.0, 780.0);
        let (esc, down, up, enter, tab) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::Enter), i.key_pressed(Key::Tab)));
        if esc {
            action = PaletteAction::Close;
        }
        let n = st.items.len();
        let mut moved = false;
        if down && n > 0 {
            st.selected = (st.selected + 1) % n;
            moved = true;
        }
        if up && n > 0 {
            st.selected = (st.selected + n - 1) % n;
            moved = true;
        }
        if tab && matches!(st.mode, PaletteMode::Everywhere | PaletteMode::Files | PaletteMode::Symbols | PaletteMode::Actions) {
            st.mode = match st.mode {
                PaletteMode::Everywhere => PaletteMode::Files,
                PaletteMode::Files => PaletteMode::Symbols,
                PaletteMode::Symbols => PaletteMode::Actions,
                _ => PaletteMode::Everywhere,
            };
        }
        if enter && let Some(it) = st.items.get(st.selected) {
            action = PaletteAction::Run(it.clone());
        }
        ctx.input_mut(|i| {
            for k in [Key::ArrowDown, Key::ArrowUp, Key::Enter, Key::Tab] {
                i.consume_key(egui::Modifiers::NONE, k);
            }
        });
        let mut outside_rect = Rect::NOTHING;
        egui::Area::new(egui::Id::new("palette")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, Vec2::new(0.0, 80.0)).show(ctx, |ui| {
            popup_frame(&pal).show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    if matches!(st.mode, PaletteMode::Everywhere | PaletteMode::Files | PaletteMode::Symbols | PaletteMode::Actions) {
                        for (m, label) in [(PaletteMode::Everywhere, "All"), (PaletteMode::Files, "Files"), (PaletteMode::Symbols, "Symbols"), (PaletteMode::Actions, "Actions")] {
                            let active = st.mode == m;
                            let g = ui.painter().layout_no_wrap(label.to_string(), fonts::ui(13.5), pal.text);
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, 28.0), Sense::click());
                            if active {
                                ui.painter().rect_filled(r, CornerRadius::same(8), pal.selected);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, CornerRadius::same(8), pal.hover);
                            }
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, label, if active { fonts::semibold(13.5) } else { fonts::ui(13.5) }, if active { pal.text } else { pal.text_dim });
                            if resp.clicked() {
                                st.mode = m;
                            }
                        }
                    } else {
                        ui.label(RichText::new(st.mode.title()).font(fonts::semibold(15.0)));
                    }
                });
                ui.add_space(10.0);
                let hint = match st.mode {
                    PaletteMode::Everywhere => "Search everywhere: files, symbols, actions",
                    PaletteMode::Actions => "Type an action name",
                    PaletteMode::Files => "Type a file name",
                    PaletteMode::Symbols => "Type a symbol name",
                    PaletteMode::Recent => "Filter recent files",
                    PaletteMode::Structure => "Filter definitions",
                    PaletteMode::Choose => "Filter",
                };
                let resp = widgets::input_frame(ui, &pal, Some(Icon::Search), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut st.query).id(egui::Id::new("palette_query")).hint_text(hint).font(fonts::ui(15.0)).desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6)))
                });
                if st.just_opened {
                    resp.request_focus();
                    st.just_opened = false;
                }
                ui.add_space(8.0);
                let row_h = 38.0;
                let list_h = (n as f32 * row_h + 50.0).clamp(70.0, 420.0);
                if n == 0 {
                    ui.add_space(10.0);
                    ui.label(RichText::new(if st.query.is_empty() { "Start typing..." } else { "Nothing found" }).color(pal.text_dim));
                    ui.add_space(10.0);
                }
                egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                    let mut last_section = "";
                    for (i, it) in st.items.iter().enumerate() {
                        if !it.section.is_empty() && it.section != last_section {
                            last_section = it.section;
                            ui.add_space(6.0);
                            widgets::caption(ui, &pal, it.section);
                            ui.add_space(2.0);
                        }
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                        if i == st.selected {
                            ui.painter().rect_filled(r, CornerRadius::same(9), pal.selected);
                            if moved {
                                resp.scroll_to_me(None);
                            }
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(9), pal.hover);
                        }
                        let bc = badge_color(self, &it.badge);
                        let br = Rect::from_min_size(Pos2::new(r.min.x + 10.0, r.center().y - 12.0), Vec2::splat(24.0));
                        ui.painter().rect_filled(br, CornerRadius::same(7), bc.gamma_multiply(0.22));
                        ui.painter().text(br.center(), Align2::CENTER_CENTER, &it.badge, fonts::semibold(12.0), bc);
                        let job = highlighted(&it.label, &it.hits, fonts::ui(14.0), pal.text, pal.accent);
                        let g = ui.painter().layout_job(job);
                        let gw = g.size().x;
                        ui.painter().galley(Pos2::new(r.min.x + 44.0, r.center().y - g.size().y / 2.0), g, pal.text);
                        if !it.detail.is_empty() {
                            ui.painter().text(Pos2::new(r.min.x + 56.0 + gw, r.center().y), Align2::LEFT_CENTER, &it.detail, fonts::ui(12.0), pal.text_dim);
                        }
                        if !it.shortcut.is_empty() {
                            ui.painter().text(Pos2::new(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, &it.shortcut, fonts::ui(12.0), pal.text_dim);
                        }
                        if resp.clicked() {
                            action = PaletteAction::Run(it.clone());
                        }
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (k, t) in [("Up Down", "navigate"), ("Enter", "select"), ("Tab", "switch"), ("Esc", "close")] {
                        widgets::chip(ui, &pal, k);
                        ui.label(RichText::new(t).color(pal.text_dim).size(11.5));
                        ui.add_space(8.0);
                    }
                });
                outside_rect = ui.min_rect().expand(16.0);
            });
        });
        let clicked_outside = ctx.input(|i| i.pointer.primary_pressed() && i.pointer.interact_pos().is_some_and(|p| !outside_rect.contains(p)));
        if clicked_outside {
            action = PaletteAction::Close;
        }
        match action {
            PaletteAction::None => self.popup = Popup::Palette(st),
            PaletteAction::Close => {
                self.popup = Popup::None;
                self.editor_focus = true;
            }
            PaletteAction::Run(it) => {
                self.popup = Popup::None;
                self.editor_focus = true;
                match it.target {
                    PTarget::Command(id) => self.run_command(ctx, &id),
                    PTarget::File(p) => {
                        self.push_nav();
                        self.open_file(&p);
                    }
                    PTarget::Symbol { path, offset } => self.goto(&path, offset, true),
                    PTarget::Offset(o) => {
                        self.push_nav();
                        if let Some(d) = self.doc_mut() {
                            d.ed.set_caret(o, false);
                            d.view.scroll_to_caret = true;
                        }
                    }
                    PTarget::Branch(b) => self.git_checkout(&b),
                }
            }
        }
    }

    // ------------------------------------------------------------------ dialogs

    /// A modal dialog card. `build` returns true when the user confirmed.
    fn dialog_card(&self, ctx: &egui::Context, id: &str, title: &str, width: f32, build: impl FnOnce(&mut egui::Ui)) -> Rect {
        let pal = self.pal;
        let mut rect = Rect::NOTHING;
        egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, Vec2::new(0.0, 130.0)).show(ctx, |ui| {
            popup_frame(&pal).show(ui, |ui| {
                ui.set_width(width);
                ui.label(RichText::new(title).font(fonts::semibold(16.0)));
                ui.add_space(12.0);
                build(ui);
                rect = ui.min_rect().expand(16.0);
            });
        });
        rect
    }

    /// OK / Cancel row; returns (ok, cancel).
    fn ok_cancel(&self, ui: &mut egui::Ui, ok_label: &str) -> (bool, bool) {
        let pal = self.pal;
        let mut res = (false, false);
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::primary_button(ui, &pal, ok_label).clicked() {
                    res.0 = true;
                }
                if widgets::secondary_button(ui, &pal, "Cancel").clicked() {
                    res.1 = true;
                }
            });
        });
        res
    }

    fn text_field(&self, ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
        let pal = self.pal;
        widgets::input_frame(ui, &pal, None, |ui| ui.add(egui::TextEdit::singleline(text).hint_text(hint).desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6))))
    }

    fn draw_input_dialogs(&mut self, ctx: &egui::Context) {
        let (esc, enter) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
        let popup = std::mem::replace(&mut self.popup, Popup::None);
        match popup {
            Popup::GotoLine { mut input, mut just_opened } => {
                let (mut ok, mut cancel) = (enter, esc);
                self.dialog_card(ctx, "goto_line", "Go to Line:Column", 420.0, |ui| {
                    let r = self.text_field(ui, &mut input, "line[:column]");
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    let (o, c) = self.ok_cancel(ui, "Go");
                    ok |= o;
                    cancel |= c;
                });
                if ok && !cancel {
                    let mut parts = input.trim().split(':');
                    let line: usize = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(1);
                    let col: usize = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(1);
                    self.push_nav();
                    if let Some(d) = self.doc_mut() {
                        let l = line.saturating_sub(1).min(d.ed.line_count() - 1);
                        let o = d.ed.offset_of(l, col.saturating_sub(1));
                        d.ed.set_caret(o, false);
                        d.view.scroll_to_caret = true;
                    }
                    self.editor_focus = true;
                } else if cancel {
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::GotoLine { input, just_opened };
                }
            }
            Popup::Rename { old, mut input, mut just_opened } => {
                let (mut ok, mut cancel) = (enter, esc);
                self.dialog_card(ctx, "rename", &format!("Rename '{old}'"), 440.0, |ui| {
                    let r = self.text_field(ui, &mut input, "new name");
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("All code occurrences in the project and open files are renamed.").color(self.pal.text_dim).size(12.0));
                    let (o, c) = self.ok_cancel(ui, "Rename");
                    ok |= o;
                    cancel |= c;
                });
                if cancel {
                    self.editor_focus = true;
                } else if ok {
                    self.rename_symbol(&old, input.trim());
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::Rename { old, input, just_opened };
                }
            }
            Popup::NewFile { dir, mut input, is_dir, mut just_opened } => {
                let (mut ok, mut cancel) = (enter, esc);
                let rel = self.project_root().map(|r| ide_core::workspace::relative(r, &dir)).unwrap_or_default();
                self.dialog_card(ctx, "new_file", if is_dir { "New Directory" } else { "New File" }, 440.0, |ui| {
                    let r = self.text_field(ui, &mut input, if is_dir { "directory name" } else { "file name, e.g. utils.lisp" });
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new(format!("in {}", if rel.is_empty() { "." } else { &rel })).color(self.pal.text_dim).size(12.0));
                    let (o, c) = self.ok_cancel(ui, "Create");
                    ok |= o;
                    cancel |= c;
                });
                if cancel {
                    self.editor_focus = true;
                } else if ok && !input.trim().is_empty() {
                    let mut name = input.trim().to_string();
                    if !is_dir && !name.contains('.') {
                        name.push_str(".lisp");
                    }
                    let path = dir.join(&name);
                    let res = if is_dir { ide_core::workspace::create_dir(&path) } else { ide_core::workspace::create_file(&path, "") };
                    match res {
                        Ok(()) => {
                            self.refresh_project();
                            if let Some(p) = self.project.as_mut() {
                                p.expanded.insert(dir.clone());
                            }
                            if !is_dir {
                                self.open_file(&path);
                            }
                        }
                        Err(e) => self.toast(format!("Cannot create {name}: {e}"), Sev::Error),
                    }
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::NewFile { dir, input, is_dir, just_opened };
                }
            }
            Popup::RenameFile { path, mut input, mut just_opened } => {
                let (mut ok, mut cancel) = (enter, esc);
                self.dialog_card(ctx, "rename_file", "Rename", 440.0, |ui| {
                    let r = self.text_field(ui, &mut input, "new name");
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    let (o, c) = self.ok_cancel(ui, "Rename");
                    ok |= o;
                    cancel |= c;
                });
                if cancel {
                } else if ok && !input.trim().is_empty() {
                    let to = path.with_file_name(input.trim());
                    match ide_core::workspace::rename(&path, &to) {
                        Ok(()) => {
                            for d in self.docs.iter_mut() {
                                if d.path.as_deref() == Some(path.as_path()) {
                                    d.path = Some(to.clone());
                                }
                            }
                            self.refresh_project();
                        }
                        Err(e) => self.toast(format!("Cannot rename: {e}"), Sev::Error),
                    }
                } else {
                    self.popup = Popup::RenameFile { path, input, just_opened };
                }
            }
            Popup::LocalCommit { mut input, mut just_opened } => {
                let (mut ok, mut cancel) = (enter, esc);
                self.dialog_card(ctx, "local_commit", "Local Commit", 480.0, |ui| {
                    let r = self.text_field(ui, &mut input, "Message (optional)");
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Saves a snapshot of your changes outside of git (in .airlisp/, invisible to git).\nDiffs and change markers start from it afterwards.").color(self.pal.text_dim).size(12.0));
                    let (o, c) = self.ok_cancel(ui, "Create");
                    ok |= o;
                    cancel |= c;
                });
                if cancel {
                    self.editor_focus = true;
                } else if ok {
                    self.local_commit(&input);
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::LocalCommit { input, just_opened };
                }
            }
            Popup::NewBranch { mut input, mut just_opened } => {
                let (mut ok, mut cancel) = (enter, esc);
                self.dialog_card(ctx, "new_branch", "New Branch", 420.0, |ui| {
                    let r = self.text_field(ui, &mut input, "branch name");
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Creates the branch from the current HEAD and switches to it.").color(self.pal.text_dim).size(12.0));
                    let (o, c) = self.ok_cancel(ui, "Create");
                    ok |= o;
                    cancel |= c;
                });
                if cancel {
                    self.editor_focus = true;
                } else if ok && !input.trim().is_empty() {
                    let name = input.trim().replace(' ', "-");
                    if self.git_run_sync("New branch", move |r| r.create_branch(&name).map(|_| String::new())) {
                        self.toast("Branch created", Sev::Weak);
                    }
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::NewBranch { input, just_opened };
                }
            }
            Popup::NewProject { mut parent, mut name, mut just_opened } => {
                let (mut ok, mut cancel) = (enter, esc);
                self.dialog_card(ctx, "new_project", "New Common Lisp Project", 520.0, |ui| {
                    widgets::caption(ui, &self.pal, "Name");
                    ui.add_space(4.0);
                    let r = self.text_field(ui, &mut name, "project name");
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(10.0);
                    widgets::caption(ui, &self.pal, "Location");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let w = ui.available_width() - 110.0;
                        let pal = self.pal;
                        widgets::input_frame(ui, &pal, None, |ui| ui.add(egui::TextEdit::singleline(&mut parent).desired_width(w - 30.0).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6))));
                        if widgets::secondary_button(ui, &pal, "Browse...").clicked()
                            && let Some(d) = rfd::FileDialog::new().pick_folder()
                        {
                            parent = d.to_string_lossy().into_owned();
                        }
                    });
                    ui.add_space(8.0);
                    ui.label(RichText::new("Creates an ASDF system with src/, test/ and a README.").color(self.pal.text_dim).size(12.0));
                    let (o, c) = self.ok_cancel(ui, "Create");
                    ok |= o;
                    cancel |= c;
                });
                if cancel {
                } else if ok {
                    match ide_core::workspace::create_lisp_project(std::path::Path::new(&parent), &name) {
                        Ok(root) => {
                            self.open_project(&root);
                            let main = root.join("src").join("main.lisp");
                            self.open_file(&main);
                        }
                        Err(e) => {
                            self.toast(format!("Cannot create project: {e}"), Sev::Error);
                            self.popup = Popup::NewProject { parent, name, just_opened: false };
                        }
                    }
                } else {
                    self.popup = Popup::NewProject { parent, name, just_opened };
                }
            }
            Popup::FindInFiles { mut query, mut replace, mut case_sensitive, mut whole_word, mut just_opened, replace_mode } => {
                let (mut ok, mut cancel) = (enter, esc);
                self.dialog_card(ctx, "find_in_files", if replace_mode { "Replace in Files" } else { "Find in Files" }, 520.0, |ui| {
                    let pal = self.pal;
                    let r = widgets::input_frame(ui, &pal, Some(Icon::Search), |ui| ui.add(egui::TextEdit::singleline(&mut query).hint_text("Text to find").desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6))));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    if replace_mode {
                        ui.add_space(8.0);
                        self.text_field(ui, &mut replace, "Replace with");
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut case_sensitive, "Match case");
                        ui.add_space(12.0);
                        ui.checkbox(&mut whole_word, "Whole word");
                    });
                    let (o, c) = self.ok_cancel(ui, if replace_mode { "Replace All" } else { "Find" });
                    ok |= o;
                    cancel |= c;
                });
                if cancel {
                    self.editor_focus = true;
                } else if ok && !query.is_empty() {
                    let rep = if replace_mode { Some(replace.clone()) } else { None };
                    self.run_find_dialog(&query, rep.as_deref(), case_sensitive, whole_word);
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::FindInFiles { query, replace, case_sensitive, whole_word, just_opened, replace_mode };
                }
            }
            other => self.popup = other,
        }
    }

    // ------------------------------------------------------------------ commit dialog

    fn draw_commit_dialog(&mut self, ctx: &egui::Context) {
        let Popup::Commit { just_opened } = self.popup else { return };
        let pal = self.pal;
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let mut just_opened = just_opened;
        let ctrl_enter = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.command)));
        if ctrl_enter {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Enter));
        }
        let mut local = false;
        let base_label = self.base_label();
        let (mut commit, mut commit_push, mut cancel) = (ctrl_enter && !self.git.checked.is_empty() && !self.git.commit_msg.trim().is_empty(), false, esc);
        let changes = self.git.changes.clone();
        let branch = self.git.branch.clone().unwrap_or_default();
        let mut toggles: Vec<String> = vec![];
        let mut diff: Option<String> = None;
        egui::Area::new(egui::Id::new("commit")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            popup_frame(&pal).show(ui, |ui| {
                ui.set_width(640.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Commit").font(fonts::semibold(16.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        ui.label(RichText::new(&branch).color(pal.text_dim));
                        icons::paint(ui.painter(), r, Icon::Branch, pal.text_dim);
                    });
                });
                ui.add_space(12.0);
                widgets::caption(ui, &pal, &format!("CHANGES  {}      COMPARED WITH  {}", changes.len(), base_label.to_uppercase()));
                ui.add_space(4.0);
                egui::Frame::new().fill(pal.input_bg).corner_radius(widgets::radius()).inner_margin(Margin::symmetric(4, 4)).show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                        if changes.is_empty() {
                            ui.add_space(14.0);
                            ui.vertical_centered(|ui| ui.label(RichText::new("No changes to commit").color(pal.text_dim)));
                            ui.add_space(14.0);
                        }
                        for c in &changes {
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                            widgets::row_background(ui, &pal, r, false, resp.hovered());
                            let cb = Rect::from_min_size(Pos2::new(r.min.x + 12.0, r.center().y - 8.0), Vec2::splat(16.0));
                            self.checkbox_for(ui, cb, self.git.checked.contains(&c.path));
                            let col = self.git_color(c.kind);
                            let br = Rect::from_min_size(Pos2::new(r.min.x + 38.0, r.center().y - 9.0), Vec2::splat(18.0));
                            ui.painter().rect_filled(br, CornerRadius::same(5), col.gamma_multiply(0.2));
                            ui.painter().text(br.center(), Align2::CENTER_CENTER, c.kind.letter(), fonts::semibold(11.0), col);
                            ui.painter().text(Pos2::new(r.min.x + 66.0, r.center().y), Align2::LEFT_CENTER, &c.path, fonts::ui(13.5), col);
                            if resp.clicked() {
                                toggles.push(c.path.clone());
                            }
                            resp.context_menu(|ui| {
                                if ui.button("Show Diff").clicked() {
                                    diff = Some(c.path.clone());
                                    ui.close();
                                }
                            });
                        }
                    });
                });
                ui.add_space(12.0);
                widgets::caption(ui, &pal, "MESSAGE");
                ui.add_space(4.0);
                let mut msg = std::mem::take(&mut self.git.commit_msg);
                egui::Frame::new().fill(pal.input_bg).corner_radius(widgets::radius()).inner_margin(Margin::same(10)).show(ui, |ui| {
                    let r = ui.add(egui::TextEdit::multiline(&mut msg).hint_text("Describe your changes").desired_rows(4).desired_width(f32::INFINITY).frame(egui::Frame::NONE).font(fonts::ui(13.5)));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                });
                self.git.commit_msg = msg;
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can = !self.git.checked.is_empty() && !self.git.commit_msg.trim().is_empty() && self.git.repo.is_some();
                        ui.add_enabled_ui(can, |ui| {
                            if widgets::primary_button(ui, &pal, "Commit").clicked() {
                                commit = true;
                            }
                            if widgets::secondary_button(ui, &pal, "Commit and Push").clicked() {
                                commit_push = true;
                            }
                        });
                        if widgets::secondary_button(ui, &pal, "Cancel").clicked() {
                            cancel = true;
                        }
                        if widgets::secondary_button(ui, &pal, "Local Commit").on_hover_text("Snapshot outside of git; the diff starts from it").clicked() {
                            local = true;
                        }
                    });
                });
            });
        });
        for t in toggles {
            if !self.git.checked.remove(&t) {
                self.git.checked.insert(t);
            }
        }
        if let Some(p) = diff {
            self.git_show_diff(&p);
            return;
        }
        if local {
            let m = self.git.commit_msg.clone();
            if self.local_commit(&m) {
                self.git.commit_msg.clear();
                self.popup = Popup::None;
                self.editor_focus = true;
            } else {
                self.popup = Popup::Commit { just_opened };
            }
            return;
        }
        if commit || commit_push {
            if self.git_commit_selected() {
                self.popup = Popup::None;
                self.editor_focus = true;
                if commit_push {
                    self.git_run_async("Push", |r| r.push());
                }
            }
        } else if cancel {
            self.popup = Popup::None;
            self.editor_focus = true;
        } else {
            self.popup = Popup::Commit { just_opened };
        }
    }

    fn checkbox_for(&self, ui: &mut egui::Ui, rect: Rect, on: bool) {
        let pal = self.pal;
        if on {
            ui.painter().rect_filled(rect, CornerRadius::same(4), pal.accent);
            icons::paint(ui.painter(), rect.shrink(2.5), Icon::Check, Color32::WHITE);
        } else {
            ui.painter().rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.3, pal.text_dim), StrokeKind::Inside);
        }
    }

    // ------------------------------------------------------------------ diff viewer

    fn draw_diff(&mut self, ctx: &egui::Context) {
        let Popup::Diff { .. } = self.popup else { return };
        let pal = self.pal;
        let Popup::Diff { title, lines, mut scroll_to } = std::mem::replace(&mut self.popup, Popup::None) else { return };
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let screen = ctx.content_rect();
        let w = (screen.width() * 0.8).clamp(640.0, 1200.0);
        let h = (screen.height() * 0.78).clamp(320.0, 900.0);
        let (adds, dels) = (lines.iter().filter(|l| l.kind == LineKind::Added).count(), lines.iter().filter(|l| l.kind == LineKind::Removed).count());
        egui::Area::new(egui::Id::new("diff")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            popup_frame(&pal).show(ui, |ui| {
                ui.set_width(w);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&title).font(fonts::semibold(15.0)));
                    ui.add_space(10.0);
                    ui.label(RichText::new(format!("+{adds}")).color(pal.git_added).font(fonts::semibold(13.0)));
                    ui.label(RichText::new(format!("-{dels}")).color(pal.error).font(fonts::semibold(13.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, &pal, Icon::Close, 30.0, pal.text_dim, "Close (Esc)").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(10.0);
                let mono = FontId::monospace(13.0);
                let row_h = 21.0;
                egui::Frame::new().fill(pal.editor_bg).corner_radius(widgets::radius()).show(ui, |ui| {
                    let mut area = egui::ScrollArea::both().max_height(h).auto_shrink([false, false]);
                    if let Some(first) = scroll_to.take() {
                        area = area.vertical_scroll_offset((first.saturating_sub(3)) as f32 * row_h);
                    }
                    area.show_rows(ui, row_h, lines.len(), |ui, range| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for l in &lines[range] {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(w - 40.0), row_h), Sense::hover());
                            let (bg, mark, fg) = match l.kind {
                                LineKind::Added => (pal.git_added.gamma_multiply(0.16), "+", pal.git_added),
                                LineKind::Removed => (pal.error.gamma_multiply(0.16), "-", pal.error),
                                LineKind::Same => (Color32::TRANSPARENT, " ", pal.plain),
                            };
                            if l.kind != LineKind::Same {
                                ui.painter().rect_filled(r, CornerRadius::ZERO, bg);
                            }
                            let num = |n: Option<usize>| n.map_or("    ".to_string(), |n| format!("{n:>4}"));
                            ui.painter().text(Pos2::new(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, num(l.old_no), mono.clone(), pal.line_number);
                            ui.painter().text(Pos2::new(r.min.x + 58.0, r.center().y), Align2::LEFT_CENTER, num(l.new_no), mono.clone(), pal.line_number);
                            ui.painter().text(Pos2::new(r.min.x + 108.0, r.center().y), Align2::LEFT_CENTER, mark, mono.clone(), fg);
                            ui.painter().text(Pos2::new(r.min.x + 128.0, r.center().y), Align2::LEFT_CENTER, &l.text, mono.clone(), if l.kind == LineKind::Same { pal.plain } else { fg });
                        }
                    });
                });
            });
        });
        if close {
            self.editor_focus = true;
        } else {
            self.popup = Popup::Diff { title, lines, scroll_to };
        }
    }

    // ------------------------------------------------------------------ info / confirm / about

    fn draw_info_dialogs(&mut self, ctx: &egui::Context) {
        let pal = self.pal;
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let popup = std::mem::replace(&mut self.popup, Popup::None);
        match popup {
            Popup::Info { title, text } => {
                let mut close = esc;
                egui::Area::new(egui::Id::new("info")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                    popup_frame(&pal).show(ui, |ui| {
                        ui.set_width(620.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&title).font(fonts::semibold(15.0)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::icon_button(ui, &pal, Icon::Close, 30.0, pal.text_dim, "Close (Esc)").clicked() {
                                    close = true;
                                }
                            });
                        });
                        ui.add_space(10.0);
                        egui::Frame::new().fill(pal.editor_bg).corner_radius(widgets::radius()).inner_margin(Margin::same(14)).show(ui, |ui| {
                            egui::ScrollArea::both().max_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
                                ui.add(egui::Label::new(RichText::new(&text).monospace().size(13.0)).wrap_mode(egui::TextWrapMode::Extend));
                            });
                        });
                    });
                });
                if close {
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::Info { title, text };
                }
            }
            Popup::About => {
                let mut close = esc;
                egui::Area::new(egui::Id::new("about")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                    popup_frame(&pal).show(ui, |ui| {
                        ui.set_width(400.0);
                        ui.vertical_centered(|ui| {
                            ui.add_space(10.0);
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(96.0), Sense::hover());
                            crate::brand::paint_logo(ui.painter(), r, self.logo.as_ref(), pal.accent);
                            ui.add_space(14.0);
                            ui.label(RichText::new("AirLisp").font(fonts::semibold(24.0)));
                            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(pal.text_dim));
                            ui.add_space(12.0);
                            ui.label("An IDE for Lisp, written in Rust.");
                            ui.add_space(4.0);
                            ui.label(RichText::new("Structural editing, a live SBCL REPL, Git, refactoring\nand navigation, in the spirit of JetBrains IDEs.").color(pal.text_dim).size(12.5));
                            ui.add_space(6.0);
                            ui.label(RichText::new("Fonts: JetBrains Mono and Inter (SIL OFL 1.1)").color(pal.text_dim).size(11.5));
                            ui.add_space(16.0);
                            if widgets::primary_button(ui, &pal, "Close").clicked() {
                                close = true;
                            }
                        });
                    });
                });
                if close {
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::About;
                }
            }
            Popup::Confirm { kind, message } => {
                let mut result: Option<bool> = esc.then_some(false);
                let mut save_first = false;
                egui::Area::new(egui::Id::new("confirm")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                    popup_frame(&pal).show(ui, |ui| {
                        ui.set_width(420.0);
                        ui.label(RichText::new(&message).size(14.0));
                        ui.add_space(18.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let main_label = match &kind {
                                ConfirmKind::CloseDoc(_) => "Save",
                                ConfirmKind::Delete(_) => "Delete",
                                ConfirmKind::Rollback(_) => "Rollback",
                                ConfirmKind::RestoreLocal(_) => "Restore",
                                ConfirmKind::DeleteLocal(_) => "Delete",
                                _ => "Yes",
                            };
                            let danger = matches!(kind, ConfirmKind::Delete(_) | ConfirmKind::Rollback(_) | ConfirmKind::RestoreLocal(_) | ConfirmKind::DeleteLocal(_));
                            let galley = ui.painter().layout_no_wrap(main_label.to_string(), fonts::semibold(13.5), Color32::WHITE);
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 32.0, 34.0), Sense::click());
                            let fill = if danger { pal.error } else { pal.accent };
                            ui.painter().rect_filled(r, widgets::radius(), if resp.hovered() { fill.gamma_multiply(1.15) } else { fill });
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, main_label, fonts::semibold(13.5), Color32::WHITE);
                            if resp.clicked() {
                                result = Some(true);
                                save_first = matches!(kind, ConfirmKind::CloseDoc(_));
                            }
                            if widgets::secondary_button(ui, &pal, "Cancel").clicked() {
                                result = Some(false);
                            }
                            if matches!(kind, ConfirmKind::CloseDoc(_)) && widgets::secondary_button(ui, &pal, "Don't Save").clicked() {
                                result = Some(true);
                            }
                        });
                    });
                });
                match result {
                    None => self.popup = Popup::Confirm { kind, message },
                    Some(false) => self.editor_focus = true,
                    Some(true) => {
                        self.editor_focus = true;
                        match kind {
                            ConfirmKind::CloseDoc(i) => {
                                if save_first {
                                    self.save_doc(i);
                                    if self.docs.get(i).is_some_and(|d| d.ed.is_modified()) {
                                        return;
                                    }
                                }
                                self.close_doc(i);
                            }
                            ConfirmKind::CloseAll => {
                                self.docs.clear();
                                self.active = 0;
                            }
                            ConfirmKind::Exit => self.exit_clean(),
                            ConfirmKind::Delete(p) => match ide_core::workspace::delete(&p) {
                                Ok(()) => {
                                    self.docs.retain(|d| d.path.as_ref().is_none_or(|dp| !dp.starts_with(&p)));
                                    self.active = self.active.min(self.docs.len().saturating_sub(1));
                                    self.refresh_project();
                                }
                                Err(e) => self.toast(format!("Cannot delete: {e}"), Sev::Error),
                            },
                            ConfirmKind::Reload(i) => {
                                if let Some(d) = self.docs.get_mut(i) {
                                    let _ = d.reload();
                                }
                            }
                            ConfirmKind::Rollback(rel) => self.rollback_file(&rel),
                            ConfirmKind::RestoreLocal(id) => self.local_restore(id),
                            ConfirmKind::DeleteLocal(id) => self.local_delete(id),
                        }
                    }
                }
            }
            other => self.popup = other,
        }
    }

    // ------------------------------------------------------------------ completion & hint

    fn draw_completion(&mut self, ctx: &egui::Context) {
        if !self.completion.active || self.completion.items.is_empty() || self.popup.is_open() {
            return;
        }
        let Some(pos) = self.caret_screen else { return };
        let pal = self.pal;
        let items = self.completion.items.clone();
        let selected = self.completion.selected;
        let mut accept: Option<usize> = None;
        let screen = ctx.content_rect();
        let mut pos = pos + Vec2::new(0.0, 4.0);
        let h = (items.len().min(10) as f32) * 28.0 + 24.0;
        if pos.y + h > screen.max.y {
            pos.y = (pos.y - h - 28.0).max(0.0);
        }
        pos.x = pos.x.min(screen.max.x - 420.0);
        egui::Area::new(egui::Id::new("completion")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::new()
                .fill(pal.popup_bg)
                
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(6))
                .shadow(egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(110) })
                .show(ui, |ui| {
                    ui.set_width(380.0);
                    egui::ScrollArea::vertical().max_height(280.0).auto_shrink([false, true]).show(ui, |ui| {
                        for (i, it) in items.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                            if i == selected {
                                ui.painter().rect_filled(r, CornerRadius::same(7), pal.selected);
                                resp.scroll_to_me(None);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, CornerRadius::same(7), pal.hover);
                            }
                            let (letter, color) = match it.kind.as_str() {
                                "f" => ("f", pal.definition),
                                "m" | "o" => ("m", pal.keyword_form),
                                "v" | "c" => ("v", pal.special_var),
                                "C" => ("C", pal.builtin),
                                "k" => (":", pal.keyword_arg),
                                _ => ("s", pal.text_dim),
                            };
                            let br = Rect::from_min_size(Pos2::new(r.min.x + 6.0, r.center().y - 9.0), Vec2::splat(18.0));
                            ui.painter().rect_filled(br, CornerRadius::same(5), color.gamma_multiply(0.22));
                            ui.painter().text(br.center(), Align2::CENTER_CENTER, letter, fonts::semibold(11.0), color);
                            let plen = self.completion.prefix.chars().count();
                            let hits: Vec<usize> = (0..plen).collect();
                            let job = highlighted(&it.text, &hits, FontId::monospace(13.0), pal.text, pal.accent);
                            let g = ui.painter().layout_job(job);
                            ui.painter().galley(Pos2::new(r.min.x + 34.0, r.center().y - g.size().y / 2.0), g, pal.text);
                            if resp.clicked() {
                                accept = Some(i);
                            }
                        }
                    });
                });
        });
        if let Some(i) = accept {
            self.completion.selected = i;
            self.accept_completion();
        }
    }

    fn draw_hint(&mut self, ctx: &egui::Context) {
        let Some((text, _)) = self.hint.clone() else { return };
        let Some(pos) = self.caret_screen else { return };
        let pal = self.pal;
        let screen = ctx.content_rect();
        let mut p = Pos2::new(pos.x, pos.y - 64.0);
        p.x = p.x.min(screen.max.x - 460.0).max(8.0);
        p.y = p.y.max(8.0);
        egui::Area::new(egui::Id::new("hint")).order(egui::Order::Tooltip).fixed_pos(p).interactable(false).show(ctx, |ui| {
            egui::Frame::new().fill(pal.popup_bg).corner_radius(CornerRadius::same(9)).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                ui.label(RichText::new(text).monospace().size(12.5));
            });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    // ------------------------------------------------------------------ find bar

    pub fn draw_find_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.find.open {
            return;
        }
        self.refresh_find();
        let pal = self.pal;
        let mut step: Option<bool> = None;
        let mut close = false;
        let mut do_replace = false;
        let mut do_replace_all = false;
        egui::Frame::new().fill(pal.panel_bg).inner_margin(Margin::symmetric(16, 8)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let id = egui::Id::new("find_query");
                let r = widgets::input_frame(ui, &pal, Some(Icon::Search), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.find.query).id(id).hint_text("Find").desired_width(260.0).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 4)))
                });
                if self.find.focus {
                    r.request_focus();
                    self.find.focus = false;
                }
                if r.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                    step = Some(!ctx.input(|i| i.modifiers.shift));
                    r.request_focus();
                }
                if r.has_focus() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
                if r.clicked() || r.gained_focus() {
                    self.editor_focus = false;
                }
                let count = if self.find.query.is_empty() {
                    String::new()
                } else if self.find.matches.is_empty() {
                    "No results".to_string()
                } else {
                    format!("{} of {}", self.find.current.map_or(0, |c| c + 1), self.find.matches.len())
                };
                ui.label(RichText::new(count).color(if !self.find.query.is_empty() && self.find.matches.is_empty() { pal.error } else { pal.text_dim }).size(12.5));
                if widgets::icon_button(ui, &pal, Icon::ChevronUp, 30.0, pal.text_dim, "Previous (Shift+F3)").clicked() {
                    step = Some(false);
                }
                if widgets::icon_button(ui, &pal, Icon::ChevronDown, 30.0, pal.text_dim, "Next (F3)").clicked() {
                    step = Some(true);
                }
                if ui.selectable_label(self.find.case_sensitive, "Cc").on_hover_text("Match case").clicked() {
                    self.find.case_sensitive = !self.find.case_sensitive;
                }
                if ui.selectable_label(self.find.whole_word, "W").on_hover_text("Whole word").clicked() {
                    self.find.whole_word = !self.find.whole_word;
                }
                if ui.selectable_label(self.find.replace_mode, "Replace").clicked() {
                    self.find.replace_mode = !self.find.replace_mode;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, &pal, Icon::Close, 28.0, pal.text_dim, "Close (Esc)").clicked() {
                        close = true;
                    }
                });
            });
            if self.find.replace_mode {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let r = widgets::input_frame(ui, &pal, None, |ui| ui.add(egui::TextEdit::singleline(&mut self.find.replace).hint_text("Replace").desired_width(282.0).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 4))));
                    if r.clicked() || r.gained_focus() {
                        self.editor_focus = false;
                    }
                    if r.has_focus() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                        close = true;
                    }
                    if widgets::secondary_button(ui, &pal, "Replace").clicked() {
                        do_replace = true;
                    }
                    if widgets::secondary_button(ui, &pal, "Replace All").clicked() {
                        do_replace_all = true;
                    }
                });
            }
        });
        widgets::divider(ui, &pal, ui.min_rect().max.y, ui.min_rect().min.x, ui.min_rect().max.x);
        if let Some(fwd) = step {
            self.find_step(fwd);
        }
        if do_replace {
            self.replace_current();
        }
        if do_replace_all {
            self.replace_all();
        }
        if close {
            self.find.open = false;
            self.find.matches.clear();
            self.editor_focus = true;
        }
    }

    // ------------------------------------------------------------------ welcome

    pub fn draw_welcome(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.pal;
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, pal.editor_bg);
        let mut open: Option<PathBuf> = None;
        let center_x = rect.center().x;
        let col_w = 460.0f32.min(rect.width() - 48.0);
        let top = rect.min.y + (rect.height() * 0.14).max(28.0);
        let mut ui2 = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::new(center_x - col_w / 2.0, top), Vec2::new(col_w, rect.height()))));
        let ui = &mut ui2;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
                crate::brand::paint_logo(ui.painter(), r, self.logo.as_ref(), pal.accent);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new("AirLisp").font(fonts::semibold(26.0)));
                    ui.label(RichText::new("The IDE for Lisp").color(pal.text_dim));
                });
            });
            ui.add_space(28.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, &pal, "New Project").clicked() {
                    self.run_command(ctx, "file.new_project");
                }
                if widgets::secondary_button(ui, &pal, "Open...").clicked() {
                    self.run_command(ctx, "file.open_project");
                }
                if widgets::secondary_button(ui, &pal, "Get from VCS").clicked() {
                    self.run_command(ctx, "file.clone");
                }
            });
            ui.add_space(28.0);
            if !self.settings.recent_projects.is_empty() {
                widgets::caption(ui, &pal, "RECENT PROJECTS");
                ui.add_space(8.0);
                for p in self.settings.recent_projects.iter().take(6) {
                    let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(col_w, 46.0), Sense::click());
                    widgets::row_background(ui, &pal, r.expand2(Vec2::new(6.0, 0.0)), false, resp.hovered());
                    let av = Rect::from_min_size(Pos2::new(r.min.x + 4.0, r.center().y - 15.0), Vec2::splat(30.0));
                    ui.painter().rect_filled(av, CornerRadius::same(8), pal.accent.gamma_multiply(0.85));
                    let init: String = name.chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
                    ui.painter().text(av.center(), Align2::CENTER_CENTER, init, fonts::semibold(12.0), Color32::WHITE);
                    ui.painter().text(Pos2::new(r.min.x + 46.0, r.center().y - 8.0), Align2::LEFT_CENTER, &name, fonts::semibold(13.5), pal.text);
                    ui.painter().text(Pos2::new(r.min.x + 46.0, r.center().y + 9.0), Align2::LEFT_CENTER, p, fonts::ui(11.5), pal.text_dim);
                    if resp.clicked() {
                        open = Some(PathBuf::from(p));
                    }
                }
                ui.add_space(20.0);
            }
            widgets::caption(ui, &pal, "QUICK START");
            ui.add_space(8.0);
            for (t, k) in [("Search Everywhere", "Double Shift"), ("Go to File", "Ctrl+Shift+N"), ("Find Action", "Ctrl+Shift+A"), ("Recent Files", "Ctrl+E"), ("Run file", "Shift+F10"), ("Commit", "Ctrl+K")] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t).color(pal.text_dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| widgets::chip(ui, &pal, k));
                });
                ui.add_space(2.0);
            }
        });
        if let Some(p) = open {
            self.open_project(&p);
        }
    }
}
