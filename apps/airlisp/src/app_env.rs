//! Where a project is built and run: on Windows, or in Linux through WSL.
//!
//! A project on a WSL file system (`\\wsl.localhost\Ubuntu\home\...`) is always a Linux project. Any other project
//! can be switched; the choice is kept in `.airlisp/project.json`, which git never sees. A Linux project runs its
//! terminal, make targets, Lisp and REPL in the distribution, and its git runs there when it lives there.

use std::path::Path;

use eframe::egui::{self, Align, Layout, RichText, Sense, Vec2};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::editor_view::Sev;
use crate::fonts;
use crate::i18n::{self, tr};
use crate::icons::{self, Icon};
use crate::widgets;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// The distribution the project is built and run in; `None`: Windows.
    pub linux: Option<String>,
    /// The user chose (answered the question or switched by hand); until then the global setting applies.
    pub decided: bool,
}

fn config_path(root: &Path) -> std::path::PathBuf {
    root.join(".airlisp").join("project.json")
}

pub fn load_config(root: &Path) -> ProjectConfig {
    std::fs::read_to_string(config_path(root)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_config(root: &Path, cfg: &ProjectConfig) -> std::io::Result<()> {
    let p = config_path(root);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(cfg).unwrap_or_default())?;
    // like the local commits, the project settings never become a change in git
    if let Some(repo) = ide_core::git::find_repo(root) {
        ide_core::localvcs::LocalVcs::ensure_git_ignored(&repo);
    }
    Ok(())
}

/// A project that is built the Unix way: a makefile, a configure script or shell scripts at the top.
pub fn looks_like_linux_project(root: &Path) -> bool {
    if ide_core::build::find_makefile(root).is_some() || root.join("configure").is_file() || root.join("CMakeLists.txt").is_file() {
        return true;
    }
    std::fs::read_dir(root).map(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x == "sh"))).unwrap_or(false)
}

/// Distributions that are not for working in (Docker Desktop's own).
pub fn usable_distro(name: &str) -> bool {
    !name.to_ascii_lowercase().starts_with("docker-desktop")
}

impl App {
    /// Read the project's environment when it is opened.
    pub fn load_project_env(&mut self) {
        let Some(root) = self.project_root().map(Path::to_path_buf) else {
            self.project_cfg = ProjectConfig::default();
            self.env_banner = false;
            return;
        };
        self.project_cfg = load_config(&root);
        let on_linux_fs = ide_core::wsl::wsl_path(&root).is_some();
        self.env_banner = cfg!(windows) && !on_linux_fs && !self.project_cfg.decided && self.wsl_target().is_none() && looks_like_linux_project(&root);
        if self.env_banner && self.wsl.probe.is_none() && !self.wsl.probing() {
            self.wsl_probe();
        }
        self.lisp = self.resolve_lisp();
        self.refresh_make_targets();
    }

    /// The project lives on a WSL file system (and can only be a Linux project).
    pub fn project_on_linux_fs(&self) -> bool {
        self.project_root().is_some_and(|r| ide_core::wsl::wsl_path(r).is_some())
    }

    /// The distribution the project asks for: the one it lives in, else the one chosen for it. `None` also when
    /// the project has not chosen: then the global setting decides.
    pub fn project_distro(&self) -> Option<String> {
        let root = self.project_root()?;
        if let Some(w) = ide_core::wsl::wsl_path(root) {
            return Some(w.distro);
        }
        self.project_cfg.linux.clone()
    }

    /// The project chose Windows explicitly.
    pub fn project_on_windows(&self) -> bool {
        self.project.is_some() && !self.project_on_linux_fs() && self.project_cfg.decided && self.project_cfg.linux.is_none()
    }

    /// Build and run the project in `distro` (`None`: on Windows) from now on.
    pub fn set_project_env(&mut self, distro: Option<String>) {
        let Some(root) = self.project_root().map(Path::to_path_buf) else { return };
        if self.project_on_linux_fs() {
            return;
        }
        self.project_cfg = ProjectConfig { linux: distro.clone(), decided: true };
        if let Err(e) = save_config(&root, &self.project_cfg) {
            self.toast(i18n::fmt("Cannot save the project settings: {}", &[&e]), Sev::Error);
        }
        self.env_banner = false;
        self.lisp_environment_changed();
        match distro {
            Some(d) => self.toast(i18n::fmt("{0} is now built and run in Linux ({1})", &[&self.project.as_ref().map(|p| p.name.clone()).unwrap_or_default(), &d]), Sev::Weak),
            None => self.toast(i18n::fmt("{} is now built and run on Windows", &[&self.project.as_ref().map(|p| p.name.clone()).unwrap_or_default()]), Sev::Weak),
        }
    }

    /// The distribution to offer for a project: the one Lisp already uses, else the first usable one.
    pub fn suggested_distro(&self) -> Option<String> {
        let distros: Vec<String> = self.wsl.distros().into_iter().filter(|d| usable_distro(d)).collect();
        let cfg = self.settings.wsl_distro.trim();
        if !cfg.is_empty() && distros.iter().any(|d| d == cfg) {
            return Some(cfg.to_string());
        }
        self.wsl.probe.as_ref().and_then(|p| p.default_distro()).map(|d| d.name.clone()).filter(|d| usable_distro(d)).or_else(|| distros.into_iter().next())
    }

