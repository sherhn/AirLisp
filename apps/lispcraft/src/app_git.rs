//! Git behaviour of the application: background status refresh, gutter bases, operations.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use ide_core::diff::{self, LineKind};
use ide_core::git::{self, ChangeKind};

use crate::app::App;
use crate::editor_view::Sev;
use crate::state::*;

impl App {
    /// (Re)discover the repository of the current project.
    pub fn git_attach(&mut self) {
        self.git = GitState::default();
        self.git.available = git::git_available();
        if !self.git.available {
            return;
        }
        if let Some(root) = self.project_root().map(Path::to_path_buf) {
            self.git.repo = git::find_repo(&root);
        }
        self.git_refresh();
    }

    pub fn git_refresh(&mut self) {
        let Some(repo) = self.git.repo.clone() else { return };
        if self.git.rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.git.rx = Some(rx);
        self.git.last_refresh = self.now;
        std::thread::spawn(move || {
            let snap = GitSnapshot { branch: repo.branch(), head: repo.head_hash(), changes: repo.status(), branches: repo.branches(), log: repo.log(80) };
            let _ = tx.send(snap);
        });
    }

    pub fn git_poll(&mut self) {
        if let Some(rx) = &self.git.rx
            && let Ok(snap) = rx.try_recv()
        {
            self.git.rx = None;
            let head_changed = snap.head != self.git.head;
            self.git.branch = snap.branch;
            self.git.head = snap.head;
            self.git.branches = snap.branches;
            self.git.log = snap.log;
            // keep the user's check marks for files that are still changed; check new ones by default
            let old: HashSet<String> = self.git.changes.iter().map(|c| c.path.clone()).collect();
            let now: HashSet<String> = snap.changes.iter().map(|c| c.path.clone()).collect();
            self.git.checked.retain(|p| now.contains(p));
            for c in &snap.changes {
                if !old.contains(&c.path) {
                    self.git.checked.insert(c.path.clone());
                }
            }
            self.git.changes = snap.changes;
            self.git_rebuild_kinds();
            if head_changed {
                for d in self.docs.iter_mut() {
                    d.git_base = None;
                }
            }
        }
        if let Some(rx) = &self.git.op_rx
            && let Ok((what, res)) = rx.try_recv()
        {
            self.git.op_rx = None;
            self.git.busy = None;
            match res {
                Ok(msg) => {
                    let text = if msg.is_empty() { format!("{what}: done") } else { format!("{what}: {}", msg.lines().last().unwrap_or("")) };
                    self.toast(text, Sev::Weak);
                }
                Err(e) => self.toast(format!("{what} failed: {}", e.lines().next().unwrap_or("")), Sev::Error),
            }
            self.git_refresh();
            self.refresh_project();
            for d in self.docs.iter_mut() {
                d.sync_with_disk();
            }
        }
        // periodic refresh while idle
        if self.git.repo.is_some() && self.git.rx.is_none() && self.now - self.git.last_refresh > 4.0 {
            self.git_refresh();
        }
    }

    fn git_rebuild_kinds(&mut self) {
        let Some(repo) = &self.git.repo else { return };
        let mut kinds: HashMap<PathBuf, ChangeKind> = HashMap::new();
        let mut dirs: HashSet<PathBuf> = HashSet::new();
        for c in &self.git.changes {
            let abs = repo.root.join(&c.path);
            let mut p = abs.parent();
            while let Some(d) = p {
                if !dirs.insert(d.to_path_buf()) {
                    break;
                }
                if d == repo.root {
                    break;
                }
                p = d.parent();
            }
            kinds.insert(abs, c.kind);
        }
        self.git.kinds = kinds;
        self.git.dirty_dirs = dirs;
    }

    /// Make sure every open document knows its HEAD content (for gutter markers).
    pub fn git_update_bases(&mut self) {
        let Some(repo) = self.git.repo.clone() else { return };
        for d in self.docs.iter_mut() {
            if d.git_base.is_some() {
                continue;
            }
            let Some(p) = &d.path else { continue };
            d.git_base = Some(repo.rel(p).and_then(|rel| repo.head_content(&rel)).map(|t| t.replace("\r\n", "\n")));
            d.git_marks_rev = u64::MAX;
        }
    }

    pub fn git_change_of(&self, path: &Path) -> Option<ChangeKind> {
        self.git.kinds.get(path).copied()
    }

    pub fn git_run_sync(&mut self, what: &str, f: impl FnOnce(&git::Repo) -> Result<String, String>) -> bool {
        let Some(repo) = self.git.repo.clone() else {
            self.toast("This project is not a Git repository", Sev::Weak);
            return false;
        };
        match f(&repo) {
            Ok(msg) => {
                if !msg.is_empty() {
                    self.toast(format!("{what}: {msg}"), Sev::Weak);
                }
                self.git.rx = None;
                self.git_refresh();
                self.refresh_project();
                true
            }
            Err(e) => {
                self.toast(format!("{what} failed: {}", e.lines().next().unwrap_or("")), Sev::Error);
                false
            }
        }
    }

