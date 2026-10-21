//! Plain data types of the application state.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use ide_core::diff::DiffLine;
use ide_core::git::{Branch, Change, Commit, Repo};
use ide_core::index::ProjectIndex;
use ide_core::localvcs::LocalVcs;
use ide_core::workspace::IgnoreMatcher;
use ide_core::search::FileHit;
use ide_core::workspace::Entry;
use lisp_runtime::{ReplSession, RunHandle};

use crate::editor_view::Sev;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftTool {
    Project,
    Structure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomTool {
    Repl,
    Run,
    Problems,
    Find,
    Git,
    Terminal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitTab {
    Changes,
    Log,
    Local,
}

pub struct GitSnapshot {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub changes: Vec<Change>,
    pub branches: Vec<Branch>,
    pub log: Vec<Commit>,
    pub truncated: bool,
}

pub struct GitState {
    pub repo: Option<Repo>,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub changes: Vec<Change>,
    pub branches: Vec<Branch>,
    pub log: Vec<Commit>,
    pub rx: Option<Receiver<GitSnapshot>>,
    pub last_refresh: f64,
    pub commit_msg: String,
    pub checked: HashSet<String>,
    pub busy: Option<String>,
    pub op_rx: Option<Receiver<(String, Result<String, String>)>>,
    pub tab: GitTab,
    pub selected_commit: Option<usize>,
    /// Absolute path -> change kind for tree colouring; directories containing changes.
    pub kinds: HashMap<PathBuf, ide_core::git::ChangeKind>,
    pub dirty_dirs: HashSet<PathBuf>,
    pub available: bool,
    /// Local commits (checkpoints kept outside git).
    pub local: Option<LocalVcs>,
    pub changes_truncated: bool,
}

impl Default for GitState {
    fn default() -> Self {
        GitState {
            repo: None,
            branch: None,
            head: None,
            changes: vec![],
            branches: vec![],
            log: vec![],
            rx: None,
            last_refresh: -100.0,
            commit_msg: String::new(),
            checked: HashSet::new(),
            busy: None,
            op_rx: None,
            tab: GitTab::Changes,
            selected_commit: None,
            kinds: HashMap::new(),
            dirty_dirs: HashSet::new(),
            available: true,
            local: None,
            changes_truncated: false,
        }
    }
}

#[derive(Clone)]
pub struct TreeRow {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
    pub name: String,
    pub ignored: bool,
}

pub struct Project {
    /// Bumped whenever the visible tree may have changed (expand/collapse, refresh).
    pub tree_ver: u64,
    pub tree_cache: Option<(u64, Vec<TreeRow>)>,
    pub ignore: IgnoreMatcher,
    pub root: PathBuf,
    pub name: String,
    pub expanded: HashSet<PathBuf>,
    pub selected: Option<PathBuf>,
    pub dir_cache: HashMap<PathBuf, Vec<Entry>>,
    pub files: Vec<PathBuf>,
    /// Project-relative paths parallel to `files` (computed once, used by Go to File).
    pub files_rel: Vec<String>,
    pub index: ProjectIndex,
    pub index_rx: Option<Receiver<(ProjectIndex, Vec<PathBuf>)>>,
    pub branch: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogKind {
    Input,
    Output,
    Value,
    Error,
    Info,
    Trace,
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
}

#[derive(Clone, Debug)]
pub enum Pending {
    Complete { start: usize, prefix: String, doc_rev: u64 },
    Doc { title: String },
    ParamInfo { name: String },
    Macroexpand,
    Definition { name: String },
}

#[derive(Default)]
pub struct ReplState {
    pub session: Option<ReplSession>,
    pub starting: bool,
    pub ready: bool,
    pub version: String,
    pub package: String,
    pub log: Vec<LogLine>,
    pub input: String,
    pub history: Vec<String>,
    pub hist_pos: Option<usize>,
    pub busy: HashSet<u64>,
    pub pending: HashMap<u64, Pending>,
    pub stick_to_bottom: bool,
    pub focus_input: bool,
}

#[derive(Default)]
pub struct RunState {
    pub handle: Option<RunHandle>,
    pub title: String,
    pub log: Vec<LogLine>,
    pub running: bool,
    pub exit_code: Option<i32>,
    pub file: Option<PathBuf>,
}

pub struct CompileJob {
    pub handle: RunHandle,
    pub fasl: PathBuf,
    pub file: PathBuf,
    pub output: String,
}

#[derive(Clone, Debug)]
pub struct ProblemRow {
    pub path: Option<PathBuf>,
    pub start: usize,
    pub end: usize,
    pub sev: Sev,
    pub message: String,
    pub source: &'static str,
}

#[derive(Default)]
pub struct FindResults {
    pub title: String,
    pub hits: Vec<FileHit>,
}

// ------------------------------------------------------------------ popups

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteMode {
    Everywhere,
    Actions,
    Files,
    Symbols,
    Recent,
    Structure,
    Choose,
}

impl PaletteMode {
    pub fn title(self) -> &'static str {
        match self {
            PaletteMode::Everywhere => "Search Everywhere",
            PaletteMode::Actions => "Find Action",
            PaletteMode::Files => "Go to File",
            PaletteMode::Symbols => "Go to Symbol",
            PaletteMode::Recent => "Recent Files",
            PaletteMode::Structure => "File Structure",
            PaletteMode::Choose => "Choose Declaration",
        }
    }
}

#[derive(Clone, Debug)]
pub enum PTarget {
    Command(String),
    File(PathBuf),
    Symbol { path: PathBuf, offset: usize },
    Offset(usize),
    Branch(String),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub badge: String,
    pub section: &'static str,
    pub target: PTarget,
    /// Matched char positions in `label` for highlighting.
    pub hits: Vec<usize>,
    pub shortcut: String,
}

pub struct PaletteState {
    pub mode: PaletteMode,
    pub query: String,
    pub items: Vec<Item>,
    pub selected: usize,
    pub built_for: Option<(PaletteMode, String)>,
    pub fixed: Option<Vec<Item>>,
    pub just_opened: bool,
}

#[derive(Default)]
pub struct FindBar {
    pub open: bool,
    pub replace_mode: bool,
    pub query: String,
    pub replace: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub matches: Vec<(usize, usize)>,
    pub current: Option<usize>,
    pub built_for: Option<(usize, u64, String, bool, bool)>,
    pub focus: bool,
}


#[derive(Clone, Debug)]
pub struct CItem {
    pub text: String,
    pub kind: String,
    pub source: u8,
}

#[derive(Default)]
pub struct Completion {
    pub active: bool,
    pub items: Vec<CItem>,
    pub selected: usize,
    pub start: usize,
    pub prefix: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmKind {
    CloseDoc(usize),
    CloseAll,
    Exit,
    Delete(PathBuf),
    Reload(usize),
    Rollback(String),
    RestoreLocal(u64),
    DeleteLocal(u64),
}

pub enum Popup {
    None,
    Palette(PaletteState),
    GotoLine { input: String, just_opened: bool },
    Rename { old: String, input: String, just_opened: bool },
    NewFile { dir: PathBuf, input: String, is_dir: bool, just_opened: bool },
    NewProject { parent: String, name: String, just_opened: bool },
    FindInFiles { query: String, replace: String, case_sensitive: bool, whole_word: bool, just_opened: bool, replace_mode: bool },
    Settings { lisp_path: String, detected: String, filter: String },
    About,
    Info { title: String, text: String },
    Confirm { kind: ConfirmKind, message: String },
    Commit { just_opened: bool },
    Diff { title: String, lines: Vec<DiffLine>, scroll_to: Option<usize> },
    NewBranch { input: String, just_opened: bool },
    LocalCommit { input: String, just_opened: bool },
    RenameFile { path: PathBuf, input: String, just_opened: bool },
}

impl Popup {
    pub fn is_open(&self) -> bool {
        !matches!(self, Popup::None)
    }
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub sev: Sev,
    pub until: f64,
}
