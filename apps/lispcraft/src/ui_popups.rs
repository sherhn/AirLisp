//! Popups and dialogs: Search Everywhere & co, input dialogs, settings, completion, find bar, welcome.

use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use ide_core::commands;
use ide_core::settings::Theme;

use crate::app::App;
use crate::editor_view::Sev;
use crate::icons::{self, Icon};
use crate::state::*;
use crate::ui_panels::badge_color;

fn popup_frame(pal: &crate::theme::Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(pal.popup_bg)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10))
        .shadow(egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(100) })
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
        self.draw_palette(ctx);
        self.draw_input_dialogs(ctx);
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
        let width = (screen.width() * 0.5).clamp(520.0, 760.0);
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
        egui::Area::new(egui::Id::new("palette")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, Vec2::new(0.0, 70.0)).show(ctx, |ui| {
            popup_frame(&pal).show(ui, |ui| {
                ui.set_width(width);
                // title / tabs
                ui.horizontal(|ui| {
                    let tabs = [(PaletteMode::Everywhere, "All"), (PaletteMode::Files, "Files"), (PaletteMode::Symbols, "Symbols"), (PaletteMode::Actions, "Actions")];
                    if matches!(st.mode, PaletteMode::Everywhere | PaletteMode::Files | PaletteMode::Symbols | PaletteMode::Actions) {
                        for (m, label) in tabs {
                            let active = st.mode == m;
                            let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(13.0), pal.text);
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 16.0, 24.0), Sense::click());
                            if active {
                                ui.painter().rect_filled(r, CornerRadius::same(6), pal.selected);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, CornerRadius::same(6), pal.hover);
                            }
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(13.0), if active { pal.text } else { pal.text_dim });
                            if resp.clicked() {
                                st.mode = m;
                            }
                        }
                    } else {
                        ui.label(RichText::new(st.mode.title()).strong());
                    }
                });
                ui.add_space(6.0);
                let te = egui::TextEdit::singleline(&mut st.query)
                    .id(egui::Id::new("palette_query"))
                    .hint_text(match st.mode {
                        PaletteMode::Everywhere => "Search everywhere: files, symbols, actions",
                        PaletteMode::Actions => "Type an action name",
                        PaletteMode::Files => "Type a file name",
                        PaletteMode::Symbols => "Type a symbol name",
                        PaletteMode::Recent => "Filter recent files",
                        PaletteMode::Structure => "Filter definitions",
                        PaletteMode::Choose => "Filter",
                    })
                    .font(FontId::proportional(15.0))
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(8, 6));
                let resp = ui.add(te);
                if st.just_opened {
                    resp.request_focus();
                    st.just_opened = false;
                }
                ui.add_space(6.0);
                let row_h = 30.0;
                let list_h = (n as f32 * row_h + 40.0).clamp(60.0, 380.0);
                if n == 0 {
                    ui.add_space(8.0);
                    ui.label(RichText::new(if st.query.is_empty() { "Start typing..." } else { "Nothing found" }).color(pal.text_dim));
                    ui.add_space(8.0);
                }
                egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                    let mut last_section = "";
                    for (i, it) in st.items.iter().enumerate() {
                        if !it.section.is_empty() && it.section != last_section {
                            last_section = it.section;
                            ui.add_space(4.0);
                            ui.label(RichText::new(it.section).color(pal.text_dim).size(11.5));
                        }
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                        if i == st.selected {
                            ui.painter().rect_filled(r, CornerRadius::same(6), pal.selected);
                            if moved {
                                resp.scroll_to_me(None);
                            }
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(6), pal.hover);
                        }
                        let bc = badge_color(self, &it.badge);
                        let br = Rect::from_min_size(Pos2::new(r.min.x + 6.0, r.center().y - 9.0), Vec2::splat(18.0));
                        ui.painter().rect_filled(br, CornerRadius::same(4), bc.gamma_multiply(0.25));
                        ui.painter().text(br.center(), Align2::CENTER_CENTER, &it.badge, FontId::proportional(11.0), bc);
                        let job = highlighted(&it.label, &it.hits, FontId::proportional(14.0), pal.text, pal.accent);
                        let g = ui.painter().layout_job(job);
                        let gw = g.size().x;
                        ui.painter().galley(Pos2::new(r.min.x + 32.0, r.center().y - g.size().y / 2.0), g, pal.text);
                        if !it.detail.is_empty() {
                            ui.painter().text(Pos2::new(r.min.x + 42.0 + gw, r.center().y), Align2::LEFT_CENTER, &it.detail, FontId::proportional(12.0), pal.text_dim);
                        }
                        if !it.shortcut.is_empty() {
                            ui.painter().text(Pos2::new(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, &it.shortcut, FontId::proportional(12.0), pal.text_dim);
                        }
                        if resp.clicked() {
                            action = PaletteAction::Run(it.clone());
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Up/Down to navigate   Enter to select   Tab to switch   Esc to close").color(pal.text_dim).size(11.0));
                outside_rect = ui.min_rect().expand(10.0);
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
                }
            }
        }
    }

    // ------------------------------------------------------------------ small input dialogs

    fn input_window(&self, ctx: &egui::Context, id: &str, title: &str, build: impl FnOnce(&mut egui::Ui)) -> Rect {
        let pal = self.pal;
        let mut rect = Rect::NOTHING;
        egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, Vec2::new(0.0, 120.0)).show(ctx, |ui| {
            popup_frame(&pal).show(ui, |ui| {
                ui.set_width(420.0);
                ui.label(RichText::new(title).strong().size(14.0));
                ui.add_space(8.0);
                build(ui);
                rect = ui.min_rect().expand(10.0);
            });
        });
        rect
    }

    fn draw_input_dialogs(&mut self, ctx: &egui::Context) {
        let (esc, enter) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
        let popup = std::mem::replace(&mut self.popup, Popup::None);
        match popup {
            Popup::GotoLine { mut input, mut just_opened } => {
                let mut done = false;
                let mut accept = false;
                self.input_window(ctx, "goto_line", "Go to Line:Column", |ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut input).hint_text("line[:column]").desired_width(f32::INFINITY));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    if enter {
                        accept = true;
                    }
                });
                if esc {
                    done = true;
                }
                if accept {
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
                    done = true;
                }
                if done {
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::GotoLine { input, just_opened };
                }
            }
            Popup::Rename { old, mut input, mut just_opened } => {
                let mut accept = false;
                self.input_window(ctx, "rename", &format!("Rename '{old}' to"), |ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut input).desired_width(f32::INFINITY));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new("All code occurrences in the project (and open files) are renamed.").size(12.0));
                    if enter {
                        accept = true;
                    }
                });
                if esc {
                    self.editor_focus = true;
                } else if accept {
                    self.rename_symbol(&old, input.trim());
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::Rename { old, input, just_opened };
                }
            }
            Popup::NewFile { dir, mut input, is_dir, mut just_opened } => {
                let mut accept = false;
                let rel = self.project_root().map(|r| ide_core::workspace::relative(r, &dir)).unwrap_or_default();
                self.input_window(ctx, "new_file", if is_dir { "New Directory" } else { "New File" }, |ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut input).hint_text(if is_dir { "directory name" } else { "file name, e.g. utils.lisp" }).desired_width(f32::INFINITY));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("in {}", if rel.is_empty() { "." } else { &rel })).color(self.pal.text_dim).size(12.0));
                    if enter {
                        accept = true;
                    }
                });
                if esc {
                    self.editor_focus = true;
                } else if accept && !input.trim().is_empty() {
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
                let mut accept = false;
                self.input_window(ctx, "rename_file", "Rename", |ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut input).desired_width(f32::INFINITY));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    if enter {
                        accept = true;
                    }
                });
                if esc {
                } else if accept && !input.trim().is_empty() {
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
            Popup::NewProject { mut parent, mut name, mut just_opened } => {
                let mut accept = false;
                self.input_window(ctx, "new_project", "New Common Lisp Project", |ui| {
                    ui.label("Name");
                    let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    ui.add_space(6.0);
                    ui.label("Location");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut parent).desired_width(300.0));
                        if ui.button("Browse...").clicked()
                            && let Some(d) = rfd::FileDialog::new().pick_folder()
                        {
                            parent = d.to_string_lossy().into_owned();
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(RichText::new("Creates an ASDF system with src/, test/ and a README.").color(self.pal.text_dim).size(12.0));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Create").color(Color32::WHITE)).fill(self.pal.accent)).clicked() || enter {
                            accept = true;
                        }
                    });
                });
                if esc {
                } else if accept {
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
                let mut accept = false;
                let mut do_replace = false;
                self.input_window(ctx, "find_in_files", if replace_mode { "Replace in Files" } else { "Find in Files" }, |ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Text to find").desired_width(f32::INFINITY));
                    if just_opened {
                        r.request_focus();
                        just_opened = false;
                    }
                    if replace_mode {
                        ui.add_space(4.0);
                        ui.add(egui::TextEdit::singleline(&mut replace).hint_text("Replace with").desired_width(f32::INFINITY));
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut case_sensitive, "Match case");
                        ui.checkbox(&mut whole_word, "Whole word");
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let label = if replace_mode { "Replace All" } else { "Find" };
                        if ui.add(egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(self.pal.accent)).clicked() || enter {
                            accept = true;
                            do_replace = replace_mode;
                        }
                    });
                });
                if esc {
                    self.editor_focus = true;
                } else if accept && !query.is_empty() {
                    let rep = if do_replace { Some(replace.clone()) } else { None };
                    self.run_find_dialog(&query, rep.as_deref(), case_sensitive, whole_word);
                    self.editor_focus = true;
                } else {
                    self.popup = Popup::FindInFiles { query, replace, case_sensitive, whole_word, just_opened, replace_mode };
                }
            }
            other => self.popup = other,
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
                        ui.set_width(560.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&title).strong().size(14.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if icons::button(ui, Icon::Close, 22.0, pal.text_dim, pal.hover, "").clicked() {
                                    close = true;
                                }
                            });
                        });
                        ui.add_space(6.0);
                        egui::ScrollArea::both().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(&text).monospace().size(13.0)).wrap_mode(egui::TextWrapMode::Extend));
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
                        ui.set_width(380.0);
                        ui.vertical_centered(|ui| {
                            ui.add_space(8.0);
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                            ui.painter().rect_filled(r, CornerRadius::same(14), pal.accent);
                            icons::paint(ui.painter(), r.shrink(14.0), Icon::Lisp, Color32::WHITE);
                            ui.add_space(10.0);
                            ui.label(RichText::new("Lispcraft").size(22.0).strong());
                            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(pal.text_dim));
                            ui.add_space(8.0);
                            ui.label("An IDE for Lisp, written in Rust.");
                            ui.label(RichText::new("Syntax-aware editing, structural editing, a live SBCL REPL,\nrefactoring and navigation - in the spirit of JetBrains IDEs.").color(pal.text_dim).size(12.0));
                            ui.add_space(12.0);
                            if ui.button("Close").clicked() {
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
                        ui.set_width(380.0);
                        ui.label(RichText::new(&message).size(14.0));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let main_label = match &kind {
                                ConfirmKind::CloseDoc(_) => "Save",
                                ConfirmKind::Delete(_) => "Delete",
                                _ => "Yes",
                            };
                            if ui.add(egui::Button::new(RichText::new(main_label).color(Color32::WHITE)).fill(if matches!(kind, ConfirmKind::Delete(_)) { pal.error } else { pal.accent })).clicked() {
                                result = Some(true);
                                save_first = matches!(kind, ConfirmKind::CloseDoc(_));
                            }
                            if matches!(kind, ConfirmKind::CloseDoc(_)) && ui.button("Don't Save").clicked() {
                                result = Some(true);
                            }
                            if ui.button("Cancel").clicked() {
                                result = Some(false);
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
                        }
                    }
                }
            }
            other => self.popup = other,
        }
    }

    // ------------------------------------------------------------------ settings

    fn draw_settings(&mut self, ctx: &egui::Context) {
        let Popup::Settings { .. } = self.popup else { return };
        let pal = self.pal;
        let Popup::Settings { mut lisp_path, mut detected, mut filter } = std::mem::replace(&mut self.popup, Popup::None) else { return };
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let mut changed_theme = false;
        let mut redetect = false;
        egui::Area::new(egui::Id::new("settings")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            popup_frame(&pal).show(ui, |ui| {
                ui.set_width(680.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Settings").strong().size(16.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icons::button(ui, Icon::Close, 24.0, pal.text_dim, pal.hover, "").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().max_height(460.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.label(RichText::new("Appearance").strong());
                    ui.horizontal(|ui| {
                        ui.label("Theme");
                        let before = self.settings.theme;
                        egui::ComboBox::from_id_salt("theme").selected_text(match self.settings.theme { Theme::Dark => "Dark", Theme::Light => "Light" }).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.settings.theme, Theme::Dark, "Dark");
                            ui.selectable_value(&mut self.settings.theme, Theme::Light, "Light");
                        });
                        changed_theme = before != self.settings.theme;
                    });
                    ui.horizontal(|ui| {
                        ui.label("Editor font size");
                        ui.add(egui::Slider::new(&mut self.settings.font_size, ide_core::settings::MIN_FONT..=ide_core::settings::MAX_FONT).step_by(1.0));
                    });
                    ui.add_space(6.0);
                    ui.label(RichText::new("Editor").strong());
                    ui.checkbox(&mut self.settings.show_line_numbers, "Show line numbers");
                    ui.checkbox(&mut self.settings.show_whitespace, "Show whitespace");
                    ui.checkbox(&mut self.settings.rainbow_brackets, "Rainbow brackets");
                    ui.checkbox(&mut self.settings.auto_popup, "Show completion popup automatically");
                    ui.checkbox(&mut self.settings.auto_save_on_focus_loss, "Save files when the window loses focus");
                    ui.add_space(6.0);
                    ui.label(RichText::new("Lisp").strong());
                    ui.horizontal(|ui| {
                        ui.label("SBCL executable");
                        let r = ui.add(egui::TextEdit::singleline(&mut lisp_path).hint_text("auto-detect").desired_width(340.0));
                        if r.lost_focus() {
                            redetect = true;
                        }
                        if ui.button("Browse...").clicked()
                            && let Some(f) = rfd::FileDialog::new().pick_file()
                        {
                            lisp_path = f.to_string_lossy().into_owned();
                            redetect = true;
                        }
                    });
                    let status = if detected.is_empty() { "No Lisp implementation detected".to_string() } else { format!("Using {detected}") };
                    ui.label(RichText::new(status).color(if detected.is_empty() { pal.error } else { pal.ok }).size(12.0));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Keymap (IntelliJ default)").strong());
                    ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter actions").desired_width(f32::INFINITY));
                    ui.add_space(4.0);
                    let f = filter.to_lowercase();
                    egui::Grid::new("keymap").striped(true).num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
                        for c in commands::COMMANDS.iter().filter(|c| !c.shortcuts.is_empty() && (f.is_empty() || c.title.to_lowercase().contains(&f) || c.shortcuts.iter().any(|s| s.to_lowercase().contains(&f)))) {
                            if c.id.starts_with("caret.") || c.id.starts_with("select.") {
                                continue;
                            }
                            ui.label(c.category.title());
                            ui.label(c.title);
                            ui.label(RichText::new(c.shortcuts.iter().map(|s| commands::display_shortcut(s)).collect::<Vec<_>>().join(",  ")).monospace().color(pal.text_dim));
                            ui.end_row();
                        }
                    });
                });
            });
        });
        if changed_theme {
            self.theme_changed();
            self.pal.apply(ctx);
        }
        if redetect {
            self.settings.lisp_path = lisp_path.clone();
            self.lisp = lisp_runtime::discover::find_sbcl(&self.settings.lisp_path);
            detected = self.lisp.as_ref().map(|l| l.path.to_string_lossy().into_owned()).unwrap_or_default();
        }
        if close {
            self.settings.lisp_path = lisp_path;
            self.lisp = lisp_runtime::discover::find_sbcl(&self.settings.lisp_path);
            self.save_settings();
            self.editor_focus = true;
        } else {
            self.popup = Popup::Settings { lisp_path, detected, filter };
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
        let h = (items.len().min(10) as f32) * 24.0 + 36.0;
        if pos.y + h > screen.max.y {
            pos.y = (pos.y - h - 24.0).max(0.0);
        }
        pos.x = pos.x.min(screen.max.x - 380.0);
        egui::Area::new(egui::Id::new("completion")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::new().fill(pal.popup_bg).stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(4)).shadow(egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(90) }).show(ui, |ui| {
                ui.set_width(340.0);
                egui::ScrollArea::vertical().max_height(240.0).auto_shrink([false, true]).show(ui, |ui| {
                    for (i, it) in items.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
                        if i == selected {
                            ui.painter().rect_filled(r, CornerRadius::same(4), pal.selected);
                            resp.scroll_to_me(None);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(4), pal.hover);
                        }
                        let (letter, color) = match it.kind.as_str() {
                            "f" => ("f", pal.definition),
                            "m" | "o" => ("m", pal.keyword_form),
                            "v" | "c" => ("v", pal.special_var),
                            "C" => ("C", pal.builtin),
                            "k" => (":", pal.keyword_arg),
                            _ => ("s", pal.text_dim),
                        };
                        let br = Rect::from_min_size(Pos2::new(r.min.x + 4.0, r.center().y - 8.0), Vec2::splat(16.0));
                        ui.painter().rect_filled(br, CornerRadius::same(4), color.gamma_multiply(0.25));
                        ui.painter().text(br.center(), Align2::CENTER_CENTER, letter, FontId::proportional(10.5), color);
                        let plen = self.completion.prefix.chars().count();
                        let hits: Vec<usize> = (0..plen).collect();
                        let job = highlighted(&it.text, &hits, FontId::monospace(13.0), pal.text, pal.accent);
                        let g = ui.painter().layout_job(job);
                        ui.painter().galley(Pos2::new(r.min.x + 28.0, r.center().y - g.size().y / 2.0), g, pal.text);
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
        let mut p = Pos2::new(pos.x, pos.y - 56.0);
        p.x = p.x.min(screen.max.x - 420.0).max(8.0);
        p.y = p.y.max(8.0);
        egui::Area::new(egui::Id::new("hint")).order(egui::Order::Tooltip).fixed_pos(p).interactable(false).show(ctx, |ui| {
            egui::Frame::new().fill(pal.popup_bg).stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
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
        egui::Frame::new().fill(pal.panel_bg).stroke(Stroke::new(1.0, pal.border)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let id = egui::Id::new("find_query");
                let r = ui.add(egui::TextEdit::singleline(&mut self.find.query).id(id).hint_text("Find").desired_width(260.0));
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
                ui.label(RichText::new(count).color(if !self.find.query.is_empty() && self.find.matches.is_empty() { pal.error } else { pal.text_dim }).size(12.0));
                if icons::button(ui, Icon::ChevronUp, 24.0, pal.text_dim, pal.hover, "Previous (Shift+F3)").clicked() {
                    step = Some(false);
                }
                if icons::button(ui, Icon::ChevronDown, 24.0, pal.text_dim, pal.hover, "Next (F3)").clicked() {
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
                    if icons::button(ui, Icon::Close, 22.0, pal.text_dim, pal.hover, "Close (Esc)").clicked() {
                        close = true;
                    }
                });
            });
            if self.find.replace_mode {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.find.replace).hint_text("Replace").desired_width(260.0));
                    if r.clicked() || r.gained_focus() {
                        self.editor_focus = false;
                    }
                    if r.has_focus() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                        close = true;
                    }
                    if ui.button("Replace").clicked() {
                        do_replace = true;
                    }
                    if ui.button("Replace All").clicked() {
                        do_replace_all = true;
                    }
                });
            }
        });
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
        ui.vertical_centered(|ui| {
            ui.add_space((rect.height() * 0.16).max(24.0));
            let (r, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::same(16), pal.accent);
            icons::paint(ui.painter(), r.shrink(16.0), Icon::Lisp, Color32::WHITE);
            ui.add_space(14.0);
            ui.label(RichText::new("Lispcraft").size(28.0).strong());
            ui.label(RichText::new("The IDE for Lisp").color(pal.text_dim).size(14.0));
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 330.0) / 2.0).max(0.0));
                if ui.add(egui::Button::new(RichText::new("New Project").color(Color32::WHITE).size(14.0)).fill(pal.accent).min_size(Vec2::new(150.0, 34.0))).clicked() {
                    self.run_command(ctx, "file.new_project");
                }
                if ui.add(egui::Button::new(RichText::new("Open...").size(14.0)).min_size(Vec2::new(150.0, 34.0))).clicked() {
                    self.run_command(ctx, "file.open_project");
                }
            });
            ui.add_space(18.0);
            if !self.settings.recent_projects.is_empty() {
                ui.label(RichText::new("Recent Projects").color(pal.text_dim));
                ui.add_space(4.0);
                for p in self.settings.recent_projects.iter().take(6) {
                    let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                    if ui.add(egui::Label::new(RichText::new(format!("{name}   {p}")).color(pal.accent)).sense(Sense::click())).clicked() {
                        open = Some(PathBuf::from(p));
                    }
                }
                ui.add_space(16.0);
            }
            let tips = [("Search Everywhere", "Double Shift"), ("Go to File", "Ctrl+Shift+N"), ("Find Action", "Ctrl+Shift+A"), ("Recent Files", "Ctrl+E"), ("Run file", "Shift+F10"), ("REPL", "Alt+8")];
            for (t, k) in tips {
                ui.label(RichText::new(format!("{t}   {k}")).color(pal.text_dim).size(12.5));
            }
        });
        if let Some(p) = open {
            self.open_project(&p);
        }
        let _ = StrokeKind::Inside;
    }
}
