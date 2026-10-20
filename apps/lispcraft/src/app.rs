//! Application state and behaviour (no drawing code here).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use eframe::egui;
use ide_core::commands::{self, Keymap};
use ide_core::index::{self, ProjectIndex};
use ide_core::search::{self, TextQuery};
use ide_core::settings::Settings;
use ide_core::workspace;
use lisp_runtime::compiler;
use lisp_runtime::{Implementation, ReplEvent, ReplSession, RunEvent, discover, runner};
use lisp_syntax::outline::outline;
use lisp_syntax::{TokenKind, symbols};

use crate::doc::Doc;
use crate::editor_view::{Marker, Sev};
use crate::state::*;
use crate::theme::Palette;

pub use crate::app_run::RunConfig;

pub struct App {
    pub settings: Settings,
    pub pal: Palette,
    pub keymap: Keymap,
    pub project: Option<Project>,
    pub docs: Vec<Doc>,
    pub active: usize,
    pub recent_files: Vec<PathBuf>,
    pub left_tool: Option<LeftTool>,
    pub bottom_tool: Option<BottomTool>,
    pub repl: ReplState,
    pub run: RunState,
    pub compile: Option<CompileJob>,
    pub compiler_problems: Vec<ProblemRow>,
    pub find_results: FindResults,
    pub popup: Popup,
    pub find: FindBar,
    pub completion: Completion,
    pub toasts: Vec<Toast>,
    pub hint: Option<(String, f64)>,
    pub status: String,
    pub editor_focus: bool,
    pub nav_back: Vec<(PathBuf, usize)>,
    pub nav_fwd: Vec<(PathBuf, usize)>,
    pub lisp: Option<Implementation>,
    pub untitled_counter: usize,
    pub last_edit: Option<(PathBuf, usize)>,
    pub last_title: String,
    pub now: f64,
    pub last_disk_check: f64,
    pub window_focused: bool,
    pub exit_now: bool,
    pub font_names: (Option<String>, Option<String>),
    pub editor_rect: Option<egui::Rect>,
    pub caret_screen: Option<egui::Pos2>,
    pub pending_close_others: bool,
    pub git: GitState,
    pub term: crate::app_terminal::TerminalState,
    pub run_config: crate::app_run::RunConfig,
    pub script: Option<crate::script::Script>,
    /// Double-Shift detection: (shift currently down, another key was used meanwhile, time of the last clean tap).
    pub shift_tap: (bool, bool, f64),
}

pub fn lisp_dialect_of(p: &Path) -> lisp_syntax::Dialect {
    lisp_syntax::Dialect::from_path(p).unwrap_or_default()
}

impl App {
    pub fn new(settings: Settings) -> App {
        let pal = Palette::for_theme(settings.theme);
        let lisp = discover::find_sbcl(&settings.lisp_path);
        App {
            pal,
            keymap: Keymap::default_keymap(),
            project: None,
            docs: vec![],
            active: 0,
            recent_files: vec![],
            left_tool: if settings.show_project_panel { Some(LeftTool::Project) } else { None },
            bottom_tool: None,
            repl: ReplState { package: "COMMON-LISP-USER".into(), stick_to_bottom: true, ..Default::default() },
            run: RunState::default(),
            compile: None,
            compiler_problems: vec![],
            find_results: FindResults::default(),
            popup: Popup::None,
            find: FindBar::default(),
            completion: Completion::default(),
            toasts: vec![],
            hint: None,
            status: String::new(),
            editor_focus: true,
            nav_back: vec![],
            nav_fwd: vec![],
            lisp,
            untitled_counter: 0,
            last_edit: None,
            last_title: String::new(),
            now: 0.0,
            last_disk_check: 0.0,
            window_focused: true,
            exit_now: false,
            font_names: (None, None),
            editor_rect: None,
            caret_screen: None,
            pending_close_others: false,
            git: GitState::default(),
            term: Default::default(),
            run_config: Default::default(),
            script: None,
            shift_tap: (false, false, -10.0),
            settings,
        }
    }

    // ------------------------------------------------------------------ small helpers

    pub fn toast(&mut self, text: impl Into<String>, sev: Sev) {
        let text = text.into();
        self.toasts.push(Toast { text, sev, until: self.now + 4.5 });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    pub fn doc(&self) -> Option<&Doc> {
        self.docs.get(self.active)
    }

    pub fn doc_mut(&mut self) -> Option<&mut Doc> {
        self.docs.get_mut(self.active)
    }

    pub fn project_root(&self) -> Option<&Path> {
        self.project.as_ref().map(|p| p.root.as_path())
    }

    pub fn theme_changed(&mut self) {
        self.pal = Palette::for_theme(self.settings.theme);
    }

    pub fn save_settings(&mut self) {
        self.settings.open_files = self.docs.iter().filter_map(|d| d.path.as_ref().map(|p| p.to_string_lossy().into_owned())).collect();
        self.settings.active_file = self.doc().and_then(|d| d.path.as_ref()).map(|p| p.to_string_lossy().into_owned());
        self.settings.show_project_panel = self.left_tool.is_some();
        if let Err(e) = self.settings.save() {
            eprintln!("could not save settings: {e}");
        }
    }

    // ------------------------------------------------------------------ project

    pub fn open_project(&mut self, path: &Path) {
        if !path.is_dir() {
            self.toast(format!("{} is not a folder", path.display()), Sev::Error);
            return;
        }
        let root = path.to_path_buf();
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.to_string_lossy().into_owned());
        let (tx, rx) = mpsc::channel();
        let r2 = root.clone();
        std::thread::spawn(move || {
            let idx = ProjectIndex::build(&r2);
            let files = workspace::list_files(&r2, 50_000);
            let _ = tx.send((idx, files));
        });
        let mut expanded = HashSet::new();
        expanded.insert(root.clone());
        let branch = workspace::git_branch(&root);
        self.project = Some(Project { root: root.clone(), name, expanded, selected: None, dir_cache: HashMap::new(), files: vec![], index: ProjectIndex::default(), index_rx: Some(rx), branch });
        self.settings.push_recent_project(&root.to_string_lossy());
        self.left_tool = Some(LeftTool::Project);
        self.compiler_problems.clear();
        self.git_attach();
        // restart the REPL in the new working directory on next use
        if self.repl.session.is_some() {
            self.repl_restart();
        }
        self.save_settings();
    }

