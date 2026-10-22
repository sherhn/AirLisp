//! Persistent IDE settings (JSON in the platform config directory).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Theme {
    /// JetBrains "New UI" dark.
    #[default]
    #[serde(alias = "Darcula")]
    Dark,
    Light,
}

impl Theme {
    pub fn toggled(self) -> Theme {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

/// Language of the user interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    English,
    Russian,
}

/// Where Lisp, the REPL and the terminal run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RunEnv {
    #[default]
    Windows,
    /// Inside a Linux distribution of the Windows Subsystem for Linux.
    Wsl,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub language: Language,
    pub font_size: f32,
    pub show_line_numbers: bool,
    pub show_whitespace: bool,
    /// Code overview (minimap) next to the scrollbar.
    pub minimap: bool,
    pub rainbow_brackets: bool,
    /// Show the completion popup automatically while typing.
    pub auto_popup: bool,
    pub recent_projects: Vec<String>,
    pub last_project: Option<String>,
    /// Explicit path to the Lisp binary; empty = auto-detect.
    pub lisp_path: String,
    pub run_env: RunEnv,
    /// WSL distribution used when `run_env` is `Wsl`.
    pub wsl_distro: String,
    /// SBCL inside the distribution (command or absolute path); empty = `sbcl`.
    pub wsl_sbcl: String,
    /// Where Windows drives are mounted inside the distribution; empty = `/mnt/`.
    pub wsl_mount_root: String,
    /// Where modules are downloaded from (a web address or a folder); empty: the published registry.
    pub registry_url: String,
    /// The first-run welcome (choosing modules) has been shown.
    pub modules_welcome_done: bool,
    /// Default parent folder for "Get from Version Control"; empty = ~/AirProjects.
    pub clone_dir: String,
    /// The GitHub login last signed in with or looked up (prefills the GitHub tab); never a secret.
    pub github_user: String,
    /// Sign in again on its own when the GitHub tab opens (only after a login the CLI or Git keeps for us).
    pub github_autologin: bool,
    pub open_files: Vec<String>,
    pub active_file: Option<String>,
    pub show_project_panel: bool,
    pub show_bottom_panel: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::Dark,
            language: Language::English,
            font_size: 14.0,
            show_line_numbers: true,
            show_whitespace: false,
            minimap: true,
            rainbow_brackets: true,
            auto_popup: true,
            recent_projects: vec![],
            last_project: None,
            lisp_path: String::new(),
            run_env: RunEnv::Windows,
            wsl_distro: String::new(),
            wsl_sbcl: String::new(),
            wsl_mount_root: String::new(),
            registry_url: String::new(),
            modules_welcome_done: false,
            clone_dir: String::new(),
            github_user: String::new(),
            github_autologin: false,
            open_files: vec![],
            active_file: None,
            show_project_panel: true,
            show_bottom_panel: true,
        }
    }
}

pub const MIN_FONT: f32 = 8.0;
pub const MAX_FONT: f32 = 40.0;

impl Settings {
    pub fn config_dir() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("AIR_CONFIG_DIR") {
            return Some(PathBuf::from(p));
        }
        dirs::config_dir().map(|d| d.join("Air"))
    }

    pub fn load_from(path: &Path) -> Settings {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .map(|mut s| {
                s.font_size = s.font_size.clamp(MIN_FONT, MAX_FONT);
                s
            })
            .unwrap_or_default()
    }

    pub fn load() -> Settings {
        let Some(dir) = Self::config_dir() else { return Settings::default() };
        let file = dir.join("settings.json");
        // the IDE used to be called AirLisp: its settings are picked up once, and saved under the new name
        if !file.exists()
            && std::env::var_os("AIR_CONFIG_DIR").is_none()
            && let Some(old) = dirs::config_dir().map(|d| d.join("AirLisp").join("settings.json"))
            && old.exists()
        {
            return Self::load_from(&old);
        }
        Self::load_from(&file)
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        // write atomically so a crash can't leave a half-written file
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)
    }

    pub fn save(&self) -> io::Result<()> {
        match Self::config_dir() {
            Some(d) => self.save_to(&d.join("settings.json")),
            None => Ok(()),
        }
    }

    pub fn push_recent_project(&mut self, path: &str) {
        self.recent_projects.retain(|p| p != path);
        self.recent_projects.insert(0, path.to_string());
        self.recent_projects.truncate(10);
        self.last_project = Some(path.to_string());
    }

    pub fn zoom(&mut self, delta: f32) {
        self.font_size = (self.font_size + delta).clamp(MIN_FONT, MAX_FONT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn roundtrip_and_defaults() {
        let d = TempDir::new("settings");
        let f = d.path().join("cfg/settings.json");
        let mut s = Settings { theme: Theme::Light, font_size: 17.0, ..Settings::default() };
        s.push_recent_project("E:/p");
        s.save_to(&f).unwrap();
        let back = Settings::load_from(&f);
        assert_eq!(back, s);
        assert_eq!(Settings::load_from(&d.path().join("missing.json")), Settings::default());
    }

    #[test]
    fn tolerates_garbage_and_partial_files() {
        let d = TempDir::new("settings2");
        let f = d.path().join("s.json");
        fs::write(&f, "{ not json").unwrap();
        assert_eq!(Settings::load_from(&f), Settings::default());
        fs::write(&f, r#"{"theme":"Light","font_size":999}"#).unwrap();
        let s = Settings::load_from(&f);
        assert_eq!(s.theme, Theme::Light);
        assert_eq!(s.font_size, MAX_FONT, "clamped");
        assert!(s.show_line_numbers, "missing fields use defaults");
    }

    #[test]
    fn wsl_settings_default_to_windows_and_survive_a_roundtrip() {
        let d = TempDir::new("settings3");
        let f = d.path().join("s.json");
        fs::write(&f, r#"{"theme":"Dark"}"#).unwrap();
        assert_eq!(Settings::load_from(&f).run_env, RunEnv::Windows, "older files run on Windows");
        let s = Settings { run_env: RunEnv::Wsl, wsl_distro: "Ubuntu-24.04".into(), wsl_sbcl: "/usr/bin/sbcl".into(), ..Settings::default() };
        s.save_to(&f).unwrap();
        assert_eq!(Settings::load_from(&f), s);
    }

    #[test]
    fn recent_projects_dedupe_and_cap() {
        let mut s = Settings::default();
        for i in 0..15 {
            s.push_recent_project(&format!("p{i}"));
        }
        s.push_recent_project("p5");
        assert_eq!(s.recent_projects.len(), 10);
        assert_eq!(s.recent_projects[0], "p5");
        assert_eq!(s.recent_projects.iter().filter(|p| *p == "p5").count(), 1);
    }

    #[test]
    fn zoom_clamps() {
        let mut s = Settings::default();
        s.zoom(1000.0);
        assert_eq!(s.font_size, MAX_FONT);
        s.zoom(-1000.0);
        assert_eq!(s.font_size, MIN_FONT);
    }
}
