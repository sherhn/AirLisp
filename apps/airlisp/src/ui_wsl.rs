//! The Linux (WSL) settings page: where Lisp runs, and a step-by-step setup - WSL itself, a Linux distribution,
//! SBCL inside it - with the progress and output of whatever is being installed.

use std::time::Duration;

use eframe::egui::{self, CornerRadius, Margin, RichText, Sense, Vec2};
use ide_core::settings::RunEnv;
use lisp_runtime::wsl::Presence;

use crate::app::App;
use crate::app_wsl::SetupTask;
use crate::editor_view::Sev;
use crate::fonts;
use crate::i18n::{self, tr};
use crate::icons::{self, Icon};
use crate::theme::Palette;
use crate::ui_settings::group;
use crate::widgets::{self, TextField};

/// Below this much free space a distribution install gets a warning.
const LOW_SPACE: u64 = 3 * 1024 * 1024 * 1024;

/// What the user asked for on the page; applied after drawing.
enum Act {
    Recheck,
    UseWindows,
    UseLinux,
    InstallWsl,
    LoadOnline,
    Choose(String),
    InstallDistro(String),
    InstallFile,
    BrowseLocation,
    Select(String),
    CheckSbcl(String),
    InstallSbcl(String),
    OpenWindow(String),
    Use(String),
    CheckTools(String),
    InstallTools(String),
    GitSetup(String),
    Cancel,
}

fn status_ok(ui: &mut egui::Ui, pal: &Palette, text: &str) {
    ui.label(RichText::new(text).color(pal.ok).font(fonts::semibold(12.5)));
    let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
    icons::paint(ui.painter(), r, Icon::Ok, pal.ok);
}

fn spinner(ui: &mut egui::Ui, pal: &Palette) {
    ui.add(egui::Spinner::new().size(18.0).color(pal.accent));
}

fn gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