    pub fn close_project(&mut self) {
        self.project = None;
        self.docs.clear();
        self.active = 0;
        self.compiler_problems.clear();
        self.git = GitState::default();
    }

    pub fn poll_index(&mut self) {
        if let Some(p) = self.project.as_mut()
            && let Some(rx) = &p.index_rx
            && let Ok((idx, files)) = rx.try_recv()
        {
            p.index = idx;
            p.files = files;
            p.index_rx = None;
        }
    }

    pub fn dir_entries(&mut self, dir: &Path) -> Vec<workspace::Entry> {
        let Some(p) = self.project.as_mut() else { return vec![] };
        if let Some(v) = p.dir_cache.get(dir) {
            return v.clone();
        }
        let v = workspace::list_dir(dir).unwrap_or_default();
        p.dir_cache.insert(dir.to_path_buf(), v.clone());
        v
    }

    pub fn refresh_project(&mut self) {
        let Some(root) = self.project.as_ref().map(|p| p.root.clone()) else { return };
        if let Some(p) = self.project.as_mut() {
            p.dir_cache.clear();
            p.branch = workspace::git_branch(&root);
            if p.index_rx.is_none() {
                let (tx, rx) = mpsc::channel();
                let r2 = root.clone();
                std::thread::spawn(move || {
                    let idx = ProjectIndex::build(&r2);
                    let files = workspace::list_files(&r2, 50_000);
                    let _ = tx.send((idx, files));
                });
                p.index_rx = Some(rx);
            }
        }
    }

    pub fn reindex_open_docs(&mut self) {
        if let Some(p) = self.project.as_mut() {
            for d in &self.docs {
                if let Some(path) = &d.path {
                    p.index.update_file(path, d.ed.text(), d.ed.dialect());
                }
            }
        }
    }

    // ------------------------------------------------------------------ documents

