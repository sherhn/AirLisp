//! Run configurations: current file, load/test an ASDF system.

use std::path::PathBuf;

use lisp_runtime::{discover, runner};

use crate::app::App;
use crate::editor_view::Sev;
use crate::state::*;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum RunConfig {
    #[default]
    CurrentFile,
    Load(String),
    Test(String),
}

impl App {
    pub fn run_config_label(&self) -> String {
        match &self.run_config {
            RunConfig::CurrentFile => self.doc().map_or("Current File".to_string(), |d| d.name()),
            RunConfig::Load(s) => format!("Load {s}"),
            RunConfig::Test(s) => format!("Test {s}"),
        }
    }

    /// ASDF systems defined by `.asd` files of the project (system name = file stem).
    pub fn asd_systems(&self) -> Vec<String> {
        let Some(p) = &self.project else { return vec![] };
        let mut v: Vec<String> = p.files.iter().filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("asd"))).filter_map(|f| f.file_stem().map(|s| s.to_string_lossy().into_owned())).collect();
        v.sort();
        v.dedup();
        v
    }

    /// Execute the selected run configuration.
    pub fn run_selected(&mut self) {
        match self.run_config.clone() {
            RunConfig::CurrentFile => self.run_current_file(),
            RunConfig::Load(s) => self.run_asdf_system(&s, false),
            RunConfig::Test(s) => self.run_asdf_system(&s, true),
        }
    }

    pub fn run_asdf_system(&mut self, system: &str, test: bool) {
        let Some(root) = self.project_root().map(|p| p.to_path_buf()) else {
            self.toast("Open a project first", Sev::Weak);
            return;
        };
        self.save_all();
        let Some(imp) = self.lisp.clone().or_else(|| discover::find_sbcl(&self.settings.lisp_path)) else {
            self.toast("SBCL not found - set the path in Settings", Sev::Error);
            return;
        };
        if let Some(h) = self.run.handle.take() {
            h.kill();
        }
        match runner::run_asdf(&imp, &root, system, test) {
            Ok(h) => {
                let verb = if test { "Test" } else { "Load" };
                self.run = RunState { handle: Some(h), title: format!("{verb} '{system}'"), log: vec![], running: true, exit_code: None, file: None::<PathBuf> };
                self.run.log.push(LogLine { kind: LogKind::Info, text: format!("{verb} ASDF system '{system}' from {}", root.display()) });
                self.bottom_tool = Some(BottomTool::Run);
            }
            Err(e) => self.toast(format!("Cannot run: {e}"), Sev::Error),
        }
    }

    /// Load the system into the live REPL.
    pub fn repl_load_system(&mut self) {
        let Some(root) = self.project_root().map(|p| p.to_path_buf()) else { return };
        let sys = match &self.run_config {
            RunConfig::Load(s) | RunConfig::Test(s) => Some(s.clone()),
            RunConfig::CurrentFile => self.asd_systems().into_iter().next(),
        };
        let Some(sys) = sys else {
            self.toast("No .asd system found in this project", Sev::Weak);
            return;
        };
        self.save_all();
        // separate top-level forms: `require` must run before the reader meets `asdf:` symbols
        let code = format!(
            "(require :asdf) (asdf:initialize-source-registry (list :source-registry (list :tree {}) :inherit-configuration)) (asdf:load-system {})",
            runner::lisp_string(&runner::lisp_path(&root)),
            runner::lisp_string(&sys)
        );
        self.bottom_tool = Some(BottomTool::Repl);
        self.repl_eval(&code, true);
    }
}