    // ------------------------------------------------------------------ UI

    /// The question shown on top of the editor when a Unix-style project is opened on Windows.
    pub fn draw_env_banner(&mut self, ui: &mut egui::Ui) {
        if !self.env_banner {
            return;
        }
        let pal = self.pal;
        let suggested = self.suggested_distro();
        let checking = self.wsl.probing();
        let mut act: Option<Option<Option<String>>> = None;
        let mut setup = false;
        egui::Frame::new().fill(pal.accent.gamma_multiply(0.12)).corner_radius(widgets::radius()).inner_margin(egui::Margin::symmetric(14, 10)).outer_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 0 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                icons::paint(ui.painter(), r, Icon::Linux, pal.accent);
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr("This project is built the Linux way (make, shell scripts)")).font(fonts::semibold(13.0)));
                    ui.label(RichText::new(tr("Build it, run it and use the terminal in Linux through WSL? Files stay where they are.")).color(pal.text_dim).size(12.0));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, &pal, Icon::Close, 26.0, pal.text_dim, tr("Not now")).clicked() {
                        act = Some(None);
                    }
                    if widgets::secondary_button(ui, &pal, tr("Keep Windows")).clicked() {
                        act = Some(Some(None));
                    }
                    match &suggested {
                        Some(d) => {
                            if widgets::primary_button(ui, &pal, &i18n::fmt("Use Linux ({})", &[d])).clicked() {
                                act = Some(Some(Some(d.clone())));
                            }
                        }
                        None if checking => {
                            ui.add(egui::Spinner::new().size(16.0).color(pal.accent));
                        }
                        None => {
                            if widgets::primary_button(ui, &pal, tr("Set up Linux...")).clicked() {
                                setup = true;
                            }
                        }
                    }
                });
            });
        });
        if setup {
            self.open_settings_page(crate::ui_settings::WSL_PAGE);
        }
        match act {
            Some(None) => self.env_banner = false,
            Some(Some(choice)) => self.set_project_env(choice),
            None => {}
        }
    }

    /// Status bar: where the project (or Lisp) runs, and a menu to switch it.
    pub fn draw_env_chip(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        let target = self.wsl_target();
        if self.project.is_none() && target.is_none() {
            return;
        }
        let (text, color) = match &target {
            Some(t) => (i18n::fmt("Linux: {}", &[&t.distro]), pal.accent),
            None => (tr("Windows").to_string(), pal.text_dim),
        };
        let resp = ui.add(egui::Label::new(RichText::new(text).color(color).font(fonts::ui(12.0))).sense(Sense::click())).on_hover_text(tr("Where this project is built and run - click to change"));
        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        icons::paint(ui.painter(), r, if target.is_some() { Icon::Linux } else { Icon::Terminal }, color);
        if resp.clicked() && self.wsl.probe.is_none() && !self.wsl.probing() {
            self.wsl_probe();
        }
        let linux_fs = self.project_on_linux_fs();
        let distros: Vec<String> = self.wsl.distros().into_iter().filter(|d| usable_distro(d)).collect();
        let current = self.project_distro();
        let has_project = self.project.is_some();
        let mut choice: Option<Option<String>> = None;
        let mut setup = false;
        egui::Popup::menu(&resp).width(260.0).show(|ui| {
            if !has_project {
                ui.label(RichText::new(tr("Open a project to choose where it runs")).color(pal.text_dim).size(12.0));
            } else if linux_fs {
                ui.label(RichText::new(tr("This project lives in Linux, so it always runs there")).color(pal.text_dim).size(12.0));
            } else {
                ui.label(RichText::new(tr("Build and run this project:")).color(pal.text_dim).size(12.0));
                if ui.selectable_label(current.is_none(), tr("On Windows")).clicked() {
                    choice = Some(None);
                    ui.close();
                }
                for d in &distros {
                    if ui.selectable_label(current.as_deref() == Some(d.as_str()), i18n::fmt("In Linux: {}", &[d])).clicked() {
                        choice = Some(Some(d.clone()));
                        ui.close();
                    }
                }
                if distros.is_empty() {
                    ui.label(RichText::new(if self.wsl.probing() { tr("Looking for Linux distributions...") } else { tr("No Linux distribution is installed yet") }).color(pal.text_dim).size(12.0));
                }
            }
            ui.separator();
            if ui.button(tr("Linux (WSL) setup...")).clicked() {
                setup = true;
                ui.close();
            }
        });
        if let Some(c) = choice {
            self.set_project_env(c);
        }
        if setup {
            self.open_settings_page(crate::ui_settings::WSL_PAGE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip_and_detection() {
        let d = std::env::temp_dir().join(format!("airlisp-env-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(load_config(&d), ProjectConfig::default());
        assert!(!looks_like_linux_project(&d));
        std::fs::write(d.join("makefile"), "all:\n").unwrap();
        assert!(looks_like_linux_project(&d));
        let cfg = ProjectConfig { linux: Some("Ubuntu-24.04".into()), decided: true };
        save_config(&d, &cfg).unwrap();
        assert_eq!(load_config(&d), cfg);
        assert!(usable_distro("Ubuntu") && !usable_distro("docker-desktop") && !usable_distro("docker-desktop-data"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