    pub fn open_file(&mut self, path: &Path) -> bool {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(path)) {
            self.active = i;
            self.editor_focus = true;
            return true;
        }
        match std::fs::metadata(path) {
            Ok(m) if m.len() > 8 * 1024 * 1024 => {
                self.toast(format!("{} is too large to open ({} MB)", path.display(), m.len() / 1_048_576), Sev::Error);
                return false;
            }
            _ => {}
        }
        match Doc::open(path) {
            Ok(d) => {
                if let Some(p) = self.project.as_mut() {
                    p.index.update_file(path, d.ed.text(), d.ed.dialect());
                }
                self.docs.push(d);
                self.active = self.docs.len() - 1;
                self.editor_focus = true;
                self.recent_files.retain(|p| p != path);
                self.recent_files.insert(0, path.to_path_buf());
                self.recent_files.truncate(30);
                true
            }
            Err(e) => {
                self.toast(format!("Cannot open {}: {e}", path.display()), Sev::Error);
                false
            }
        }
    }

    /// Open a file and put the caret at `offset` (a byte offset).
    pub fn goto(&mut self, path: &Path, offset: usize, record_nav: bool) {
        if record_nav {
            self.push_nav();
        }
        if !self.open_file(path) {
            return;
        }
        if let Some(d) = self.doc_mut() {
            let o = offset.min(d.ed.text().len());
            let o = (0..=o).rev().find(|&i| d.ed.text().is_char_boundary(i)).unwrap_or(0);
            d.ed.set_caret(o, false);
            d.view.scroll_to_caret = true;
            d.view.last_input_time = 0.0;
        }
        self.editor_focus = true;
    }

    pub fn goto_range(&mut self, path: &Path, start: usize, end: usize, record_nav: bool) {
        self.goto(path, start, record_nav);
        if let Some(d) = self.doc_mut() {
            let n = d.ed.text().len();
            d.ed.select_range(start.min(n), end.min(n));
        }
    }

    pub fn push_nav(&mut self) {
        if let Some(d) = self.doc()
            && let Some(p) = &d.path
        {
            let entry = (p.clone(), d.ed.caret());
            if self.nav_back.last() != Some(&entry) {
                self.nav_back.push(entry);
                if self.nav_back.len() > 100 {
                    self.nav_back.remove(0);
                }
            }
            self.nav_fwd.clear();
        }
    }

    pub fn navigate_back(&mut self) {
        let Some((p, o)) = self.nav_back.pop() else { return };
        if let Some(d) = self.doc()
            && let Some(cur) = &d.path
        {
            self.nav_fwd.push((cur.clone(), d.ed.caret()));
        }
        self.goto(&p, o, false);
    }

    pub fn navigate_forward(&mut self) {
        let Some((p, o)) = self.nav_fwd.pop() else { return };
        if let Some(d) = self.doc()
            && let Some(cur) = &d.path
        {
            self.nav_back.push((cur.clone(), d.ed.caret()));
        }
        self.goto(&p, o, false);
    }

    pub fn new_untitled(&mut self) {
        self.untitled_counter += 1;
        self.docs.push(Doc::untitled(self.untitled_counter));
        self.active = self.docs.len() - 1;
        self.editor_focus = true;
    }

    /// Save one document; untitled documents ask for a location.
    pub fn save_doc(&mut self, idx: usize) -> bool {
        let Some(d) = self.docs.get_mut(idx) else { return false };
        if d.path.is_none() {
            let start = self.project.as_ref().map(|p| p.root.clone());
            let mut dlg = rfd::FileDialog::new().set_file_name(d.name());
            if let Some(s) = start {
                dlg = dlg.set_directory(s);
            }
            match dlg.save_file() {
                Some(path) => {
                    if let Err(e) = d.save_as(&path) {
                        let msg = format!("Cannot save: {e}");
                        self.toast(msg, Sev::Error);
                        return false;
                    }
                }
                None => return false,
            }
        } else if let Err(e) = d.save() {
            let msg = format!("Cannot save {}: {e}", d.name());
            self.toast(msg, Sev::Error);
            return false;
        }
        let (path, text, dialect) = {
            let d = &self.docs[idx];
            (d.path.clone(), d.ed.text().to_string(), d.ed.dialect())
        };
        if let (Some(p), Some(proj)) = (path.as_ref(), self.project.as_mut()) {
            proj.index.update_file(p, &text, dialect);
            proj.dir_cache.clear();
        }
        self.git.last_refresh = -100.0;
        true
    }

    pub fn save_all(&mut self) {
        let mut saved = 0;
        for i in 0..self.docs.len() {
            if self.docs[i].ed.is_modified() && self.docs[i].path.is_some() && self.save_doc(i) {
                saved += 1;
            }
        }
        if saved > 0 {
            self.status = format!("Saved {saved} file(s)");
        }
        // untitled documents are saved only by an explicit Save
        if let Some(d) = self.doc()
            && d.path.is_none()
        {
            let a = self.active;
            self.save_doc(a);
        }
    }

    pub fn has_unsaved(&self) -> bool {
        self.docs.iter().any(|d| d.ed.is_modified())
    }

    pub fn close_doc(&mut self, idx: usize) {
        if idx >= self.docs.len() {
            return;
        }
        self.docs.remove(idx);
        if self.active >= self.docs.len() {
            self.active = self.docs.len().saturating_sub(1);
        } else if idx < self.active {
            self.active -= 1;
        }
        self.find.built_for = None;
        self.completion.active = false;
    }

    pub fn request_close_doc(&mut self, idx: usize) {
        match self.docs.get(idx) {
            Some(d) if d.ed.is_modified() => {
                self.popup = Popup::Confirm { kind: ConfirmKind::CloseDoc(idx), message: format!("Save changes to {} before closing?", d.name()) };
            }
            Some(_) => self.close_doc(idx),
            None => {}
        }
    }

    // ------------------------------------------------------------------ periodic work

    pub fn tick(&mut self, ctx: &egui::Context) {
        self.now = ctx.input(|i| i.time);
        self.toasts.retain(|t| t.until > self.now);
        if self.hint.as_ref().is_some_and(|h| h.1 < self.now) {
            self.hint = None;
        }
        self.poll_index();
        self.poll_repl();
        self.poll_run();
        self.poll_compile();
        self.git_poll();
        self.poll_terminal();
        self.git_update_bases();

        // external file changes (once a second)
        if self.now - self.last_disk_check > 1.0 {
            self.last_disk_check = self.now;
            let mut reloaded = vec![];
            for d in self.docs.iter_mut() {
                if d.sync_with_disk() {
                    reloaded.push(d.name());
                }
            }
            for n in reloaded {
                self.toast(format!("{n} reloaded from disk"), Sev::Weak);
            }
            if let Some(p) = self.project.as_mut() {
                p.dir_cache.clear();
            }
        }
        // save on focus loss
        let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
        if self.window_focused && !focused && self.settings.auto_save_on_focus_loss {
            for i in 0..self.docs.len() {
                if self.docs[i].ed.is_modified() && self.docs[i].path.is_some() {
                    self.save_doc(i);
                }
            }
        }
        self.window_focused = focused;
    }

    // ------------------------------------------------------------------ REPL

    pub fn ensure_repl(&mut self) -> bool {
        if self.repl.session.as_ref().is_some_and(|s| s.is_alive()) {
            return true;
        }
        self.repl.session = None;
        if self.lisp.is_none() {
            self.lisp = discover::find_sbcl(&self.settings.lisp_path);
        }
        let Some(imp) = self.lisp.clone() else {
            self.repl.log.push(LogLine { kind: LogKind::Error, text: "No Lisp implementation found. Install SBCL (https://www.sbcl.org) or set its path in Settings > Lisp.".into() });
            self.toast("SBCL not found - set the path in Settings", Sev::Error);
            return false;
        };
        let cwd = self.project.as_ref().map(|p| p.root.clone());
        match ReplSession::start(&imp, cwd.as_deref()) {
            Ok(s) => {
                self.repl.session = Some(s);
                self.repl.starting = true;
                self.repl.ready = false;
                self.repl.busy.clear();
                self.repl.pending.clear();
                self.repl.log.push(LogLine { kind: LogKind::Info, text: format!("Starting {} ...", imp.name) });
                true
            }
            Err(e) => {
                self.repl.log.push(LogLine { kind: LogKind::Error, text: format!("Cannot start {}: {e}", imp.path.display()) });
                self.toast(format!("Cannot start Lisp: {e}"), Sev::Error);
                false
            }
        }
    }

    pub fn repl_restart(&mut self) {
        if let Some(s) = self.repl.session.take() {
            s.kill();
        }
        self.repl.ready = false;
        self.repl.starting = false;
        self.repl.busy.clear();
        self.repl.pending.clear();
        self.repl.log.push(LogLine { kind: LogKind::Info, text: "REPL restarted".into() });
        self.ensure_repl();
    }

    pub fn repl_eval(&mut self, code: &str, echo: bool) {
        if code.trim().is_empty() || !self.ensure_repl() {
            return;
        }
        if echo {
            self.repl.log.push(LogLine { kind: LogKind::Input, text: code.trim_end().to_string() });
        }
        let res = self.repl.session.as_ref().map(|s| s.eval(code));
        match res {
            Some(Ok(id)) => {
                self.repl.busy.insert(id);
                self.repl.stick_to_bottom = true;
            }
            Some(Err(e)) => self.repl.log.push(LogLine { kind: LogKind::Error, text: format!("Cannot send to REPL: {e}") }),
            None => {}
        }
    }

    pub fn repl_request(&mut self, kind: &str, payload: &str, pending: Pending) -> bool {
        if !self.repl.ready || self.repl.session.is_none() {
            return false;
        }
        let r = self.repl.session.as_ref().map(|s| s.request(kind, payload));
        if let Some(Ok(id)) = r {
            self.repl.pending.insert(id, pending);
            return true;
        }
        false
    }

    fn log_repl_output(&mut self, text: &str) {
        let text = text.replace('\r', "");
        for seg in text.split_inclusive('\n') {
            match self.repl.log.last_mut() {
                Some(l) if l.kind == LogKind::Output && !l.text.ends_with('\n') => l.text.push_str(seg),
                _ => self.repl.log.push(LogLine { kind: LogKind::Output, text: seg.to_string() }),
            }
        }
    }

    pub fn poll_repl(&mut self) {
        let mut events = Vec::new();
        if let Some(s) = &self.repl.session {
            while let Some(e) = s.try_recv() {
                events.push(e);
                if events.len() > 500 {
                    break;
                }
            }
        }
        for e in events {
            match e {
                ReplEvent::Ready(v) => {
                    self.repl.ready = true;
                    self.repl.starting = false;
                    self.repl.version = v.clone();
                    self.repl.log.push(LogLine { kind: LogKind::Info, text: format!("{v} ready.") });
                }
                ReplEvent::Output(t) => self.log_repl_output(&t),
                ReplEvent::Value { text, .. } => {
                    for l in text.lines() {
                        self.repl.log.push(LogLine { kind: LogKind::Value, text: l.to_string() });
                    }
                }
                ReplEvent::Error { message, .. } => {
                    for l in message.lines() {
                        self.repl.log.push(LogLine { kind: LogKind::Error, text: l.to_string() });
                    }
                }
                ReplEvent::Trace { text, .. } => {
                    for l in text.lines().take(12) {
                        self.repl.log.push(LogLine { kind: LogKind::Trace, text: l.to_string() });
                    }
                }
                ReplEvent::Done { id, package } => {
                    self.repl.busy.remove(&id);
                    self.repl.package = package;
                }
                ReplEvent::Reply { id, payload } => {
                    if let Some(p) = self.repl.pending.remove(&id) {
                        self.handle_reply(p, &payload);
                    }
                }
                ReplEvent::Exited(code) => {
                    self.repl.log.push(LogLine { kind: LogKind::Info, text: format!("REPL process exited (code {})", code.map_or("?".to_string(), |c| c.to_string())) });
                    self.repl.session = None;
                    self.repl.ready = false;
                    self.repl.starting = false;
                    self.repl.busy.clear();
                    self.repl.pending.clear();
                }
            }
        }
        if self.repl.log.len() > 6000 {
            self.repl.log.drain(..1000);
        }
        if !self.repl.log.is_empty() {
            self.repl.stick_to_bottom = true;
        }
    }

    fn handle_reply(&mut self, p: Pending, payload: &str) {
        match p {
            Pending::Complete { start, prefix, .. } => {
                if self.completion.active && self.completion.start == start && self.completion.prefix == prefix {
                    let mut live: Vec<CItem> = payload
                        .lines()
                        .filter_map(|l| {
                            let (n, k) = l.split_once('\t')?;
                            Some(CItem { text: n.to_string(), kind: k.to_string(), source: 0 })
                        })
                        .collect();
                    let mut seen: HashSet<String> = live.iter().map(|c| c.text.to_lowercase()).collect();
                    for c in self.completion.items.drain(..) {
                        if seen.insert(c.text.to_lowercase()) {
                            live.push(c);
                        }
                    }
                    live.retain(|c| c.text.to_lowercase() != prefix.to_lowercase() || live_len_gt1());
                    live.truncate(80);
                    if !live.is_empty() {
                        self.completion.selected = 0;
                        self.completion.items = live;
                    } else {
                        self.completion.active = false;
                    }
                }
            }
            Pending::Doc { title } => {
                let text = if payload.trim().is_empty() { "No documentation available.".to_string() } else { payload.to_string() };
                self.popup = Popup::Info { title, text };
            }
            Pending::ParamInfo { name } => {
                if !payload.trim().is_empty() {
                    self.hint = Some((payload.to_string(), self.now + 6.0));
                } else {
                    self.hint = Some((format!("No parameter info for {name}"), self.now + 3.0));
                }
            }
            Pending::Macroexpand => {
                self.popup = Popup::Info { title: "Macroexpand".into(), text: payload.to_string() };
            }
            Pending::Definition { name } => {
                let first = payload.lines().next().unwrap_or("");
                if let Some((path, off)) = first.split_once('\t') {
                    let p = PathBuf::from(path);
                    let char_off: usize = off.trim().parse().unwrap_or(0);
                    let byte = std::fs::read_to_string(&p).ok().map(|t| t.char_indices().nth(char_off).map_or(t.len(), |(b, _)| b)).unwrap_or(0);
                    self.goto(&p, byte, true);
                } else {
                    self.toast(format!("Cannot find declaration of {name}"), Sev::Weak);
                }
            }
        }
    }

    // ------------------------------------------------------------------ run / compile

    pub fn run_current_file(&mut self) {
        let Some(d) = self.doc() else { return };
        if d.path.is_none() {
            let a = self.active;
            if !self.save_doc(a) {
                return;
            }
        }
        self.save_all();
        let Some(path) = self.doc().and_then(|d| d.path.clone()) else { return };
        let Some(imp) = self.lisp.clone().or_else(|| discover::find_sbcl(&self.settings.lisp_path)) else {
            self.toast("SBCL not found - set the path in Settings", Sev::Error);
            return;
        };
        if let Some(h) = self.run.handle.take() {
            h.kill();
        }
        match runner::run_file(&imp, &path, self.project_root()) {
            Ok(h) => {
                self.run = RunState { handle: Some(h), title: format!("Run '{}'", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()), log: vec![], running: true, exit_code: None, file: Some(path.clone()) };
                self.run.log.push(LogLine { kind: LogKind::Info, text: format!("{} --non-interactive --load {}", imp.path.display(), path.display()) });
                self.bottom_tool = Some(BottomTool::Run);
            }
            Err(e) => self.toast(format!("Cannot run: {e}"), Sev::Error),
        }
    }

    pub fn stop_run(&mut self) {
        if let Some(h) = &self.run.handle {
            h.kill();
        }
        if self.run.running {
            self.run.log.push(LogLine { kind: LogKind::Info, text: "Process terminated by user".into() });
            self.run.running = false;
        }
        if let Some(c) = self.compile.take() {
            c.handle.kill();
            let _ = std::fs::remove_file(&c.fasl);
        }
    }

    pub fn poll_run(&mut self) {
        let mut exited = None;
        let mut lines = Vec::new();
        if let Some(h) = &self.run.handle {
            while let Some(ev) = h.try_recv() {
                match ev {
                    RunEvent::Output { text, is_err } => lines.push((text, is_err)),
                    RunEvent::Exited(c) => exited = Some(c),
                }
            }
        }
        for (text, is_err) in lines {
            let text = text.replace('\r', "");
            for seg in text.split_inclusive('\n') {
                let kind = if is_err { LogKind::Error } else { LogKind::Output };
                match self.run.log.last_mut() {
                    Some(l) if l.kind == kind && !l.text.ends_with('\n') => l.text.push_str(seg),
                    _ => self.run.log.push(LogLine { kind, text: seg.to_string() }),
                }
            }
        }
        if let Some(code) = exited {
            self.run.running = false;
            self.run.exit_code = code;
            let msg = format!("Process finished with exit code {}", code.map_or("?".into(), |c| c.to_string()));
            self.run.log.push(LogLine { kind: if code == Some(0) { LogKind::Info } else { LogKind::Error }, text: msg });
            self.run.handle = None;
        }
    }

    pub fn compile_current_file(&mut self) {
        if self.doc().is_none() {
            return;
        }
        if self.doc().is_some_and(|d| d.path.is_none()) {
            let a = self.active;
            if !self.save_doc(a) {
                return;
            }
        }
        self.save_all();
        let Some(path) = self.doc().and_then(|d| d.path.clone()) else { return };
        let Some(imp) = self.lisp.clone().or_else(|| discover::find_sbcl(&self.settings.lisp_path)) else {
            self.toast("SBCL not found - set the path in Settings", Sev::Error);
            return;
        };
        match compiler::compile_file(&imp, &path) {
            Ok((handle, fasl)) => {
                self.compile = Some(CompileJob { handle, fasl, file: path, output: String::new() });
                self.status = "Compiling...".into();
            }
            Err(e) => self.toast(format!("Cannot compile: {e}"), Sev::Error),
        }
    }

    pub fn poll_compile(&mut self) {
        let Some(job) = self.compile.as_mut() else { return };
        let mut exit = None;
        while let Some(ev) = job.handle.try_recv() {
            match ev {
                RunEvent::Output { text, .. } => job.output.push_str(&text),
                RunEvent::Exited(c) => exit = Some(c),
            }
        }
        let Some(code) = exit else { return };
        let job = self.compile.take().unwrap();
        let _ = std::fs::remove_file(&job.fasl);
        let problems = compiler::parse_compiler_output(&job.output);
        let (text, dialect, rev) = match self.docs.iter().find(|d| d.path.as_deref() == Some(job.file.as_path())) {
            Some(d) => (d.ed.text().to_string(), d.ed.dialect(), d.ed.revision()),
            None => (std::fs::read_to_string(&job.file).unwrap_or_default(), lisp_dialect_of(&job.file), 0),
        };
        self.compiler_problems.retain(|p| p.path.as_deref() != Some(job.file.as_path()));
        let mut markers = Vec::new();
        let (mut errors, mut warnings) = (0, 0);
        for p in &problems {
            let loc = compiler::locate(p, &text, dialect);
            let sev = match p.severity {
                compiler::Severity::Error => {
                    errors += 1;
                    Sev::Error
                }
                compiler::Severity::Warning => {
                    warnings += 1;
                    Sev::Warning
                }
                _ => Sev::Weak,
            };
            let (s, e) = loc.unwrap_or((0, 0));
            if loc.is_some() {
                markers.push(Marker { start: s, end: e.max(s + 1), sev, msg: p.message.lines().next().unwrap_or("").to_string() });
            }
            self.compiler_problems.push(ProblemRow { path: Some(job.file.clone()), start: s, end: e, sev, message: p.message.lines().next().unwrap_or("").to_string(), source: "compiler" });
        }
        if let Some(d) = self.docs.iter_mut().find(|d| d.path.as_deref() == Some(job.file.as_path())) {
            d.compiler_markers = markers;
            d.compiled_rev = rev;
        }
        let name = job.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if code == Some(0) && errors == 0 {
            self.status = format!("Compiled {name}: {warnings} warning(s)");
            self.toast(format!("Compiled {name} successfully ({warnings} warnings)"), Sev::Weak);
        } else {
            self.status = format!("Compilation of {name} failed");
            self.toast(format!("Compilation of {name}: {errors} error(s), {warnings} warning(s)"), Sev::Error);
            self.bottom_tool = Some(BottomTool::Problems);
        }
    }

    // ------------------------------------------------------------------ problems

    pub fn all_problems(&self) -> Vec<ProblemRow> {
        let mut v = Vec::new();
        for d in &self.docs {
            for diag in &d.ed.parsed().diagnostics {
                v.push(ProblemRow {
                    path: d.path.clone(),
                    start: diag.start,
                    end: diag.end,
                    sev: if diag.severity == lisp_syntax::Severity::Error { Sev::Error } else { Sev::Warning },
                    message: diag.message.clone(),
                    source: "syntax",
                });
            }
        }
        v.extend(self.compiler_problems.iter().cloned());
        v
    }

    // ------------------------------------------------------------------ symbols / navigation

    pub fn symbol_at_caret(&self) -> Option<String> {
        let d = self.doc()?;
        let (s, e) = d.ed.symbol_range_at(d.ed.caret());
        (s < e).then(|| d.ed.text()[s..e].to_string())
    }

    pub fn goto_declaration(&mut self, name: Option<String>) {
        let Some(name) = name.or_else(|| self.symbol_at_caret()) else { return };
        self.reindex_open_docs();
        let defs = self.project.as_ref().map(|p| p.index.find_definitions(&name)).unwrap_or_default();
        match defs.len() {
            0 => {
                if !self.repl_request("definition", &name, Pending::Definition { name: name.clone() }) {
                    self.toast(format!("Cannot find declaration of '{name}'"), Sev::Weak);
                }
            }
            1 => {
                let d = &defs[0];
                self.goto(&d.path, d.def.name_start, true);
            }
            _ => {
                let root = self.project_root().map(Path::to_path_buf);
                let items: Vec<Item> = defs
                    .iter()
                    .map(|d| Item {
                        label: d.def.name.clone(),
                        detail: root.as_ref().map_or_else(|| d.path.to_string_lossy().into_owned(), |r| workspace::relative(r, &d.path)),
                        badge: d.def.kind.badge().to_string(),
                        section: "",
                        target: PTarget::Symbol { path: d.path.clone(), offset: d.def.name_start },
                        hits: vec![],
                        shortcut: String::new(),
                    })
                    .collect();
                self.popup = Popup::Palette(PaletteState { mode: PaletteMode::Choose, query: String::new(), items: items.clone(), selected: 0, built_for: None, fixed: Some(items), just_opened: true });
            }
        }
    }

    fn local_definition_text(&self, name: &str) -> Option<String> {
        let defs = self.project.as_ref()?.index.find_definitions(name);
        let d = defs.first()?;
        let text = self
            .docs
            .iter()
            .find(|x| x.path.as_deref() == Some(d.path.as_path()))
            .map(|x| x.ed.text().to_string())
            .or_else(|| std::fs::read_to_string(&d.path).ok())?;
        let dialect = lisp_dialect_of(&d.path);
        let p = lisp_syntax::parse(&text, dialect);
        let tl = p.top_level();
        let form = tl.iter().find(|f| p.start_of(**f) == d.def.start)?;
        let open = p.partner.get(form.last).copied().flatten()?;
        let kids = p.children(open);
        let sig_end = kids.iter().take(3).map(|k| p.end_of(*k)).max()?;
        let header = text[d.def.start..sig_end].to_string() + ")";
        let doc = kids.iter().skip(3).find(|k| p.tokens[k.first].kind == TokenKind::String).map(|k| text[p.start_of(*k)..p.end_of(*k)].trim_matches('"').to_string());
        Some(match doc {
            Some(dc) => format!("{header}\n\n{dc}"),
            None => header,
        })
    }

    pub fn show_quick_doc(&mut self) {
        let Some(name) = self.symbol_at_caret() else { return };
        self.reindex_open_docs();
        if let Some(t) = self.local_definition_text(&name) {
            self.popup = Popup::Info { title: name, text: t };
            return;
        }
        if self.repl_request("doc", &name, Pending::Doc { title: name.clone() }) {
            return;
        }
        let bare = symbols::strip_package(&name);
        let text = if symbols::is_keyword(bare) {
            format!("{bare}\n\nCommon Lisp special form / macro.\nStart the REPL (Alt+8) to see the full documentation from the running image.")
        } else if symbols::is_builtin_function(bare) {
            format!("{bare}\n\nCommon Lisp standard function.\nStart the REPL (Alt+8) to see the full documentation from the running image.")
        } else {
            format!("No documentation found for '{name}'.")
        };
        self.popup = Popup::Info { title: name, text };
    }

    pub fn show_param_info(&mut self) {
        // operator of the innermost enclosing list
        let Some(d) = self.doc() else { return };
        let p = d.ed.parsed();
        let Some(open) = p.enclosing_open(d.ed.caret()) else { return };
        let kids = p.children(open);
        let Some(head) = kids.first() else { return };
        let name = d.ed.text()[p.start_of(*head)..p.end_of(*head)].to_string();
        self.reindex_open_docs();
        if let Some(t) = self.local_definition_text(&name) {
            self.hint = Some((t.lines().next().unwrap_or("").to_string(), self.now + 6.0));
            return;
        }
        if !self.repl_request("arglist", &name, Pending::ParamInfo { name: name.clone() }) {
            self.hint = Some((format!("{name}: start the REPL (Alt+8) for parameter info"), self.now + 3.0));
        }
    }

    pub fn macroexpand_form(&mut self) {
        let Some(d) = self.doc() else { return };
        let range = d.ed.sexp_before_caret().or_else(|| d.ed.form_to_eval());
        let Some((s, e)) = range else { return };
        let form = d.ed.text()[s..e].to_string();
        if !self.repl_request("macroexpand", &form, Pending::Macroexpand) {
            self.ensure_repl();
            self.toast("Starting REPL - run Macroexpand again when it is ready", Sev::Weak);
        }
    }

    pub fn find_usages(&mut self) {
        let Some(name) = self.symbol_at_caret() else { return };
        let Some(root) = self.project_root().map(Path::to_path_buf) else { return };
        self.reindex_open_docs();
        let hits = index::find_usages(&root, &name, 2000);
        self.find_results = FindResults { title: format!("Usages of '{name}'"), hits };
        self.bottom_tool = Some(BottomTool::Find);
    }

    pub fn run_find_in_files(&mut self, query: &str, opts: TextQuery) {
        let Some(root) = self.project_root().map(Path::to_path_buf) else {
            self.toast("Open a project first", Sev::Weak);
            return;
        };
        self.save_all();
        let hits = search::find_in_files(&root, query, opts, 3000);
        self.find_results = FindResults { title: format!("Find '{query}'"), hits };
        self.bottom_tool = Some(BottomTool::Find);
    }

    /// Rename every code occurrence of `old` (project files and open buffers).
    pub fn rename_symbol(&mut self, old: &str, new: &str) {
        if new.is_empty() || new.chars().any(|c| c.is_whitespace() || "()\"';".contains(c)) {
            self.toast("Invalid symbol name", Sev::Error);
            return;
        }
        let mut count = 0;
        let mut files = 0;
        let mut paths: Vec<PathBuf> = self.project.as_ref().map(|p| p.files.clone()).unwrap_or_default();
        for d in &self.docs {
            if let Some(p) = &d.path
                && !paths.contains(p)
            {
                paths.push(p.clone());
            }
        }
        for path in paths {
            let dialect = lisp_dialect_of(&path);
            if lisp_syntax::Dialect::from_path(&path).is_none() {
                continue;
            }
            if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(path.as_path())) {
                let ranges = index::symbol_ranges(self.docs[i].ed.text(), dialect, old);
                if !ranges.is_empty() {
                    count += ranges.len();
                    files += 1;
                    self.docs[i].ed.replace_ranges(&ranges, new);
                }
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                let ranges = index::symbol_ranges(&text, dialect, old);
                if ranges.is_empty() {
                    continue;
                }
                let mut out = text.clone();
                for &(s, e) in ranges.iter().rev() {
                    out.replace_range(s..e, new);
                }
                if std::fs::write(&path, out).is_ok() {
                    count += ranges.len();
                    files += 1;
                }
            }
        }
        self.refresh_project();
        self.toast(format!("Renamed {count} occurrence(s) in {files} file(s)"), Sev::Weak);
    }

    // ------------------------------------------------------------------ completion

    pub fn refresh_completion(&mut self, explicit: bool) {
        let Some(d) = self.docs.get(self.active) else {
            self.completion.active = false;
            return;
        };
        let caret = d.ed.caret();
        let (s, _) = d.ed.symbol_range_at(caret);
        let prefix = d.ed.text()[s..caret].to_string();
        if (prefix.is_empty() && !explicit) || (!explicit && prefix.chars().count() < 2) || d.ed.has_selection() {
            self.completion.active = false;
            return;
        }
        if d.ed.parsed().in_line_comment(caret) {
            self.completion.active = false;
            return;
        }
        let low = prefix.to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CItem> = Vec::new();
        let mut push = |text: String, kind: &str, source: u8, items: &mut Vec<CItem>| {
            let l = text.to_lowercase();
            if l.starts_with(&low) && l != low && seen.insert(l) {
                items.push(CItem { text, kind: kind.to_string(), source });
            }
        };
        // definitions in the project
        if let Some(p) = &self.project {
            for sym in p.index.all() {
                let name = sym.def.name.split(' ').next().unwrap_or("").to_string();
                let k = match sym.def.kind {
                    outline_kind if outline_kind.badge() == "m" => "m",
                    outline_kind if outline_kind.badge() == "v" || outline_kind.badge() == "c" => "v",
                    _ => "f",
                };
                push(name, k, 1, &mut items);
            }
        }
        // identifiers in the current file
        let p = d.ed.parsed();
        for t in &p.tokens {
            if t.kind == TokenKind::Symbol && t.end != caret {
                push(t.text(d.ed.text()).to_string(), "s", 2, &mut items);
            }
        }
        for name in symbols::complete(&low) {
            push(name.to_string(), if symbols::is_keyword(name) { "m" } else { "f" }, 3, &mut items);
        }
        items.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.text.len().cmp(&b.text.len())).then_with(|| a.text.cmp(&b.text)));
        items.truncate(60);
        let rev = d.ed.revision();
        let started = self.completion.active && self.completion.start == s && self.completion.prefix == prefix;
        self.completion = Completion { active: !items.is_empty() || self.repl.ready, items, selected: if started { self.completion.selected } else { 0 }, start: s, prefix: prefix.clone() };
        if self.repl.ready && !prefix.is_empty() {
            let p2 = prefix.clone();
            self.repl_request("complete", &p2, Pending::Complete { start: s, prefix, doc_rev: rev });
        }
        if self.completion.items.is_empty() && !self.repl.ready {
            self.completion.active = false;
        }
        if self.completion.selected >= self.completion.items.len() {
            self.completion.selected = 0;
        }
    }

    pub fn accept_completion(&mut self) {
        if !self.completion.active {
            return;
        }
        let Some(item) = self.completion.items.get(self.completion.selected).cloned() else {
            self.completion.active = false;
            return;
        };
        let start = self.completion.start;
        if let Some(d) = self.docs.get_mut(self.active) {
            let caret = d.ed.caret();
            if start <= caret {
                d.ed.replace_range(start, caret, &item.text);
                d.view.scroll_to_caret = true;
            }
        }
        self.completion.active = false;
    }

    // ------------------------------------------------------------------ find bar

    pub fn open_find(&mut self, replace: bool) {
        if self.doc().is_none() {
            return;
        }
        self.find.open = true;
        self.find.replace_mode = replace;
        self.find.focus = true;
        if let Some(d) = self.doc()
            && d.ed.has_selection()
            && !d.ed.selected_text().contains('\n')
        {
            self.find.query = d.ed.selected_text().to_string();
        }
        self.find.built_for = None;
    }

    pub fn refresh_find(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let key = (self.active, d.ed.revision(), self.find.query.clone(), self.find.case_sensitive, self.find.whole_word);
        if self.find.built_for.as_ref() == Some(&key) {
            return;
        }
        self.find.matches = if self.find.open { search::find_in_text(d.ed.text(), &self.find.query, TextQuery { case_sensitive: self.find.case_sensitive, whole_word: self.find.whole_word }) } else { vec![] };
        let caret = d.ed.selection().0;
        self.find.current = if self.find.matches.is_empty() { None } else { Some(self.find.matches.iter().position(|m| m.0 >= caret).unwrap_or(0)) };
        self.find.built_for = Some(key);
    }

    pub fn find_step(&mut self, forward: bool) {
        self.refresh_find();
        if self.find.matches.is_empty() {
            if !self.find.open {
                self.open_find(false);
            }
            return;
        }
        let n = self.find.matches.len();
        let cur = self.find.current.unwrap_or(0);
        // when the current match is selected, move on; otherwise land on the nearest
        let selected_is_current = self.doc().is_some_and(|d| d.ed.selection() == self.find.matches[cur.min(n - 1)]);
        let next = if selected_is_current { if forward { (cur + 1) % n } else { (cur + n - 1) % n } } else { cur };
        self.find.current = Some(next);
        let (s, e) = self.find.matches[next];
        if let Some(d) = self.doc_mut() {
            d.ed.select_range(s, e);
            d.view.scroll_to_caret = true;
        }
    }

    pub fn replace_current(&mut self) {
        self.refresh_find();
        let Some(i) = self.find.current else { return };
        let Some(&(s, e)) = self.find.matches.get(i) else { return };
        let r = self.find.replace.clone();
        if let Some(d) = self.doc_mut() {
            d.ed.replace_range(s, e, &r);
        }
        self.find.built_for = None;
        self.refresh_find();
        self.find_step(true);
    }

    pub fn replace_all(&mut self) {
        self.refresh_find();
        let ms = self.find.matches.clone();
        if ms.is_empty() {
            return;
        }
        let r = self.find.replace.clone();
        if let Some(d) = self.doc_mut() {
            d.ed.replace_ranges(&ms, &r);
        }
        self.toast(format!("Replaced {} occurrence(s)", ms.len()), Sev::Weak);
        self.find.built_for = None;
    }

    // ------------------------------------------------------------------ palettes

    pub fn open_palette(&mut self, mode: PaletteMode) {
        let initial = if let Popup::Palette(p) = &self.popup { Some(p.query.clone()) } else { None };
        let fixed = None;
        self.popup = Popup::Palette(PaletteState { mode, query: initial.unwrap_or_default(), items: vec![], selected: 0, built_for: None, fixed, just_opened: true });
    }

    pub fn open_structure_palette(&mut self) {
        let Some(d) = self.doc() else { return };
        let defs = outline(d.ed.text(), d.ed.dialect());
        let items: Vec<Item> = defs
            .iter()
            .map(|x| Item { label: x.name.clone(), detail: format!("{}  line {}", x.kind.label(), x.line + 1), badge: x.kind.badge().into(), section: "", target: PTarget::Offset(x.name_start), hits: vec![], shortcut: String::new() })
            .collect();
        self.popup = Popup::Palette(PaletteState { mode: PaletteMode::Structure, query: String::new(), items: vec![], selected: 0, built_for: None, fixed: Some(items), just_opened: true });
    }

    fn rel(&self, p: &Path) -> String {
        match self.project_root() {
            Some(r) => workspace::relative(r, p),
            None => p.to_string_lossy().into_owned(),
        }
    }

    pub fn build_items(&self, mode: PaletteMode, query: &str, fixed: Option<&Vec<Item>>) -> Vec<Item> {
        let mut out: Vec<Item> = Vec::new();
        let files_items = |limit: usize, section: &'static str| -> Vec<Item> {
            let mut v = Vec::new();
            let all: Vec<PathBuf> = {
                let mut a: Vec<PathBuf> = self.recent_files.clone();
                if let Some(p) = &self.project {
                    for f in &p.files {
                        if !a.contains(f) {
                            a.push(f.clone());
                        }
                    }
                }
                a
            };
            if query.trim().is_empty() {
                for f in all.iter().take(limit) {
                    v.push(Item { label: f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), detail: self.rel(f), badge: "F".into(), section, target: PTarget::File(f.clone()), hits: vec![], shortcut: String::new() });
                }
            } else {
                let rels: Vec<String> = all.iter().map(|f| self.rel(f)).collect();
                for (i, m) in search::rank(query, &rels, |s| s.clone(), limit * 3).into_iter().take(limit) {
                    let name = all[i].file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let ni = rels[i].chars().count() - name.chars().count();
                    let hits: Vec<usize> = search::fuzzy_match(query, &name).map(|m| m.positions).unwrap_or_else(|| m.positions.iter().filter(|&&p| p >= ni).map(|p| p - ni).collect());
                    v.push(Item { label: name, detail: rels[i].clone(), badge: "F".into(), section, target: PTarget::File(all[i].clone()), hits, shortcut: String::new() });
                }
            }
            v
        };
        let symbol_items = |limit: usize, section: &'static str| -> Vec<Item> {
            let Some(p) = &self.project else { return vec![] };
            if query.trim().is_empty() {
                return p
                    .index
                    .all()
                    .into_iter()
                    .take(limit)
                    .map(|s| Item { label: s.def.name.clone(), detail: self.rel(&s.path), badge: s.def.kind.badge().into(), section, target: PTarget::Symbol { path: s.path.clone(), offset: s.def.name_start }, hits: vec![], shortcut: String::new() })
                    .collect();
            }
            p.index
                .search(query, limit)
                .into_iter()
                .map(|(s, m)| Item { label: s.def.name.clone(), detail: self.rel(&s.path), badge: s.def.kind.badge().into(), section, target: PTarget::Symbol { path: s.path.clone(), offset: s.def.name_start }, hits: m.positions, shortcut: String::new() })
                .collect()
        };
        let action_items = |limit: usize, section: &'static str| -> Vec<Item> {
            let list: Vec<&commands::Command> = commands::COMMANDS.iter().filter(|c| !c.title.is_empty() && !matches!(c.id, "edit.newline" | "edit.backspace" | "edit.delete" | "app.escape") && !c.id.starts_with("caret.") && !c.id.starts_with("select.") && !c.id.starts_with("scroll.")).collect();
            let mk = |c: &commands::Command, hits: Vec<usize>| Item { label: c.title.to_string(), detail: c.category.title().to_string(), badge: "A".into(), section, target: PTarget::Command(c.id.to_string()), hits, shortcut: c.shortcuts.first().map(|s| commands::display_shortcut(s)).unwrap_or_default() };
            if query.trim().is_empty() {
                return list.iter().take(limit).map(|c| mk(c, vec![])).collect();
            }
            search::rank(query, &list, |c| c.title.to_string(), limit).into_iter().map(|(i, m)| mk(list[i], m.positions)).collect()
        };
        match mode {
            PaletteMode::Everywhere => {
                out.extend(files_items(6, "Files"));
                out.extend(symbol_items(6, "Symbols"));
                out.extend(action_items(6, "Actions"));
            }
            PaletteMode::Files => out.extend(files_items(60, "")),
            PaletteMode::Symbols => out.extend(symbol_items(60, "")),
            PaletteMode::Actions => out.extend(action_items(60, "")),
            PaletteMode::Recent => {
                let q = query.to_lowercase();
                for f in &self.recent_files {
                    let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    if q.is_empty() || name.to_lowercase().contains(&q) {
                        out.push(Item { label: name, detail: self.rel(f), badge: "F".into(), section: "", target: PTarget::File(f.clone()), hits: vec![], shortcut: String::new() });
                    }
                }
            }
            PaletteMode::Structure | PaletteMode::Choose => {
                if let Some(f) = fixed {
                    if query.trim().is_empty() {
                        out.extend(f.iter().cloned());
                    } else {
                        for (i, m) in search::rank(query, f, |it| it.label.clone(), 100) {
                            let mut it = f[i].clone();
                            it.hits = m.positions;
                            out.push(it);
                        }
                    }
                }
            }
        }
        out
    }

    pub fn tick_palette(&mut self) {
        let Popup::Palette(p) = &self.popup else { return };
        let key = (p.mode, p.query.clone());
        if p.built_for.as_ref() == Some(&key) {
            return;
        }
        let items = self.build_items(p.mode, &p.query, p.fixed.as_ref());
        if let Popup::Palette(p) = &mut self.popup {
            p.items = items;
            p.selected = 0;
            p.built_for = Some(key);
        }
    }

    pub fn exit_clean(&mut self) {
        self.save_settings();
        if let Some(s) = self.repl.session.take() {
            s.kill();
        }
        self.stop_run();
        self.exit_now = true;
    }

}

fn live_len_gt1() -> bool {
    true
}
