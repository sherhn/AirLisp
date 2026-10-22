//! Run configurations: current file, load/test an ASDF system, a make target (in Linux for Linux projects).

use std::path::{Path, PathBuf};
use std::process::Command;

use ide_core::build::{self, Level};
use lisp_runtime::discover::Launch;
use lisp_runtime::runner;
use lisp_runtime::wsl::{self, Target};

use crate::app::App;
use crate::editor_view::Sev;
use crate::i18n::{self, tr};
use crate::state::*;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum RunConfig {
    #[default]
    CurrentFile,
    Load(String),
    Test(String),
    Make(String),
}

/// Where a make run happened, to find the files its compiler messages name.
#[derive(Clone, Debug)]
pub struct MakeRun {
    pub root: PathBuf,
    pub target: Option<Target>,
}

/// `a/b/../c` -> `a/c` for `/`-separated paths.
fn normalize(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            x => out.push(x),
        }
    }
    let joined = out.join("/");
    if p.starts_with('/') { format!("/{joined}") } else { joined }
}

/// The Windows path of a file named in make output.
pub fn message_path(run: &MakeRun, dir: &str, file: &str) -> PathBuf {
    match &run.target {
        Some(t) => {
            let root = wsl::to_linux(&run.root, t);
            let base = if dir.is_empty() { root } else { dir.to_string() };
            let full = if file.starts_with('/') { file.to_string() } else { format!("{base}/{file}") };
            wsl::to_windows(&normalize(&full), t)
        }
        None => {
            let f = Path::new(file);
            if f.is_absolute() {
                f.to_path_buf()
            } else {
                let base = if dir.is_empty() { run.root.clone() } else { PathBuf::from(dir) };
                base.join(f)
            }
        }
    }
}

impl App {
    pub fn run_config_label(&self) -> String {
        match &self.run_config {
            RunConfig::CurrentFile => self.doc().map_or(tr("Current File").to_string(), |d| d.name()),
            RunConfig::Load(s) => i18n::fmt("Load {}", &[&s]),
            RunConfig::Test(s) => i18n::fmt("Test {}", &[&s]),
            RunConfig::Make(t) => format!("make {t}"),
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
            RunConfig::Make(t) => self.run_make(&t),
        }
    }

    /// Re-read the targets of the project's makefile.
    pub fn refresh_make_targets(&mut self) {
        self.make_targets = self.project_root().and_then(build::find_makefile).and_then(|m| std::fs::read_to_string(m).ok()).map(|t| build::targets(&t)).unwrap_or_default();
        if let RunConfig::Make(t) = &self.run_config
            && !self.make_targets.contains(t)
        {
            self.run_config = RunConfig::CurrentFile;
        }
        // a project built with make starts with its first target selected
        if self.run_config == RunConfig::CurrentFile
            && let Some(t) = self.make_targets.first()
            && self.asd_systems().is_empty()
        {
            self.run_config = RunConfig::Make(t.clone());
        }
    }

    /// `make TARGET` in the project folder: inside the project's Linux distribution, or on Windows.
    pub fn run_make(&mut self, target: &str) {
        let Some(root) = self.project_root().map(Path::to_path_buf) else {
            self.toast(tr("Open a project first"), Sev::Weak);
            return;
        };
        self.save_all();
        if let Some(h) = self.run.handle.take() {
            h.kill();
        }
        let t = self.wsl_target();
        let launch = match &t {
            Some(t) => {
                let marker = wsl::new_marker();
                // one stream: make's "Entering directory" lines must come before the compiler messages they explain
                let cmd = wsl::shell(t, &root, &format!("make {} 2>&1", wsl::sh_quote(target)), &[], &marker);
                Launch { cmd, cleanup: Some(wsl::Cleanup { target: t.clone(), marker }) }
            }
            None => {
                let mut cmd = Command::new("make");
                cmd.arg(target).current_dir(&root);
                Launch { cmd, cleanup: None }
            }
        };
        match runner::spawn_launch(launch) {
            Ok(h) => {
                let place = match &t {
                    Some(t) => i18n::fmt("in Linux ({0}): {1}", &[&t.distro, &wsl::to_linux(&root, t)]),
                    None => root.display().to_string(),
                };
                self.run = RunState { handle: Some(h), title: format!("make {target}"), log: vec![], running: true, exit_code: None, file: None };
                self.run.log.push(LogLine { kind: LogKind::Info, text: format!("make {target}   {place}") });
                self.make_run = Some(MakeRun { root, target: t });
                self.compiler_problems.retain(|p| p.source != "make");
                self.bottom_tool = Some(BottomTool::Run);
            }
            Err(e) if t.is_none() && e.kind() == std::io::ErrorKind::NotFound => {
                self.toast(tr("make is not installed on Windows - build this project in Linux (WSL): click Windows in the status bar"), Sev::Error);
            }
            Err(e) => self.toast(i18n::fmt("Cannot run: {}", &[&e]), Sev::Error),
        }
    }