impl App {
    pub fn draw_wsl_page(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        if !self.wsl.requested {
            self.wsl.requested = true;
            self.wsl_probe();
        }
        if self.wsl.busy() {
            ui.ctx().request_repaint_after(Duration::from_millis(120));
        }
        // ---------------------------------------------------------------- what is there (copied out for the closures)
        let presence = self.wsl.probe.as_ref().map(|p| p.presence.clone());
        let installed = self.wsl.wsl_installed();
        let distros = self.wsl.distros();
        let running = self.wsl.task.as_ref().map(|t| t.task.clone());
        let busy_task = running.is_some();
        let selected = self.wsl.selected.clone();
        let chosen = if self.wsl.chosen.is_empty() { lisp_runtime::wsl::RECOMMENDED_DISTRO.to_string() } else { self.wsl.chosen.clone() };
        let chosen_friendly = self.wsl.friendly(&chosen);
        let online: Vec<(String, String)> = self.wsl.online.iter().map(|o| (o.name.clone(), o.friendly.clone())).collect();
        let online_error = self.wsl.online_error.clone();
        let sbcl = self.wsl.sbcl.get(&selected).cloned();
        let checking_sbcl = self.wsl.checking_sbcl.as_deref() == Some(selected.as_str());
        let ready = self.wsl.sbcl_path(&selected).is_some();
        let wsl_on = self.settings.run_env == RunEnv::Wsl;
        let in_use = wsl_on && self.settings.wsl_distro == selected && ready;
        let free = self.wsl.free;
        let probing = self.wsl.probing();
        let mut act: Option<Act> = None;

        // ---------------------------------------------------------------- intro
        ui.label(RichText::new(tr("Work as on Linux without a virtual machine: WSL runs a real Linux next to Windows, and AirLisp starts SBCL, the REPL and the terminal inside it. Your files stay where they are.")).color(pal.text_dim).size(12.5));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if widgets::secondary_button(ui, &pal, tr("Check again")).clicked() {
                act = Some(Act::Recheck);
            }
            if probing {
                spinner(ui, &pal);
            }
        });
        ui.add_space(14.0);

        // ---------------------------------------------------------------- progress of the running / last setup step
        if busy_task || !self.wsl.log.is_empty() || self.wsl.outcome.is_some() {
            let progress = self.wsl.task.as_ref().and_then(|t| t.progress);
            let outcome = self.wsl.outcome.clone();
            let log = self.wsl.log.clone();
            group(ui, &pal, tr("Progress"), |ui, _rows| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if busy_task {
                        spinner(ui, &pal);
                        let what = match &running {
                            Some(SetupTask::InstallWsl) => tr("Installing WSL...").to_string(),
                            Some(SetupTask::InstallDistro(n)) | Some(SetupTask::ImportFile(n)) => i18n::fmt("Installing {}...", &[n]),
                            Some(SetupTask::InstallSbcl(d)) => i18n::fmt("Installing SBCL in {}...", &[d]),
                            Some(SetupTask::InstallTools(d)) => i18n::fmt("Installing build tools in {}...", &[d]),
                            Some(SetupTask::GitSetup(d)) => i18n::fmt("Setting up git in {}...", &[d]),
                            None => String::new(),
                        };
                        ui.label(RichText::new(what).font(fonts::semibold(13.0)));
                        if !matches!(running, Some(SetupTask::InstallWsl)) {
                            widgets::right_row(ui, 30.0, |ui| {
                                if widgets::secondary_button(ui, &pal, tr("Cancel")).clicked() {
                                    act = Some(Act::Cancel);
                                }
                            });
                        }
                    }
                });
                if let Some(p) = progress {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        widgets::progress_bar(ui, &pal, p / 100.0, (ui.available_width() - 70.0).max(80.0));
                        ui.label(RichText::new(format!("{p:.0}%")).color(pal.text_dim).size(12.0));
                    });
                }
                if let Some(o) = &outcome {
                    let (text, color) = match o {
                        Ok(t) => (t.as_str(), pal.ok),
                        Err(t) => (t.as_str(), pal.error),
                    };
                    ui.add_space(6.0);
                    ui.label(RichText::new(text).color(color).size(12.5));
                }
                if !log.is_empty() {
                    ui.add_space(8.0);
                    egui::Frame::new().fill(pal.editor_bg).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("wsl_log").max_height(170.0).auto_shrink([false, true]).stick_to_bottom(true).show(ui, |ui| {
                            for l in &log {
                                ui.label(RichText::new(l).font(egui::FontId::monospace(11.5)).color(pal.plain));
                            }
                        });
                    });
                }
                ui.add_space(12.0);
            });
        }

        // ---------------------------------------------------------------- mode
        group(ui, &pal, tr("Where Lisp runs"), |ui, rows| {
            let desc = format!("{}. {}", if wsl_on { i18n::fmt("In Linux: {} through WSL", &[&self.settings.wsl_distro]) } else { tr("On Windows, with SBCL for Windows").to_string() }, tr("A project can choose for itself in the status bar"));
            rows.row(ui, &pal, tr("Run Lisp, the REPL and the terminal"), &desc, 250.0, |ui| {
                if let Some(i) = widgets::segmented(ui, &pal, &[tr("Windows"), tr("Linux (WSL)")], usize::from(wsl_on)) {
                    act = Some(if i == 0 { Act::UseWindows } else { Act::UseLinux });
                }
            });
        });

        // ---------------------------------------------------------------- setup steps
        group(ui, &pal, tr("Setup"), |ui, rows| {
            // 1. WSL
            let desc1 = match &presence {
                None => tr("Checking...").to_string(),
                Some(Presence::Unsupported) => tr("This version of Windows has no WSL (Windows 10 version 2004 or newer is needed).").to_string(),
                Some(Presence::NotInstalled) => tr("Not installed. Installing needs administrator rights, and Windows may ask for a restart.").to_string(),
                Some(Presence::Installed { version: Some(v) }) => i18n::fmt("Installed, version {}", &[v]),
                Some(Presence::Installed { version: None }) => tr("Installed").to_string(),
            };
            rows.row(ui, &pal, tr("1. Windows Subsystem for Linux"), &desc1, 190.0, |ui| match &presence {
                None => spinner(ui, &pal),
                Some(Presence::Installed { .. }) => status_ok(ui, &pal, tr("Ready")),
                Some(Presence::NotInstalled) => {
                    ui.add_enabled_ui(!busy_task, |ui| {
                        if widgets::primary_button(ui, &pal, tr("Install WSL")).clicked() {
                            act = Some(Act::InstallWsl);
                        }
                    });
                }
                Some(Presence::Unsupported) => {}
            });

            // 2. distribution
            let desc2 = if !installed {
                tr("Needs WSL (step 1).").to_string()
            } else if distros.is_empty() {
                i18n::fmt("None installed yet. Recommended: {} (around 0.5 GB to download, about 1.5 GB on disk).", &[&chosen_friendly])
            } else {
                i18n::fmt("Installed: {}", &[&distros.join(", ")])
            };
            rows.row(ui, &pal, tr("2. Linux distribution"), &desc2, 250.0, |ui| {
                if !installed {
                    return;
                }
                if distros.is_empty() {
                    ui.add_enabled_ui(!busy_task, |ui| {
                        if widgets::primary_button(ui, &pal, &i18n::fmt("Install {}", &[&chosen_friendly])).clicked() {
                            act = Some(Act::InstallDistro(chosen.clone()));
                        }
                    });
                } else {
                    let label = if selected.is_empty() { tr("Choose").to_string() } else { selected.clone() };
                    let resp = widgets::Pill::new(&label).icon(Icon::Linux, pal.text_dim).chevron().bordered().height(30.0).show(ui, &pal);
                    egui::Popup::menu(&resp).width(240.0).show(|ui| {
                        for d in &distros {
                            if ui.button(d).clicked() {
                                act = Some(Act::Select(d.clone()));
                                ui.close();
                            }
                        }
                    });
                }
            });
            if installed {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(tr("Distribution to install:")).color(pal.text_dim).size(12.0));
                    let resp = widgets::Pill::new(&chosen_friendly).icon(Icon::Install, pal.text_dim).chevron().bordered().height(28.0).show(ui, &pal);
                    egui::Popup::menu(&resp).width(300.0).show(|ui| {
                        if online.is_empty() {
                            if ui.button(tr("Load the list of distributions (needs the internet)")).clicked() {
                                act = Some(Act::LoadOnline);
                            }
                            if let Some(e) = &online_error {
                                ui.label(RichText::new(crate::ui_chrome::elide(e, 120)).color(pal.error).size(11.5));
                            }
                        }
                        for (name, friendly) in &online {
                            let text = if friendly.is_empty() { name.clone() } else { format!("{friendly}   ({name})") };
                            if ui.button(text).clicked() {
                                act = Some(Act::Choose(name.clone()));
                                ui.close();
                            }
                        }
                    });
                    if !distros.is_empty() {
                        ui.add_enabled_ui(!busy_task, |ui| {
                            if widgets::secondary_button(ui, &pal, &i18n::fmt("Install {}", &[&chosen_friendly])).clicked() {
                                act = Some(Act::InstallDistro(chosen.clone()));
                            }
                        });
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Install into:")).color(pal.text_dim).size(12.0));
                    let r = TextField::new(&mut self.wsl.location).hint(tr("the default WSL folder")).icon(Icon::Folder).width((ui.available_width() - 130.0).max(160.0)).height(32.0).show(ui, &pal);
                    if r.lost_focus() {
                        act = Some(Act::Recheck);
                    }
                    if widgets::secondary_button(ui, &pal, tr("Browse...")).clicked() {
                        act = Some(Act::BrowseLocation);
                    }
                });
                if let Some((drive, bytes)) = free {
                    let low = bytes < LOW_SPACE;
                    let text = if low { i18n::fmt("Only {0} GB free on {1}: - a distribution needs about 1.5 GB plus packages. Free some space or pick a folder on another drive.", &[&gb(bytes), &drive]) } else { i18n::fmt("{0} GB free on {1}:", &[&gb(bytes), &drive]) };
                    ui.label(RichText::new(text).color(if low { pal.warning } else { pal.text_dim }).size(11.5));
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(!busy_task, |ui| {
                        if widgets::secondary_button(ui, &pal, tr("Install from a file (.wsl, .tar)...")).clicked() {
                            act = Some(Act::InstallFile);
                        }
                    });
                    ui.label(RichText::new(tr("An Ubuntu desktop .iso is an installer for a PC or a virtual machine; WSL cannot use it.")).color(pal.text_dim).size(11.5));
                });
                ui.add_space(12.0);
            }

            // 3. SBCL
            let desc3 = if selected.is_empty() {
                tr("Install a distribution first (step 2).").to_string()
            } else if checking_sbcl {
                i18n::fmt("Looking for SBCL in {}...", &[&selected])
            } else {
                match &sbcl {
                    Some(Ok(c)) if c.path.is_some() => i18n::fmt("{0} at {1}", &[&c.version.clone().unwrap_or_else(|| "SBCL".into()), &c.path.clone().unwrap_or_default()]),
                    Some(Ok(_)) => i18n::fmt("SBCL is not installed in {}.", &[&selected]),
                    Some(Err(e)) => i18n::fmt("Could not check {0}: {1}", &[&selected, &crate::ui_chrome::elide(e, 140)]),
                    None => tr("Not checked yet.").to_string(),
                }
            };
            rows.row(ui, &pal, tr("3. SBCL in Linux"), &desc3, 190.0, |ui| {
                if selected.is_empty() {
                    return;
                }
                if checking_sbcl {
                    spinner(ui, &pal);
                } else if ready {
                    status_ok(ui, &pal, tr("Ready"));
                } else if matches!(sbcl, Some(Ok(_))) {
                    ui.add_enabled_ui(!busy_task, |ui| {
                        if widgets::primary_button(ui, &pal, tr("Install SBCL")).clicked() {
                            act = Some(Act::InstallSbcl(selected.clone()));
                        }
                    });
                } else if widgets::secondary_button(ui, &pal, tr("Check again")).clicked() {
                    act = Some(Act::CheckSbcl(selected.clone()));
                }
            });

            // 4. use it
            if !selected.is_empty() {
                let desc4 = if in_use { tr("SBCL, the REPL and the terminal run in this distribution.").to_string() } else { i18n::fmt("Switch Lisp, the REPL and the terminal to {}.", &[&selected]) };
                rows.row(ui, &pal, tr("4. Run Lisp in Linux"), &desc4, 190.0, |ui| {
                    if in_use {
                        status_ok(ui, &pal, tr("In use"));
                    } else {
                        ui.add_enabled_ui(ready, |ui| {
                            if widgets::primary_button(ui, &pal, &i18n::fmt("Use {}", &[&selected])).clicked() {
                                act = Some(Act::Use(selected.clone()));
                            }
                        });
                    }
                });
                rows.row(ui, &pal, tr("Linux user (optional)"), tr("On its first start a new distribution asks for a Linux user name and password. Until then AirLisp runs Lisp as root."), 190.0, |ui| {
                    if widgets::secondary_button(ui, &pal, tr("Open Linux window")).clicked() {
                        act = Some(Act::OpenWindow(selected.clone()));
                    }
                });
            }
        });

        // ---------------------------------------------------------------- C / make projects (os-pi and the like)
        if !selected.is_empty() {
            let tools = self.wsl.tools.get(&selected).cloned();
            let checking_tools = self.wsl_checking_tools();
            if tools.is_none() && !checking_tools && installed {
                act = act.or(Some(Act::CheckTools(selected.clone())));
            }
            group(ui, &pal, tr("Linux projects: C, assembly, make"), |ui, rows| {
                let missing: Vec<String> = tools.as_ref().and_then(|r| r.as_ref().ok()).map(|v| v.iter().filter(|(_, f)| !f).map(|(t, _)| t.clone()).collect()).unwrap_or_default();
                let desc = match &tools {
                    _ if checking_tools => i18n::fmt("Looking for make, gcc, nasm, gdb, git and QEMU in {}...", &[&selected]),
                    Some(Ok(_)) if missing.is_empty() => tr("make, gcc (with 32-bit support), nasm, gdb, git and QEMU are installed.").to_string(),
                    Some(Ok(_)) => i18n::fmt("Missing: {}", &[&missing.join(", ")]),
                    Some(Err(e)) => i18n::fmt("Could not check {0}: {1}", &[&selected, &crate::ui_chrome::elide(e, 140)]),
                    None => tr("Not checked yet.").to_string(),
                };
                rows.row(ui, &pal, tr("Build tools"), &desc, 210.0, |ui| {
                    if checking_tools {
                        spinner(ui, &pal);
                    } else if matches!(&tools, Some(Ok(_))) && missing.is_empty() {
                        status_ok(ui, &pal, tr("Ready"));
                    } else {
                        ui.add_enabled_ui(!busy_task, |ui| {
                            if widgets::primary_button(ui, &pal, tr("Install build tools")).clicked() {
                                act = Some(Act::InstallTools(selected.clone()));
                            }
                        });
                    }
                });
                rows.row(ui, &pal, tr("Git in Linux"), tr("Use your git name, e-mail and GitHub logins from Windows in this distribution (needs the Linux user from its first start)."), 210.0, |ui| {
                    ui.add_enabled_ui(!busy_task, |ui| {
                        if widgets::secondary_button(ui, &pal, tr("Set up git")).clicked() {
                            act = Some(Act::GitSetup(selected.clone()));
                        }
                    });
                });
                ui.label(RichText::new(tr("A project is built in Linux when you choose it in the status bar (or open it from \\\\wsl.localhost). Its terminal, make targets and Lisp then run in the distribution; the files can stay on Windows.")).color(pal.text_dim).size(11.5));
                ui.add_space(10.0);
            });
        }

        // ---------------------------------------------------------------- advanced
        if installed {
            let mut changed = false;
            group(ui, &pal, tr("Advanced"), |ui, rows| {
                rows.row(ui, &pal, tr("SBCL in Linux"), tr("Command or absolute path inside the distribution; empty means sbcl from its PATH"), 10.0, |_| {});
                let r = TextField::new(&mut self.settings.wsl_sbcl).hint("sbcl").icon(Icon::Terminal).show(ui, &pal);
                changed = r.lost_focus();
                ui.add_space(12.0);
            });
            if changed && !selected.is_empty() {
                act = Some(Act::CheckSbcl(selected.clone()));
            }
        }

        // ---------------------------------------------------------------- apply
        match act {
            None => {}
            Some(Act::Recheck) => {
                self.wsl.sbcl.clear();
                self.wsl_probe();
            }
            Some(Act::UseWindows) => self.use_windows(),
            Some(Act::UseLinux) => {
                if ready {
                    self.wsl_use(&selected);
                } else {
                    self.toast(tr("Finish the setup below first: Linux needs a distribution with SBCL"), Sev::Weak);
                }
            }
            Some(Act::InstallWsl) => self.wsl_install_wsl(),
            Some(Act::LoadOnline) => self.wsl_list_online(),
            Some(Act::Choose(n)) => self.wsl.chosen = n,
            Some(Act::InstallDistro(n)) => self.wsl_install_distro(&n),
            Some(Act::InstallFile) => {
                let dlg = rfd::FileDialog::new().set_title(tr("Install from a file")).add_filter(tr("WSL images"), &["wsl", "tar", "gz", "xz", "tgz", "zst", "iso"]).add_filter(tr("All files"), &["*"]);
                if let Some(f) = dlg.pick_file() {
                    self.wsl_install_file(&f);
                }
            }
            Some(Act::BrowseLocation) => {
                if let Some(d) = rfd::FileDialog::new().set_title(tr("Install into")).pick_folder() {
                    self.wsl.location = d.to_string_lossy().into_owned();
                    self.wsl_refresh_free();
                }
            }
            Some(Act::Select(d)) => {
                self.wsl.selected = d.clone();
                if !self.wsl.sbcl.contains_key(&d) {
                    self.wsl_check_sbcl(&d);
                }
            }
            Some(Act::CheckSbcl(d)) => {
                self.wsl.sbcl.remove(&d);
                self.wsl_check_sbcl(&d);
            }
            Some(Act::InstallSbcl(d)) => self.wsl_install_sbcl(&d),
            Some(Act::OpenWindow(d)) => self.wsl_open_window(&d),
            Some(Act::Use(d)) => self.wsl_use(&d),
            Some(Act::Cancel) => self.wsl_cancel(),
            Some(Act::CheckTools(d)) => self.wsl_check_tools(&d),
            Some(Act::InstallTools(d)) => self.wsl_install_tools(&d),
            Some(Act::GitSetup(d)) => self.wsl_git_setup(&d),
        }
    }
}
