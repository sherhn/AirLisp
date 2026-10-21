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
use crate::i18n::{self, tr};
use crate::icons::Icon;
use crate::input::dirs_home;
use crate::state::*;
use crate::widgets::{self, TextField};

impl App {
    pub fn open_clone_dialog(&mut self) {
        let parent = if self.settings.clone_dir.trim().is_empty() { dirs_home().join("AirLispProjects").to_string_lossy().into_owned() } else { self.settings.clone_dir.clone() };
        self.popup = Popup::Clone(Box::new(CloneState { tab: CloneTab::Url, gh_selected: None, url: String::new(), dir: String::new(), dir_touched: false, parent, branch: String::new(), shallow: false, error: None, just_opened: true, run: None }));
        // a login the CLI or Git keeps for us is picked up again on its own
        if self.settings.github_autologin && !self.github.signed_in() && !self.github.signing_in {
            self.github_sign_in_saved();
        }
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
            st.error = Some(tr("Choose a directory to clone into").into());
            return;
        }
        let dest = PathBuf::from(st.parent.trim()).join(clone::default_dir_name(&url));
        // signed in to GitHub: private repositories clone too (the token reaches git for github.com only)
        let env = self.github.token.as_deref().map(|t| ide_core::github::git_auth_env(&url, t)).unwrap_or_default();
        let opts = clone::CloneOptions { shallow: st.shallow, branch: Some(st.branch.clone()).filter(|b| !b.trim().is_empty()), env };
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
        st.run = Some(CloneRun { rx, cancel, progress: 0.0, status: tr("Connecting...").into(), dest });
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
        // Enter belongs to the text field being edited: on the GitHub tab it signs in or searches, it does not clone
        let enter = enter && st.tab == CloneTab::Url;
        egui::Area::new(egui::Id::new("clone_dialog")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            let r = widgets::glass_frame(&pal).show(ui, |ui| {
                ui.set_width(700.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
                    crate::icons::paint(ui.painter(), r, Icon::Download, pal.accent);
                    ui.label(RichText::new(tr("Get from Version Control")).font(fonts::semibold(17.0)));
                });
                ui.add_space(4.0);
                ui.label(RichText::new(tr("Clone a repository from GitHub or any Git URL and open it as a project.")).color(pal.text_dim).size(12.5));
                ui.add_space(14.0);
                if !running && let Some(i) = widgets::segmented(ui, &pal, &[tr("Repository URL"), tr("GitHub")], usize::from(st.tab == CloneTab::GitHub)) {
                    st.tab = if i == 0 { CloneTab::Url } else { CloneTab::GitHub };
                    st.error = None;
                }
                ui.add_space(14.0);

                if st.tab == CloneTab::Url {
                    widgets::caption(ui, &pal, &tr("Repository URL").to_uppercase());
                    ui.add_space(6.0);
                    let id = egui::Id::new("clone_url");
                    let r = TextField::new(&mut st.url).id(id).hint(tr("https://github.com/owner/repo  or  owner/repo")).icon(Icon::Globe).enabled(!running).show(ui, &pal);
                    if r.changed() {
                        st.error = None;
                        st.gh_selected = None;
                    }
                    widgets::focus_once(ui, &r, &mut st.just_opened);
                } else if self.draw_github_tab(ui, &mut st, running) {
                    start = true;
                }
                ui.add_space(16.0);

                widgets::caption(ui, &pal, &tr("Parent directory").to_uppercase());
                ui.add_space(6.0);
                widgets::right_row(ui, widgets::FIELD_H, |ui| {
                    if !running && widgets::secondary_button(ui, &pal, tr("Browse...")).clicked()
                        && let Some(d) = rfd::FileDialog::new().set_title(tr("Clone into")).pick_folder()
                    {
                        st.parent = d.to_string_lossy().into_owned();
                        st.dir_touched = true;
                    }
                    TextField::new(&mut st.parent).icon(Icon::Folder).enabled(!running).show(ui, &pal);
                });
                if st.tab == CloneTab::GitHub && st.gh_selected.is_none() {
                    ui.add_space(8.0);
                    ui.label(RichText::new(tr("Pick a repository from the list")).color(pal.text_dim).size(12.0));
                } else if let Ok(u) = clone::normalize_url(&st.url) {
                    let dest = PathBuf::from(st.parent.trim()).join(clone::default_dir_name(&u));
                    ui.add_space(8.0);
                    ui.label(RichText::new(i18n::fmt("Will be cloned into  {}", &[&dest.display()])).color(pal.text_dim).size(12.0));
                }
                ui.add_space(16.0);

                let col_w = (ui.available_width() - 24.0) / 2.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 24.0;
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        widgets::caption(ui, &pal, &tr("Branch (optional)").to_uppercase());
                        ui.add_space(6.0);
                        TextField::new(&mut st.branch).hint(tr("default branch")).icon(Icon::Branch).enabled(!running).show(ui, &pal);
                    });
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        widgets::caption(ui, &pal, &tr("Shallow clone").to_uppercase());
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.set_min_height(widgets::FIELD_H);
                            if !running {
                                widgets::toggle(ui, &pal, &mut st.shallow);
                            }
                            ui.label(RichText::new(tr("latest commit only")).color(pal.text_dim).size(12.5));
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
                        let can = st.tab == CloneTab::Url || st.gh_selected.is_some();
                        ui.add_enabled_ui(!running && can, |ui| {
                            if widgets::primary_button(ui, &pal, tr("Clone")).clicked() {
                                start = true;
                            }
                        });
                        if widgets::secondary_button(ui, &pal, tr("Cancel")).clicked() {
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
        if start && !running && (st.tab == CloneTab::Url || st.gh_selected.is_some()) {
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
            self.toast(i18n::fmt("Cloned into {}", &[&dest.display()]), Sev::Weak);
            self.request_open_project(&dest);
            self.editor_focus = true;
            return;
        }
        self.popup = Popup::Clone(st);
    }
}
