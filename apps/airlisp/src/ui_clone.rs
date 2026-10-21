//! "Get from Version Control": clone a GitHub (or any git) repository into a folder and open it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use eframe::egui::{self, Align2, Key, Margin, RichText, Vec2};
use ide_core::clone;

use crate::app::App;
use crate::editor_view::Sev;
use crate::fonts;
use crate::icons::Icon;
use crate::input::dirs_home;
use crate::state::*;
use crate::widgets;

impl App {
    pub fn open_clone_dialog(&mut self) {
        let parent = if self.settings.clone_dir.trim().is_empty() { dirs_home().join("AirLispProjects").to_string_lossy().into_owned() } else { self.settings.clone_dir.clone() };
        self.popup = Popup::Clone(Box::new(CloneState { url: String::new(), dir: String::new(), dir_touched: false, parent, branch: String::new(), shallow: false, error: None, just_opened: true, run: None }));
    }

    fn start_clone(&mut self, st: &mut CloneState) {
        let url = match clone::normalize_url(&st.url) {
            Ok(u) => u,
            Err(e) => {
                st.error = Some(e);
                return;
            }
        };
        if st.parent.trim().is_empty() {
            st.error = Some("Choose a directory to clone into".into());
            return;
        }
        let dest = PathBuf::from(st.parent.trim()).join(clone::default_dir_name(&url));
        let opts = clone::CloneOptions { shallow: st.shallow, branch: Some(st.branch.clone()).filter(|b| !b.trim().is_empty()) };
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (c2, d2) = (cancel.clone(), dest.clone());
        st.error = None;
        std::thread::spawn(move || {
            let tx2 = tx.clone();
            let r = clone::clone_repo(&url, &d2, &opts, &c2, &mut |l| {
                let _ = tx2.send(CloneMsg::Line(l.to_string()));
            });
            let _ = tx.send(CloneMsg::Done(r));
        });
        st.run = Some(CloneRun { rx, cancel, progress: 0.0, status: "Connecting...".into(), dest });
    }

    pub fn draw_clone(&mut self, ctx: &egui::Context) {
        if !matches!(self.popup, Popup::Clone(_)) {
            return;
        }
        let Popup::Clone(mut st) = std::mem::replace(&mut self.popup, Popup::None) else { return };
        let pal = self.pal;
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));

        // progress from the background thread
        let mut finished: Option<Result<(), String>> = None;
        if let Some(run) = st.run.as_mut() {
            while let Ok(m) = run.rx.try_recv() {
                match m {
                    CloneMsg::Line(l) => {
                        if let Some(p) = clone::parse_progress(&l) {
                            run.progress = p / 100.0;
                        }
                        run.status = l;
                    }
                    CloneMsg::Done(r) => finished = Some(r),
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        let mut open_dest: Option<PathBuf> = None;
        if let Some(r) = finished {
            let run = st.run.take().expect("running");
            match r {
                Ok(()) => open_dest = Some(run.dest),
                Err(e) => st.error = Some(e),
            }
        }

        let mut close = esc;
        let mut start = false;
        let running = st.run.is_some();
        egui::Area::new(egui::Id::new("clone_dialog")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            let r = widgets::glass_frame(&pal).show(ui, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
                    crate::icons::paint(ui.painter(), r, Icon::Download, pal.accent);
                    ui.label(RichText::new("Get from Version Control").font(fonts::semibold(17.0)));
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Clone a repository from GitHub or any Git URL and open it as a project.").color(pal.text_dim).size(12.5));
                ui.add_space(16.0);

                widgets::caption(ui, &pal, "REPOSITORY URL");
                ui.add_space(5.0);
                let id = egui::Id::new("clone_url");
                let r = widgets::input_frame(ui, &pal, Some(Icon::Globe), |ui| {
                    ui.add_enabled(!running, egui::TextEdit::singleline(&mut st.url).id(id).hint_text("https://github.com/owner/repo  or  owner/repo").desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6)))
                });
                if r.changed() {
                    st.error = None;
                }
                if st.just_opened {
                    r.request_focus();
                    st.just_opened = false;
                }
                ui.add_space(14.0);

                widgets::caption(ui, &pal, "PARENT DIRECTORY");
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 110.0;
                    widgets::input_frame(ui, &pal, Some(Icon::Folder), |ui| {
                        ui.add_enabled(!running, egui::TextEdit::singleline(&mut st.parent).desired_width(w - 50.0).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6)));
                    });
                    if !running && widgets::secondary_button(ui, &pal, "Browse...").clicked()
                        && let Some(d) = rfd::FileDialog::new().set_title("Clone into").pick_folder()
                    {
                        st.parent = d.to_string_lossy().into_owned();
                        st.dir_touched = true;
                    }
                });
                if let Ok(u) = clone::normalize_url(&st.url) {
                    let dest = PathBuf::from(st.parent.trim()).join(clone::default_dir_name(&u));
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("Will be cloned into  {}", dest.display())).color(pal.text_dim).size(12.0));
                }
                ui.add_space(14.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        widgets::caption(ui, &pal, "BRANCH (OPTIONAL)");
                        ui.add_space(5.0);
                        widgets::input_frame(ui, &pal, Some(Icon::Branch), |ui| {
                            ui.add_enabled(!running, egui::TextEdit::singleline(&mut st.branch).hint_text("default branch").desired_width(200.0).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6)));
                        });
                    });
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        widgets::caption(ui, &pal, "SHALLOW CLONE");
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if !running {
                                widgets::toggle(ui, &pal, &mut st.shallow);
                            }
                            ui.label(RichText::new("latest commit only").color(pal.text_dim).size(12.5));
                        });
                    });
                });

                if let Some(run) = st.run.as_ref() {
                    ui.add_space(18.0);
                    widgets::progress_bar(ui, &pal, run.progress, ui.available_width());
                    ui.add_space(6.0);
                    ui.label(RichText::new(run.status.trim()).color(pal.text_dim).size(12.0));
                }
                if let Some(e) = st.error.as_ref() {
                    ui.add_space(14.0);
                    egui::Frame::new().fill(pal.error.gamma_multiply(0.16)).corner_radius(widgets::radius()).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(e).color(pal.error).size(12.5));
                    });
                }

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !running && widgets::primary_button(ui, &pal, "Clone").clicked() {
                            start = true;
                        }
                        if widgets::secondary_button(ui, &pal, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            });
            widgets::sheen(ui.painter(), r.response.rect, &pal);
        });

        if enter && !running {
            start = true;
        }
        if start && !running {
            self.start_clone(&mut st);
        }
        if close {
            if let Some(run) = st.run.as_ref() {
                run.cancel.store(true, Ordering::Relaxed);
            }
            self.editor_focus = true;
            return;
        }
        if let Some(dest) = open_dest {
            self.settings.clone_dir = st.parent.trim().to_string();
            self.save_settings();
            self.toast(format!("Cloned into {}", dest.display()), Sev::Weak);
            self.open_project(&dest);
            self.editor_focus = true;
            return;
        }
        self.popup = Popup::Clone(st);
    }
}
