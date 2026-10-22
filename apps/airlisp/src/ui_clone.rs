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
        let linux_distro = self.suggested_distro().unwrap_or_default();
        self.popup = Popup::Clone(Box::new(CloneState {
            tab: CloneTab::Url,
            gh_selected: None,
            url: String::new(),
            dir: String::new(),
            dir_touched: false,
            parent,
            branch: String::new(),
            shallow: false,
            error: None,
            just_opened: true,
            run: None,
            linux: false,
            linux_distro,
            linux_dir: "~/projects".into(),
            build_in_linux: false,
        }));
        // which Linux distributions there are, for "Clone into Linux"
        if cfg!(windows) && self.wsl.probe.is_none() && !self.wsl.probing() {
            self.wsl_probe();
        }
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
        let linux = st.linux && !st.linux_distro.is_empty();
        if !linux && st.parent.trim().is_empty() {
            st.error = Some(tr("Choose a directory to clone into").into());
            return;
        }
        if linux && st.linux_dir.trim().is_empty() {
            st.error = Some(tr("Choose a folder in Linux to clone into").into());
            return;
        }
        let name = clone::default_dir_name(&url);
        let dest = PathBuf::from(st.parent.trim()).join(&name);
        // signed in to GitHub: private repositories clone too (the token reaches git for github.com only)
        let env = self.github.token.as_deref().map(|t| ide_core::github::git_auth_env(&url, t)).unwrap_or_default();
        let opts = clone::CloneOptions { shallow: st.shallow, branch: Some(st.branch.clone()).filter(|b| !b.trim().is_empty()), env, lf: st.build_in_linux && !linux };
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (c2, d2) = (cancel.clone(), dest.clone());
        st.error = None;
        let (distro, linux_dir) = (st.linux_distro.clone(), st.linux_dir.trim().to_string());
        std::thread::spawn(move || {
            let tx2 = tx.clone();
            let mut dest = d2;
            if linux {
                // the folder inside the distribution: `~` is the Linux user's home; make sure the parent exists
                let home = if linux_dir.starts_with('~') {
                    let _ = tx.send(CloneMsg::Line(format!("Starting {distro}...")));
                    match lisp_runtime::wsl::home_dir(&distro) {
                        Ok(h) => h,
                        Err(e) => {
                            let _ = tx.send(CloneMsg::Done(Err(format!("{distro}: {e}"))));
                            return;
                        }
                    }
                } else {
                    String::new()
                };
                let parent = linux_parent(&linux_dir, &home);
                let mk = lisp_runtime::wsl::exec(&lisp_runtime::wsl::Target::new(&distro, ""), None, "mkdir", &["-p".to_string(), parent.clone()]);
                let _ = lisp_runtime::wsl::capture(mk, std::time::Duration::from_secs(60));
                dest = ide_core::wsl::windows_path(&distro, &format!("{}/{name}", parent.trim_end_matches('/')));
                let _ = tx.send(CloneMsg::Dest(dest.clone()));
            }
            let r = clone::clone_repo(&url, &dest, &opts, &c2, &mut |l| {
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
                    CloneMsg::Dest(d) => run.dest = d,
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

                let distros: Vec<String> = self.wsl.distros();
                if !distros.is_empty() {
                    widgets::caption(ui, &pal, &tr("Clone into").to_uppercase());
                    ui.add_space(6.0);
                    if st.linux_distro.is_empty() || !distros.contains(&st.linux_distro) {
                        st.linux_distro = self.suggested_distro().unwrap_or_else(|| distros[0].clone());
                    }
                    if !running && let Some(i) = widgets::segmented(ui, &pal, &[tr("Windows folder"), &i18n::fmt("Linux: {}", &[&st.linux_distro])], usize::from(st.linux)) {
                        st.linux = i == 1;
                    }
                    ui.add_space(12.0);
                } else {
                    st.linux = false;
                }
                if st.linux {
                    widgets::caption(ui, &pal, &tr("Folder in Linux").to_uppercase());
                    ui.add_space(6.0);
                    widgets::right_row(ui, widgets::FIELD_H, |ui| {
                        if distros.len() > 1 {
                            let resp = widgets::Pill::new(&st.linux_distro).icon(Icon::Linux, pal.text_dim).chevron().bordered().height(widgets::FIELD_H).show(ui, &pal);
                            egui::Popup::menu(&resp).width(220.0).show(|ui| {
                                for d in &distros {
                                    if ui.button(d).clicked() {
                                        st.linux_distro = d.clone();
                                        ui.close();
                                    }
                                }
                            });
                        }
                        TextField::new(&mut st.linux_dir).hint("~/projects").icon(Icon::Linux).enabled(!running).show(ui, &pal);
                    });
                } else {
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
                    if !distros.is_empty() {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if !running {
                                widgets::toggle(ui, &pal, &mut st.build_in_linux);
                            }
                            ui.label(RichText::new(i18n::fmt("Build and run it in Linux ({}): keeps Linux line endings", &[&st.linux_distro])).color(pal.text_dim).size(12.5));
                        });
                    }
                }
                if st.tab == CloneTab::GitHub && st.gh_selected.is_none() {
                    ui.add_space(8.0);
                    ui.label(RichText::new(tr("Pick a repository from the list")).color(pal.text_dim).size(12.0));
                } else if let Ok(u) = clone::normalize_url(&st.url) {
                    let target = if st.linux {
                        i18n::fmt("{0}/{1}  in {2}", &[&st.linux_dir.trim().trim_end_matches('/'), &clone::default_dir_name(&u), &st.linux_distro])
                    } else {
                        PathBuf::from(st.parent.trim()).join(clone::default_dir_name(&u)).display().to_string()
                    };
                    ui.add_space(8.0);
                    ui.label(RichText::new(i18n::fmt("Will be cloned into  {}", &[&target])).color(pal.text_dim).size(12.0));
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
            if st.build_in_linux && !st.linux && !st.linux_distro.is_empty() {
                // the project opens in Linux right away
                let cfg = crate::app_env::ProjectConfig { linux: Some(st.linux_distro.clone()), decided: true };
                if let Err(e) = crate::app_env::save_config(&dest, &cfg) {
                    self.toast(i18n::fmt("Cannot save the project settings: {}", &[&e]), Sev::Error);
                }
            }
            if !st.linux {
                self.settings.clone_dir = st.parent.trim().to_string();
            }
            self.save_settings();
            self.toast(i18n::fmt("Cloned into {}", &[&dest.display()]), Sev::Weak);
            self.request_open_project(&dest);
            self.editor_focus = true;
            return;
        }
        self.popup = Popup::Clone(st);
    }
}

/// The Linux folder `dir` with `~` replaced by `home` (`~/projects` -> `/home/me/projects`).
pub fn linux_parent(dir: &str, home: &str) -> String {
    let dir = dir.trim();
    let p = if dir == "~" {
        home.to_string()
    } else if let Some(rest) = dir.strip_prefix("~/") {
        format!("{}/{rest}", home.trim_end_matches('/'))
    } else {
        dir.to_string()
    };
    let p = p.trim_end_matches('/').to_string();
    if p.is_empty() { "/".into() } else { p }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_folders() {
        assert_eq!(linux_parent("~/projects", "/home/me"), "/home/me/projects");
        assert_eq!(linux_parent("~", "/home/me/"), "/home/me");
        assert_eq!(linux_parent("/srv/code/", ""), "/srv/code");
        assert_eq!(linux_parent("  ~/a/b/ ", "/root"), "/root/a/b");
    }
}
