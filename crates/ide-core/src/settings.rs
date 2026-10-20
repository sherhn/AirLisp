//! Persistent IDE settings (JSON in the platform config directory).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Theme {
    #[default]
    Darcula,
    Light,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub font_size: f32,
    pub show_line_numbers: bool,
    pub show_whitespace: bool,
    pub rainbow_brackets: bool,
    pub auto_save_on_focus_loss: bool,
    pub recent_projects: Vec<String>,
    pub last_project: Option<String>,
    /// Explicit path to the Lisp binary; empty = auto-detect.
    pub lisp_path: String,
    pub open_files: Vec<String>,
    pub active_file: Option<String>,
    pub show_project_panel: bool,
    pub show_bottom_panel: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::Darcula,
            font_size: 14.0,
            show_line_numbers: true,
            show_whitespace: false,
            rainbow_brackets: true,
            auto_save_on_focus_loss: true,
            recent_projects: vec![],
            last_project: None,
            lisp_path: String::new(),
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
        if let Ok(p) = std::env::var("LISPCRAFT_CONFIG_DIR") {
            return Some(PathBuf::from(p));
        }
        dirs::config_dir().map(|d| d.join("Lispcraft"))
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
        Self::config_dir().map(|d| Self::load_from(&d.join("settings.json"))).unwrap_or_default()
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
        let mut s = Settings::default();
        s.theme = Theme::Light;
        s.font_size = 17.0;
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
