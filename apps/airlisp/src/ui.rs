//! The root layout and the `eframe::App` implementation.

use eframe::egui::{self, CornerRadius, Frame, Margin};

use crate::app::App;
use crate::editor_view::{self, ViewCfg};
use crate::state::*;
use crate::widgets::{self, STATUS_H, STRIP_W, TOOLBAR_H};

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.script_inject(raw_input);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.tick(&ctx);
        self.handle_input(&ctx);
        self.script_step(&ctx);
        self.handle_dropped_files(&ctx);

        let pal = self.pal;
        // window title
        let title = {
            let file = self.doc().map(|d| d.name());
            match (&self.project, file) {
                (Some(p), Some(f)) => format!("{} [{}] - {f} - AirLisp", p.name, p.root.display()),
                (Some(p), None) => format!("{} [{}] - AirLisp", p.name, p.root.display()),
                (None, Some(f)) => format!("{f} - AirLisp"),
                (None, None) => "AirLisp".to_string(),
            }
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if self.exit_now {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.has_unsaved() && !self.exit_now {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.popup = Popup::Confirm { kind: ConfirmKind::Exit, message: "You have unsaved changes. Exit without saving?".into() };
        }

        // soft gradient window background; tool windows float above it as rounded cards
        widgets::paint_base(ui.painter(), ui.max_rect(), &pal);
        let none = Frame::NONE;
        let gap = widgets::GAP;
        egui::Panel::top("titlebar").exact_size(crate::ui_titlebar::TITLEBAR_H).show_separator_line(false).frame(none).show(ui, |ui| self.draw_titlebar(ui, &ctx));
        egui::Panel::top("toolbar").exact_size(TOOLBAR_H).show_separator_line(false).frame(none).show(ui, |ui| self.draw_toolbar(ui, &ctx));
        egui::Panel::bottom("status").exact_size(STATUS_H).show_separator_line(false).frame(none).show(ui, |ui| self.draw_status_bar(ui, &ctx));
        egui::Panel::left("strip").exact_size(STRIP_W).resizable(false).show_separator_line(false).frame(none).show(ui, |ui| self.draw_tool_strip(ui, &ctx));
        if let Some(t) = self.left_tool {
            let margin = Margin { left: 0, right: gap, top: 0, bottom: gap };
            egui::Panel::left("left_tool").resizable(true).default_size(290.0).size_range(180.0..=700.0).show_separator_line(false).frame(none.inner_margin(margin)).show(ui, |ui| {
                widgets::card_frame(&pal, pal.panel_bg).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    match t {
                        LeftTool::Project => self.draw_project_panel(ui, &ctx),
                        LeftTool::Structure => self.draw_structure_panel(ui),
                    }
                });
            });
        }
        if self.bottom_tool.is_some() {
            let margin = Margin { left: 0, right: gap, top: gap, bottom: gap };
            egui::Panel::bottom("bottom_tool").resizable(true).default_size(280.0).size_range(120.0..=900.0).show_separator_line(false).frame(none.inner_margin(margin)).show(ui, |ui| {
                widgets::card_frame(&pal, pal.panel_bg).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    self.draw_bottom_panel(ui, &ctx);
                });
            });
        }
        let margin = Margin { left: 0, right: gap, top: 0, bottom: if self.bottom_tool.is_some() { 0 } else { gap } };
        egui::CentralPanel::default().frame(none.inner_margin(margin)).show(ui, |ui| {
            widgets::card_frame(&pal, pal.editor_bg).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                self.draw_central(ui, &ctx);
            });
        });

        self.tick_palette();
        self.draw_popups(&ctx);
        self.draw_toasts(&ctx);
        self.process_screenshots(&ctx);
        self.draw_resize_handles(&ctx);

        // keep animating only when needed
        let busy = self.repl.starting || !self.repl.busy.is_empty() || self.run.running || self.compile.is_some() || self.repl.session.is_some();
        if busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        } else if self.doc().is_some() && self.editor_focus {
            ctx.request_repaint_after(std::time::Duration::from_millis(250)); // caret blink
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_settings();
        if let Some(s) = self.repl.session.take() {
            s.kill();
        }
        if let Some(h) = self.run.handle.take() {
            h.kill();
        }
    }
}

impl App {
    fn draw_central(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.docs.is_empty() {
            self.draw_welcome(ui, ctx);
            return;
        }
        self.draw_tabs(ui, ctx);
        // conflict banner
        let conflict = self.doc().is_some_and(|d| d.conflict);
        if conflict {
            let pal = self.pal;
            let mut reload = false;
            let mut keep = false;
            Frame::new().fill(pal.warning.gamma_multiply(0.18)).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("This file was changed on disk, and you have unsaved edits.");
                    if ui.button("Reload from disk").clicked() {
                        reload = true;
                    }
                    if ui.button("Keep my version").clicked() {
                        keep = true;
                    }
                });
            });
            if reload && let Some(d) = self.doc_mut() {
                let _ = d.reload();
            }
            if keep && let Some(d) = self.doc_mut() {
                d.conflict = false;
            }
        }
        self.draw_breadcrumbs(ui);
        self.draw_find_bar(ui, ctx);

        let idx = self.active.min(self.docs.len() - 1);
        self.active = idx;
        self.refresh_find();
        let markers = self.docs[idx].markers();
        let find: Vec<(usize, usize)> = if self.find.open { self.find.matches.clone() } else { vec![] };
        let git_marks: Vec<ide_core::diff::Mark> = App::git_marks(&mut self.docs[idx]).to_vec();
        let editor_on = self.editor_active(ctx);
        let cfg = ViewCfg {
            pal: &self.pal,
            font_size: self.settings.font_size,
            line_numbers: self.settings.show_line_numbers,
            whitespace: self.settings.show_whitespace,
            rainbow: self.settings.rainbow_brackets,
            focused: editor_on,
            markers: &markers,
            find: &find,
            find_current: self.find.current,
            git_marks: &git_marks,
        };
        let doc = &mut self.docs[idx];
        let out = editor_view::show(ui, &mut doc.ed, &mut doc.view, &mut doc.folds, &cfg);
        if let Some(key) = out.fold_toggle.clone()
            && !doc.folds.remove(&key)
        {
            doc.folds.insert(key);
        }
        self.editor_rect = out.text_rect;
        self.caret_screen = out.caret_screen;
        if out.clicked {
            self.editor_focus = true;
            self.completion.active = false;
        }
        if let Some(off) = out.ctrl_click {
            let doc = &mut self.docs[idx];
            doc.ed.set_caret(off, false);
            self.goto_declaration(None);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for p in dropped {
            if p.is_dir() {
                self.open_project(&p);
            } else {
                self.open_file(&p);
            }
        }
    }

    fn process_screenshots(&mut self, ctx: &egui::Context) {
        let _ = (ctx, CornerRadius::ZERO);
    }
}