    /// A make run ended: its compiler messages become problems (and markers in open files).
    pub fn make_finished(&mut self, code: Option<i32>) {
        let Some(run) = self.make_run.take() else { return };
        let output: String = self.run.log.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        let messages = build::parse_output(&output);
        if messages.is_empty() && code != Some(0) {
            self.status = tr("make failed").into();
        }
        let (mut errors, mut warnings) = (0, 0);
        let mut by_file: std::collections::HashMap<PathBuf, Vec<crate::editor_view::Marker>> = std::collections::HashMap::new();
        let files: Vec<PathBuf> = self.project.as_ref().map(|p| p.files.clone()).unwrap_or_default();
        for m in &messages {
            let mut path = message_path(&run, &m.dir, &m.file);
            if !path.exists() {
                // the directory was not announced: the one project file with that name (and relative path)
                let tail: Vec<&str> = m.file.split(['/', '\\']).filter(|c| !c.is_empty() && *c != "." && *c != "..").collect();
                let hits: Vec<&PathBuf> = files.iter().filter(|f| {
                    let comps: Vec<String> = f.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                    comps.len() >= tail.len() && comps[comps.len() - tail.len()..].iter().zip(&tail).all(|(a, b)| a == b)
                }).collect();
                if hits.len() == 1 {
                    path = hits[0].clone();
                }
            }
            let text = match self.docs.iter().find(|d| d.path.as_deref() == Some(path.as_path())) {
                Some(d) => d.ed.text().to_string(),
                None => std::fs::read_to_string(&path).unwrap_or_default(),
            };
            let start = build::offset_of(&text, m.line, m.col.max(1));
            let end = text[start..].find('\n').map_or(text.len(), |e| start + e).max(start + 1).min(text.len().max(start));
            let sev = match m.level {
                Level::Error => {
                    errors += 1;
                    Sev::Error
                }
                Level::Warning => {
                    warnings += 1;
                    Sev::Warning
                }
                Level::Note => Sev::Weak,
            };
            by_file.entry(path.clone()).or_default().push(crate::editor_view::Marker { start, end, sev, msg: m.text.clone() });
            self.compiler_problems.push(ProblemRow { path: Some(path), start, end, sev, message: m.text.clone(), source: "make" });
        }
        for d in self.docs.iter_mut() {
            if let Some(ms) = d.path.as_ref().and_then(|p| by_file.remove(p)) {
                d.compiler_markers = ms;
                d.compiled_rev = d.ed.revision();
            }
        }
        if errors > 0 {
            self.status = i18n::fmt("make: {0} error(s), {1} warning(s)", &[&errors, &warnings]);
            self.toast(i18n::fmt("make: {0} error(s), {1} warning(s) - see Problems", &[&errors, &warnings]), Sev::Error);
        } else if code == Some(0) {
            self.status = i18n::fmt("make finished: {} warning(s)", &[&warnings]);
        }
    }

    pub fn run_asdf_system(&mut self, system: &str, test: bool) {
        let Some(root) = self.project_root().map(|p| p.to_path_buf()) else {
            self.toast(tr("Open a project first"), Sev::Weak);
            return;
        };
        self.save_all();
        let Some(imp) = self.lisp.clone().or_else(|| self.resolve_lisp()) else {
            self.toast(tr("SBCL not found - set the path in Settings"), Sev::Error);
            return;
        };
        if let Some(h) = self.run.handle.take() {
            h.kill();
        }
        match runner::run_asdf(&imp, &root, system, test) {
            Ok(h) => {
                let verb = if test { tr("Test") } else { tr("Load") };
                self.run = RunState { handle: Some(h), title: format!("{verb} '{system}'"), log: vec![], running: true, exit_code: None, file: None::<PathBuf> };
                self.run.log.push(LogLine { kind: LogKind::Info, text: i18n::fmt("{0} ASDF system '{1}' from {2}", &[&verb, &system, &root.display()]) });
                self.bottom_tool = Some(BottomTool::Run);
            }
            Err(e) => self.toast(i18n::fmt("Cannot run: {}", &[&e]), Sev::Error),
        }
    }

    /// Load the system into the live REPL.
    pub fn repl_load_system(&mut self) {
        let Some(root) = self.project_root().map(|p| p.to_path_buf()) else { return };
        let sys = match &self.run_config {
            RunConfig::Load(s) | RunConfig::Test(s) => Some(s.clone()),
            RunConfig::CurrentFile | RunConfig::Make(_) => self.asd_systems().into_iter().next(),
        };
        let Some(sys) = sys else {
            self.toast(tr("No .asd system found in this project"), Sev::Weak);
            return;
        };
        self.save_all();
        // separate top-level forms: `require` must run before the reader meets `asdf:` symbols
        let code = format!(
            "(require :asdf) (asdf:initialize-source-registry (list :source-registry (list :tree {}) :inherit-configuration)) (asdf:load-system {})",
            runner::lisp_string(&self.lisp_path_of(&root)),
            runner::lisp_string(&sys)
        );
        self.bottom_tool = Some(BottomTool::Repl);
        self.repl_eval(&code, true);
    }
}