    pub fn git_run_async(&mut self, what: &str, f: impl FnOnce(git::Repo) -> Result<String, String> + Send + 'static) {
        let Some(repo) = self.git.repo.clone() else {
            self.toast("This project is not a Git repository", Sev::Weak);
            return;
        };
        if self.git.op_rx.is_some() {
            self.toast("Another Git operation is still running", Sev::Weak);
            return;
        }
        let (tx, rx) = mpsc::channel();
        let name = what.to_string();
        self.git.busy = Some(what.to_string());
        self.git.op_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send((name, f(repo)));
        });
    }

    pub fn git_checkout(&mut self, branch: &str) {
        // unsaved edits would be lost on a file-changing checkout; save first
        self.save_all();
        let b = branch.trim_start_matches("origin/").to_string();
        let target = if self.git.branches.iter().any(|x| x.name == branch && !x.remote) { branch.to_string() } else { b };
        if self.git_run_sync("Checkout", move |r| r.checkout(&target).map(|_| String::new())) {
            for d in self.docs.iter_mut() {
                d.sync_with_disk();
                d.git_base = None;
            }
            self.toast(format!("Switched to {branch}"), Sev::Weak);
        }
    }

    pub fn git_commit_selected(&mut self) -> bool {
        self.save_all();
        let msg = self.git.commit_msg.clone();
        let paths: Vec<String> = self.git.changes.iter().filter(|c| self.git.checked.contains(&c.path)).map(|c| c.path.clone()).collect();
        let ok = self.git_run_sync("Commit", move |r| r.commit(&msg, &paths).map(|h| format!("committed {h}")));
        if ok {
            self.git.commit_msg.clear();
            for d in self.docs.iter_mut() {
                d.git_base = None;
            }
        }
        ok
    }

    pub fn git_show_diff(&mut self, rel: &str) {
        let Some(repo) = self.git.repo.clone() else { return };
        let abs = repo.root.join(rel);
        let new = self.docs.iter().find(|d| d.path.as_deref() == Some(abs.as_path())).map(|d| d.ed.text().to_string()).or_else(|| std::fs::read_to_string(&abs).ok()).unwrap_or_default().replace("\r\n", "\n");
        let old = repo.head_content(rel).unwrap_or_default().replace("\r\n", "\n");
        let lines = diff::diff_lines(&old, &new);
        let first = lines.iter().position(|l| l.kind != LineKind::Same);
        self.popup = Popup::Diff { title: rel.to_string(), lines, scroll_to: first };
    }

    pub fn git_show_commit(&mut self, idx: usize) {
        let Some(repo) = self.git.repo.clone() else { return };
        let Some(c) = self.git.log.get(idx) else { return };
        let title = format!("{}  {}", c.short, c.subject);
        let text = repo.show(&c.hash).unwrap_or_else(|e| e);
        self.popup = Popup::Info { title, text };
    }

    /// Diff lines of an open document against HEAD (for the gutter).
    pub fn git_marks(doc: &mut crate::doc::Doc) -> &[diff::Mark] {
        if doc.git_marks_rev != doc.ed.revision() {
            doc.git_marks = match &doc.git_base {
                Some(Some(base)) => diff::gutter_marks(base, doc.ed.text()),
                Some(None) => {
                    // untracked file: every line is new
                    let n = doc.ed.line_count();
                    (0..n).map(|l| diff::Mark { line: l, kind: diff::MarkKind::Added }).collect()
                }
                None => vec![],
            };
            doc.git_marks_rev = doc.ed.revision();
        }
        &doc.git_marks
    }

    pub fn git_dirty_count(&self) -> usize {
        self.git.changes.len()
    }

}


impl App {
    /// Path of the active document relative to the repository root.
    pub fn current_rel_path(&self) -> Option<String> {
        let repo = self.git.repo.as_ref()?;
        let p = self.doc()?.path.as_ref()?;
        repo.rel(p)
    }

    pub fn open_branch_palette(&mut self) {
        if self.git.repo.is_none() {
            self.toast("This project is not a Git repository", Sev::Weak);
            return;
        }
        let items: Vec<Item> = self
            .git
            .branches
            .iter()
            .map(|b| Item {
                label: b.name.clone(),
                detail: if b.current { "current".into() } else if b.remote { "remote".into() } else { "local".into() },
                badge: "B".into(),
                section: "",
                target: PTarget::Branch(b.name.clone()),
                hits: vec![],
                shortcut: String::new(),
            })
            .collect();
        self.popup = Popup::Palette(PaletteState { mode: PaletteMode::Choose, query: String::new(), items: items.clone(), selected: 0, built_for: None, fixed: Some(items), just_opened: true });
    }

    pub fn fold_command(&mut self, id: &str) {
        let Some(d) = self.docs.get_mut(self.active) else { return };
        let regions = crate::folding::regions(&d.ed);
        let line = d.ed.line_col(d.ed.caret()).0;
        match id {
            "edit.fold" => {
                if let Some(r) = crate::folding::region_at(&regions, line) {
                    let key = r.key.clone();
                    let start = r.start_line;
                    d.folds.insert(key);
                    // keep the caret visible: move it to the end of the fold's first line
                    let ls = d.ed.line_start(start);
                    let le = ls + d.ed.line_text(start).len();
                    if d.ed.caret() > le {
                        d.ed.set_caret(le, false);
                    }
                }
            }
            "edit.unfold" => {
                if let Some(r) = crate::folding::hidden_by(&regions, &d.folds, line).or_else(|| crate::folding::region_at(&regions, line)) {
                    let key = r.key.clone();
                    d.folds.remove(&key);
                }
            }
            "edit.fold_all" => {
                for r in &regions {
                    d.folds.insert(r.key.clone());
                }
                let visible = crate::folding::visible_lines(d.ed.line_count(), &regions, &d.folds);
                if !visible.contains(&line) {
                    let nl = visible.iter().rev().find(|&&l| l <= line).copied().unwrap_or(0);
                    let o = d.ed.line_start(nl);
                    d.ed.set_caret(o, false);
                }
            }
            _ => d.folds.clear(),
        }
        d.view.scroll_to_caret = true;
    }
}
