//! Automation: drive the real UI from a JSON script (synthetic keyboard/mouse events), assert on
//! application state and take screenshots of the app window. Used for end-to-end tests.
//!
//! A script is a JSON array of steps, e.g.
//! `[{"op":"open_project","path":"..."}, {"op":"key","value":"Ctrl+Shift+A"}, {"op":"text","value":"save"},
//!   {"op":"assert_popup","value":"palette"}, {"op":"screenshot","path":"a.png"}]`

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Event, Pos2};
use serde_json::Value;

use crate::app::App;
use crate::input;
use crate::state::*;

pub struct Script {
    steps: VecDeque<Value>,
    pending: Vec<Event>,
    wait_frames: u32,
    wait_cond: Option<(Value, f64)>,
    shot: Option<(PathBuf, Arc<Mutex<Option<Arc<egui::ColorImage>>>>)>,
    pub failures: Vec<String>,
    pub log: Vec<String>,
    total: usize,
    done: bool,
    started: f64,
}

pub fn load(path: &Path) -> Result<Script, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("bad script JSON: {e}"))?;
    let arr = v.as_array().ok_or("script must be a JSON array")?;
    Ok(Script { total: arr.len(), steps: arr.iter().cloned().collect(), pending: vec![], wait_frames: 3, wait_cond: None, shot: None, failures: vec![], log: vec![], done: false, started: 0.0 })
}

pub fn save_png(path: &Path, img: &egui::ColorImage) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.width() as u32, img.height() as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    w.write_image_data(&bytes).map_err(|e| e.to_string())
}

impl App {
    pub fn script_inject(&mut self, raw: &mut egui::RawInput) {
        if let Some(s) = self.script.as_mut()
            && !s.pending.is_empty() {
                raw.events.append(&mut s.pending);
            }
    }

    fn cond_met(&self, cond: &Value) -> bool {
        match cond["what"].as_str().unwrap_or("") {
            "repl_ready" => self.repl.ready,
            "idle" => self.repl.busy.is_empty() && !self.run.running && self.compile.is_none() && self.repl.pending.is_empty(),
            "index" => self.project.as_ref().is_none_or(|p| p.index_rx.is_none()),
            "run_done" => !self.run.running && self.run.handle.is_none(),
            "compile_done" => self.compile.is_none(),
            "repl_contains" => {
                let t = cond["value"].as_str().unwrap_or("");
                self.repl.log.iter().any(|l| l.text.contains(t))
            }
            "completion" => self.completion.active && !self.completion.items.is_empty(),
            "git_ready" => self.git.repo.is_some() && self.git.head.is_some() && self.git.rx.is_none(),
            "git_dirty" => self.git.rx.is_none() && !self.git.changes.is_empty(),
            "git_settled" => self.git.rx.is_none(),
            "git_clean" => self.git.rx.is_none() && self.git.changes.is_empty(),
            "term_idle" => !self.term_busy(),
            "script_done" => self.term.script.is_none() && self.term.handle.is_none(),
            "term_contains" => {
                let t = cond["value"].as_str().unwrap_or("");
                self.term.log.iter().any(|l| l.text.contains(t))
            }
            "diff_new_contains" => {
                let t = cond["value"].as_str().unwrap_or("");
                self.docs.iter().filter_map(|d| d.diff.as_deref()).any(|d| d.new.contains(t))
            }
            "wsl_idle" => !self.wsl.busy(),
            // the running setup step has got at least this far (0.0..1.0)
            "wsl_progress" => {
                let min = cond["value"].as_f64().unwrap_or(0.0) as f32;
                self.wsl.task.as_ref().is_some_and(|t| t.tracker.progress().fraction.is_some_and(|f| f >= min))
            }
            "github_idle" => !self.github.busy(),
            "popup" => !matches!(self.popup, Popup::None),
            "all_saved" => self.docs.iter().all(|d| !d.ed.is_modified() || d.path.is_none()),
            "find_done" => self.find_rx.is_none(),
            "ask_open" => matches!(self.popup, Popup::OpenProject { .. }),
            "no_popup" => matches!(self.popup, Popup::None),
            _ => true,
        }
    }

    fn fail(&mut self, msg: String) {
        eprintln!("SCRIPT FAIL: {msg}");
        if let Some(s) = self.script.as_mut() {
            s.failures.push(msg);
        }
    }

    pub fn script_step(&mut self, ctx: &egui::Context) {
        let Some(mut s) = self.script.take() else { return };
        let now = ctx.input(|i| i.time);
        if s.started == 0.0 {
            s.started = now;
        }
        ctx.request_repaint();
        s.pending.clear();
        // finish a pending screenshot
        if let Some((path, slot)) = s.shot.take() {
            let img = slot.lock().unwrap().take();
            match img {
                Some(img) => {
                    if let Err(e) = save_png(&path, &img) {
                        s.failures.push(format!("screenshot {}: {e}", path.display()));
                    } else {
                        println!("screenshot saved: {}", path.display());
                    }
                }
                None => {
                    s.shot = Some((path, slot));
                    self.script = Some(s);
                    return;
                }
            }
        }
        if s.wait_frames > 0 {
            s.wait_frames -= 1;
            self.script = Some(s);
            return;
        }
        if let Some((cond, deadline)) = s.wait_cond.take() {
            if self.cond_met(&cond) {
                s.wait_frames = 2;
            } else if now > deadline {
                s.failures.push(format!("timeout waiting for {}", cond));
                eprintln!("SCRIPT FAIL: timeout waiting for {cond}");
            } else {
                s.wait_cond = Some((cond, deadline));
            }
            self.script = Some(s);
            return;
        }
        let Some(step) = s.steps.pop_front() else {
            if !s.done {
                s.done = true;
                let ok = s.failures.is_empty();
                println!("SCRIPT {}: {} steps, {} failure(s)", if ok { "OK" } else { "FAILED" }, s.total, s.failures.len());
                println!("max frame time: {:.0} ms", self.max_frame_ms);
                for f in &s.failures {
                    println!("  - {f}");
                }
                let code = if ok { 0 } else { 1 };
                std::process::exit(code);
            }
            self.script = Some(s);
            return;
        };
        let op = step["op"].as_str().unwrap_or("").to_string();
        let sval = step["value"].as_str().unwrap_or("").to_string();
        let path = step["path"].as_str().map(PathBuf::from);
        // put the script back before executing steps that need &mut self
        self.script = Some(s);
        match op.as_str() {
            "open_project" => {
                if let Some(p) = path {
                    self.open_project(&p);
                }
            }
            "open" => {
                if let Some(p) = path {
                    self.open_file(&p);
                }
            }
            "close_project" => self.close_project(),
            "project_env" => self.set_project_env(if sval == "windows" { None } else { Some(sval.clone()) }),
            "ask_open_project" => {
                if let Some(p) = path {
                    self.request_open_project(&p);
                }
            }
            "clone_set" => {
                if let Popup::Clone(c) = &mut self.popup {
                    if let Some(u) = step["value"].as_str() {
                        c.url = u.to_string();
                    }
                    if let Some(d) = step["dir"].as_str() {
                        c.parent = d.to_string();
                        c.dir_touched = true;
                    }
                    if let Some(b) = step["branch"].as_str() {
                        c.branch = b.to_string();
                    }
                    if let Some(b) = step["build_in_linux"].as_bool() {
                        c.build_in_linux = b;
                    }
                    if let Some(b) = step["linux"].as_bool() {
                        c.linux = b;
                    }
                } else {
                    self.fail("clone_set: the clone dialog is not open".to_string());
                }
            }
            "expand" => {
                if let (Some(p), Some(proj)) = (path, self.project.as_mut()) {
                    proj.expanded.insert(p);
                    proj.tree_ver += 1;
                }
            }
            "find_in_files" => self.run_find_in_files(&sval, ide_core::search::TextQuery::default()),
            "key" => {
                if let Some((key, mods)) = input::parse_key_combo(&sval) {
                    let s = self.script.as_mut().unwrap();
                    s.pending.push(Event::ModifiersChanged(mods));
                    s.pending.push(Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: mods });
                    s.pending.push(Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: mods });
                    s.pending.push(Event::ModifiersChanged(egui::Modifiers::NONE));
                } else {
                    self.fail(format!("bad key combo '{sval}'"));
                }
            }
            "text" => {
                let s = self.script.as_mut().unwrap();
                for ch in sval.chars() {
                    s.pending.push(Event::Text(ch.to_string()));
                }
            }
            "paste" => {
                let s = self.script.as_mut().unwrap();
                s.pending.push(Event::Paste(sval.clone()));
            }
            "command" => self.run_command(ctx, &sval),
            "frames" => {
                self.script.as_mut().unwrap().wait_frames = step["value"].as_u64().unwrap_or(1) as u32;
            }
            "wait" => {
                let timeout = step["timeout"].as_f64().unwrap_or(60.0);
                self.script.as_mut().unwrap().wait_cond = Some((step.clone(), now + timeout));
            }
            "set_text" => {
                if let Some(d) = self.doc_mut() {
                    let t = sval.replace("\\n", "\n");
                    d.ed.select_all();
                    d.ed.insert_text_raw(&t);
                    d.ed.set_caret(0, false);
                }
            }
            "caret" => {
                let line = step["line"].as_u64().unwrap_or(1) as usize;
                let col = step["col"].as_u64().unwrap_or(1) as usize;
                if let Some(d) = self.doc_mut() {
                    let o = d.ed.offset_of(line.saturating_sub(1), col.saturating_sub(1));
                    d.ed.set_caret(o, false);
                    d.view.scroll_to_caret = true;
                }
                self.editor_focus = true;
            }
            "focus_editor" => self.editor_focus = true,
            "click" => {
                let (x, y) = (step["x"].as_f64().unwrap_or(0.0) as f32, step["y"].as_f64().unwrap_or(0.0) as f32);
                let s = self.script.as_mut().unwrap();
                let pos = Pos2::new(x, y);
                s.pending.push(Event::PointerMoved(pos));
                s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
                s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
            }
            "screenshot" => {
                let slot: Arc<Mutex<Option<Arc<egui::ColorImage>>>> = Arc::new(Mutex::new(None));
                let s2 = slot.clone();
                ctx.request_screenshot(move |img| {
                    *s2.lock().unwrap() = Some(img);
                });
                if let Some(p) = path {
                    self.script.as_mut().unwrap().shot = Some((p, slot));
                }
            }
            "mods" => {
                let m = match sval.as_str() {
                    "Shift" => egui::Modifiers::SHIFT,
                    _ => egui::Modifiers::NONE,
                };
                self.script.as_mut().unwrap().pending.push(Event::ModifiersChanged(m));
            }
            "assert_find" => {
                let want = step["matches"].as_u64().unwrap_or(0) as usize;
                self.refresh_find();
                if self.find.matches.len() != want {
                    self.fail(format!("assert_find: expected {want} matches, got {}", self.find.matches.len()));
                }
            }
            "assert_clone_url" => match &self.popup {
                Popup::Clone(c) if c.url == sval => {}
                Popup::Clone(c) => self.fail(format!("assert_clone_url: expected {sval:?}, got {:?}", c.url)),
                _ => self.fail("assert_clone_url: the clone dialog is not open".to_string()),
            },
            "assert_graph" => {
                let lanes = self.git.graph.iter().map(|g| g.width).max().unwrap_or(0);
                let want_lanes = step["min_lanes"].as_u64().unwrap_or(1) as usize;
                let want_commits = step["min_commits"].as_u64().unwrap_or(1) as usize;
                if lanes < want_lanes || self.git.graph.len() < want_commits {
                    self.fail(format!("assert_graph: {} commits in {lanes} lanes, wanted at least {want_commits} in {want_lanes}", self.git.graph.len()));
                }
            }
            "assert_hint" => {
                let ok = self.hint.as_ref().is_some_and(|h| h.0.contains(&sval));
                if !ok {
                    self.fail(format!("assert_hint: {sval:?} not shown (hint={:?})", self.hint));
                }
            }
            "log" => println!("[script] {sval}"),
            "assert_text" => {
                let want = sval.replace("\\n", "\n");
                let got = self.doc().map(|d| d.ed.text().to_string()).unwrap_or_default();
                if got != want {
                    self.fail(format!("assert_text: expected {want:?}, got {got:?}"));
                }
            }
            "assert_text_contains" => {
                let got = self.doc().map(|d| d.ed.text().to_string()).unwrap_or_default();
                if !got.contains(&sval.replace("\\n", "\n")) {
                    self.fail(format!("assert_text_contains: {sval:?} not in {got:?}"));
                }
            }
            "assert_caret" => {
                let (l, c) = self.doc().map(|d| d.ed.line_col(d.ed.caret())).unwrap_or((0, 0));
                let (wl, wc) = (step["line"].as_u64().unwrap_or(1) as usize, step["col"].as_u64().unwrap_or(1) as usize);
                if (l + 1, c + 1) != (wl, wc) {
                    self.fail(format!("assert_caret: expected {wl}:{wc}, got {}:{}", l + 1, c + 1));
                }
            }
            "assert_selection" => {
                let got = self.doc().map(|d| d.ed.selected_text().to_string()).unwrap_or_default();
                if got != sval {
                    self.fail(format!("assert_selection: expected {sval:?}, got {got:?}"));
                }
            }
            "assert_repl_contains" => {
                let ok = self.repl.log.iter().any(|l| l.text.contains(&sval));
                if !ok {
                    let tail: Vec<String> = self.repl.log.iter().rev().take(8).map(|l| l.text.clone()).collect();
                    self.fail(format!("assert_repl_contains: {sval:?} not found; tail: {tail:?}"));
                }
            }
            "assert_run_contains" => {
                if !self.run.log.iter().any(|l| l.text.contains(&sval)) {
                    let tail: Vec<String> = self.run.log.iter().rev().take(8).map(|l| l.text.clone()).collect();
                    self.fail(format!("assert_run_contains: {sval:?} not found; tail: {tail:?}"));
                }
            }
            "assert_popup" => {
                let name = match &self.popup {
                    Popup::None => "none",
                    Popup::Palette(_) => "palette",
                    Popup::GotoLine { .. } => "goto_line",
                    Popup::Rename { .. } => "rename",
                    Popup::NewFile { .. } => "new_file",
                    Popup::NewProject { .. } => "new_project",
                    Popup::FindInFiles { .. } => "find_in_files",
                    Popup::Settings { .. } => "settings",
                    Popup::Clone(_) => "clone",
                    Popup::LocalView(_) => "local_view",
                    Popup::About => "about",
                    Popup::OpenProject { .. } => "open_project",
                    Popup::Info { .. } => "info",
                    Popup::Confirm { .. } => "confirm",
                    Popup::RenameFile { .. } => "rename_file",
                    Popup::NewBranch { .. } => "new_branch",
                    Popup::LocalCommit { .. } => "local_commit",
                };
                if name != sval {
                    self.fail(format!("assert_popup: expected {sval}, got {name}"));
                }
            }
            "assert_completion" => {
                let has = self.completion.items.iter().any(|c| c.text == sval);
                if !(self.completion.active && has) {
                    let names: Vec<_> = self.completion.items.iter().take(8).map(|c| c.text.clone()).collect();
                    let idx = self.project.as_ref().map_or(0, |p| p.index.symbol_count());
                    self.fail(format!("assert_completion: {sval:?} not offered (active={}, prefix={:?}, index_symbols={idx}, items={names:?})", self.completion.active, self.completion.prefix));
                }
            }
            "assert_no_completion" => {
                if self.completion.active {
                    self.fail("assert_no_completion: popup is active".into());
                }
            }
            "assert_tabs" => {
                let n = step["value"].as_u64().unwrap_or(0) as usize;
                if self.docs.len() != n {
                    self.fail(format!("assert_tabs: expected {n}, got {}", self.docs.len()));
                }
            }
            "assert_active_file" => {
                let got = self.doc().map(|d| d.name()).unwrap_or_default();
                if got != sval {
                    self.fail(format!("assert_active_file: expected {sval}, got {got}"));
                }
            }
            "assert_modified" => {
                let want = step["value"].as_bool().unwrap_or(true);
                let got = self.doc().is_some_and(|d| d.ed.is_modified());
                if got != want {
                    self.fail(format!("assert_modified: expected {want}, got {got}"));
                }
            }
            "assert_env" => {
                let got = match self.wsl_target() {
                    Some(t) => format!("linux:{}", t.distro),
                    None => "windows".to_string(),
                };
                if !sval.is_empty() && got != sval {
                    self.fail(format!("assert_env: expected {sval}, got {got}"));
                }
                if let Some(b) = step["banner"].as_bool()
                    && b != self.env_banner
                {
                    self.fail(format!("assert_env: banner expected {b}, got {}", self.env_banner));
                }
                if let Some(rc) = step["run_config"].as_str() {
                    let label = self.run_config_label();
                    if label != rc {
                        self.fail(format!("assert_env: run configuration expected {rc:?}, got {label:?}"));
                    }
                }
            }
            "assert_make_problem" => {
                let file = step["file"].as_str().unwrap_or("").replace('/', "\\");
                let line = step["line"].as_u64().unwrap_or(1) as usize;
                let found = self.compiler_problems.iter().any(|p| {
                    p.source == "make"
                        && p.path.as_ref().is_some_and(|f| f.to_string_lossy().replace('/', "\\").ends_with(&file) && f.exists())
                        && p.path.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).is_some_and(|t| t[..p.start.min(t.len())].matches('\n').count() + 1 == line)
                });
                if !found {
                    let rows: Vec<String> = self.compiler_problems.iter().map(|p| format!("{:?} @{} {}", p.path, p.start, p.message)).collect();
                    self.fail(format!("assert_make_problem: no make problem in {file} line {line}; have {rows:?}"));
                }
            }
            "assert_max_problems" => {
                let max = step["value"].as_u64().unwrap_or(0) as usize;
                let all = self.all_problems();
                if all.len() > max {
                    let rows: Vec<String> = all.iter().map(|p| format!("{:?}: {}", p.path, p.message)).collect();
                    self.fail(format!("assert_max_problems: expected <= {max}, got {rows:?}"));
                }
            }
            "assert_problems" => {
                let min = step["min"].as_u64().unwrap_or(1) as usize;
                let n = self.all_problems().len();
                if n < min {
                    self.fail(format!("assert_problems: expected >= {min}, got {n}"));
                }
            }
            "assert_file_contains" => {
                let got = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
                if !got.contains(&sval) {
                    self.fail(format!("assert_file_contains: {sval:?} not in {:?}", path));
                }
            }
            "assert_bottom" => {
                let name = match self.bottom_tool {
                    Some(BottomTool::Repl) => "repl",
                    Some(BottomTool::Run) => "run",
                    Some(BottomTool::Problems) => "problems",
                    Some(BottomTool::Find) => "find",
                    Some(BottomTool::Git) => "git",
                    Some(BottomTool::Terminal) => "terminal",
                    None => "none",
                };
                if name != sval {
                    self.fail(format!("assert_bottom: expected {sval}, got {name}"));
                }
            }
            "assert_left" => {
                let name = match self.left_tool {
                    Some(LeftTool::Project) => "project",
                    Some(LeftTool::Structure) => "structure",
                    Some(LeftTool::Commit) => "commit",
                    None => "none",
                };
                if name != sval {
                    self.fail(format!("assert_left: expected {sval}, got {name}"));
                }
            }
            "assert_find_results" => {
                let min = step["min"].as_u64().unwrap_or(1) as usize;
                if self.find_results.hits.len() < min {
                    self.fail(format!("assert_find_results: expected >= {min}, got {}", self.find_results.hits.len()));
                }
            }
            "assert_git_changes" => {
                let want = step["value"].as_u64().unwrap_or(0) as usize;
                if self.git.changes.len() != want {
                    let names: Vec<_> = self.git.changes.iter().map(|c| c.path.clone()).collect();
                    self.fail(format!("assert_git_changes: expected {want}, got {names:?}"));
                }
            }
            "assert_base" => {
                let want_local = sval == "local";
                if self.base_is_local() != want_local {
                    let l = self.base_label();
                    self.fail(format!("assert_base: expected {sval}, got {l}"));
                }
            }
            "assert_local_commits" => {
                let want = step["value"].as_u64().unwrap_or(0) as usize;
                let n = self.git.local.as_ref().map_or(0, |l| l.commits().len());
                if n != want {
                    self.fail(format!("assert_local_commits: expected {want}, got {n}"));
                }
            }
            "assert_ignored" => {
                let want = step["ignored"].as_bool().unwrap_or(true);
                let is_dir = step["dir"].as_bool().unwrap_or(false);
                let got = match self.project.as_mut() {
                    Some(p) => {
                        let abs = p.root.join(&sval);
                        p.ignore.is_ignored(&abs, is_dir)
                    }
                    None => false,
                };
                if got != want {
                    self.fail(format!("assert_ignored: {sval} expected ignored={want}, got {got}"));
                }
            }
            "assert_project_file" => {
                let want = step["present"].as_bool().unwrap_or(true);
                let got = self.project.as_ref().is_some_and(|p| p.files_rel.iter().any(|f| *f == sval));
                if got != want {
                    self.fail(format!("assert_project_file: {sval} expected present={want}, got {got}"));
                }
            }
            "assert_branch" => {
                let got = self.git.branch.clone().unwrap_or_default();
                if got != sval {
                    self.fail(format!("assert_branch: expected {sval:?}, got {got:?}"));
                }
            }
            "assert_git_log" => {
                if !self.git.log.iter().any(|c| c.subject.contains(&sval)) {
                    let all: Vec<_> = self.git.log.iter().map(|c| c.subject.clone()).collect();
                    self.fail(format!("assert_git_log: {sval:?} not in {all:?}"));
                }
            }
            "assert_marks" => {
                let min = step["min"].as_u64().unwrap_or(1) as usize;
                let n = self.docs.get_mut(self.active).map_or(0, |d| App::git_marks(d).len());
                if let Some(exact) = step["exact"].as_u64() {
                    if n != exact as usize {
                        self.fail(format!("assert_marks: expected exactly {exact} gutter marks, got {n}"));
                    }
                } else if n < min {
                    self.fail(format!("assert_marks: expected >= {min} gutter marks, got {n}"));
                }
            }
            "assert_folded" => {
                let want = step["value"].as_u64().unwrap_or(0) as usize;
                let n = self.doc().map_or(0, |d| d.folds.len());
                if n != want {
                    self.fail(format!("assert_folded: expected {want}, got {n}"));
                }
            }
            "assert_term_contains" => {
                if !self.term.log.iter().any(|l| l.text.contains(&sval)) {
                    let tail: Vec<String> = self.term.log.iter().rev().take(6).map(|l| l.text.clone()).collect();
                    self.fail(format!("assert_term_contains: {sval:?} not found; tail: {tail:?}"));
                }
            }
            "assert_no_term_contains" => {
                if self.term.log.iter().any(|l| l.text.contains(&sval)) {
                    self.fail(format!("assert_no_term_contains: {sval:?} was printed"));
                }
            }
            "assert_toast" => {
                if !self.toasts.iter().any(|t| t.text.contains(&sval)) {
                    let all: Vec<_> = self.toasts.iter().map(|t| t.text.clone()).collect();
                    self.fail(format!("assert_toast: {sval:?} not in {all:?}"));
                }
            }
            "language" => {
                let l = if sval.eq_ignore_ascii_case("ru") { ide_core::settings::Language::Russian } else { ide_core::settings::Language::English };
                self.settings.language = l;
                crate::i18n::set(l);
            }
            "open_local_commit" => self.open_local_view(step["value"].as_u64().unwrap_or(1)),
            "local_view" => {
                let p = std::mem::replace(&mut self.popup, Popup::None);
                if let Popup::LocalView(mut st) = p {
                    if let Some(id) = step["commit"].as_u64() {
                        self.local_view_goto(&mut st, id);
                    }
                    if let Some(f) = step["file"].as_str() {
                        match st.files.iter().position(|c| c.path == f) {
                            Some(i) => st.sel = i,
                            None => self.fail(format!("local_view: file {f} is not part of commit #{}", st.id)),
                        }
                    }
                    match step["mode"].as_str() {
                        Some("code") => st.mode = LocalViewMode::Code,
                        Some("changes") => st.mode = LocalViewMode::Changes,
                        _ => {}
                    }
                    self.popup = Popup::LocalView(st);
                } else {
                    self.popup = p;
                    self.fail("local_view: the commit browser is not open".to_string());
                }
            }
            "assert_local_view" => {
                let p = std::mem::replace(&mut self.popup, Popup::None);
                if let Popup::LocalView(mut st) = p {
                    self.local_view_load(&mut st);
                    if let Some(id) = step["commit"].as_u64()
                        && st.id != id
                    {
                        self.fail(format!("assert_local_view: expected commit #{id}, got #{}", st.id));
                    }
                    if let Some(n) = step["files"].as_u64()
                        && st.files.len() != n as usize
                    {
                        let names: Vec<_> = st.files.iter().map(|c| c.path.clone()).collect();
                        self.fail(format!("assert_local_view: expected {n} files, got {names:?}"));
                    }
                    if !sval.is_empty() {
                        let shown = match st.mode {
                            LocalViewMode::Code => st.code.join("\n"),
                            LocalViewMode::Changes => st.diff.iter().map(|l| format!("{}{}", match l.kind { ide_core::diff::LineKind::Added => "+", ide_core::diff::LineKind::Removed => "-", ide_core::diff::LineKind::Same => " " }, l.text)).collect::<Vec<_>>().join("\n"),
                        };
                        if !shown.contains(&sval.replace("\\n", "\n")) {
                            self.fail(format!("assert_local_view: {sval:?} not shown in {:?} view: {shown:?}", st.mode));
                        }
                    }
                    self.popup = Popup::LocalView(st);
                } else {
                    self.popup = p;
                    self.fail("assert_local_view: the commit browser is not open".to_string());
                }
            }
            "gutter_click" => {
                let line = step["line"].as_u64().unwrap_or(1).saturating_sub(1) as usize;
                match (self.gutter_rect, self.docs.get(self.active)) {
                    (Some(g), Some(d)) => {
                        let pos = crate::editor_view::gutter_mark_pos(g, d.view.scroll.y, self.settings.font_size, line);
                        let s = self.script.as_mut().unwrap();
                        s.pending.push(Event::PointerMoved(pos));
                        s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
                        s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
                    }
                    _ => self.fail("gutter_click: no editor on screen".to_string()),
                }
            }
            "assert_hunk_popup" => {
                if step["none"].as_bool().unwrap_or(false) {
                    if self.hunk_popup.is_some() {
                        self.fail("assert_hunk_popup: a popup is open but none was expected".to_string());
                    }
                } else {
                    let hp = self.hunk_popup.clone();
                    match &hp {
                        None => self.fail("assert_hunk_popup: no popup is open".to_string()),
                        Some(h) => {
                            let old = h.hunk.old_lines.join("\n");
                            if !sval.is_empty() && !old.contains(&sval) {
                                self.fail(format!("assert_hunk_popup: {sval:?} not in the old code {old:?}"));
                            }
                            if let Some(n) = step["old_lines"].as_u64()
                                && h.hunk.old_lines.len() != n as usize
                            {
                                self.fail(format!("assert_hunk_popup: expected {n} old lines, got {}", h.hunk.old_lines.len()));
                            }
                        }
                    }
                }
            }
            "hunk_revert" => self.hunk_revert(),
            "diff_edit_beside" => self.diff_edit_beside(),
            "clone_tab" => {
                if let Popup::Clone(c) = &mut self.popup {
                    c.tab = if sval == "github" { CloneTab::GitHub } else { CloneTab::Url };
                } else {
                    self.fail("clone_tab: the clone dialog is not open".to_string());
                }
            }
            "github_sign_in" => self.github_sign_in_token(&sval, step["remember"].as_bool().unwrap_or(false)),
            "github_sign_in_saved" => self.github_sign_in_saved(),
            "github_sign_in_browser" => self.github_sign_in_browser(),
            "github_cancel_browser" => self.github_cancel_browser(),
            "github_show_user" => self.github_show_user(&sval),
            "github_sign_out" => self.github_sign_out(),
            "github_filter" => {
                self.github.filter = sval.clone();
                if let Some(b) = step["forks"].as_bool() {
                    self.github.include_forks = b;
                }
                if let Some(b) = step["archived"].as_bool() {
                    self.github.include_archived = b;
                }
            }
            "github_select" => {
                let repo = self.github.repos.iter().find(|r| r.full_name == sval).cloned();
                match (repo, &mut self.popup) {
                    (Some(r), Popup::Clone(c)) => {
                        c.url = r.clone_url;
                        c.gh_selected = Some(r.full_name);
                    }
                    (None, _) => self.fail(format!("github_select: {sval} is not in the list")),
                    _ => self.fail("github_select: the clone dialog is not open".to_string()),
                }
            }
            "github_click" => {
                let hit = self.github.rects.iter().find(|(n, _)| *n == sval).map(|(_, r)| egui::pos2(r.center().x, r.center().y));
                match hit {
                    Some(pos) => {
                        let presses = if step["double"].as_bool().unwrap_or(false) { 2 } else { 1 };
                        let s = self.script.as_mut().unwrap();
                        s.pending.push(Event::PointerMoved(pos));
                        for _ in 0..presses {
                            s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
                            s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
                        }
                    }
                    None => self.fail(format!("github_click: {sval} is not a visible row")),
                }
            }
            "assert_github" => {
                let g = &self.github;
                let mut errors: Vec<String> = vec![];
                if let Some(b) = step["signed_in"].as_bool()
                    && g.signed_in() != b
                {
                    errors.push(format!("signed_in is {}, expected {b}", g.signed_in()));
                }
                if let Some(l) = step["login"].as_str() {
                    let shown = g.shown.as_ref().map(|a| a.login.as_str()).unwrap_or("");
                    if shown != l {
                        errors.push(format!("listing {shown:?}, expected {l:?}"));
                    }
                }
                if let Some(n) = step["repos"].as_u64()
                    && g.repos.len() != n as usize
                {
                    errors.push(format!("{} repositories, expected {n}", g.repos.len()));
                }
                if let Some(n) = step["visible"].as_u64()
                    && g.visible().len() != n as usize
                {
                    errors.push(format!("{} visible, expected {n}", g.visible().len()));
                }
                if let Some(t) = step["error_contains"].as_str() {
                    let e = g.error.as_ref().map(crate::app_github::error_text).unwrap_or_default();
                    if !e.contains(t) {
                        errors.push(format!("error {e:?} does not contain {t:?}"));
                    }
                }
                if let Some(t) = step["login_error_contains"].as_str() {
                    let e = g.login_error.as_ref().and_then(crate::app_github::login_error_text).unwrap_or_default();
                    if !e.contains(t) {
                        errors.push(format!("login error {e:?} does not contain {t:?}"));
                    }
                }
                if step["no_login_error"].as_bool().unwrap_or(false) && g.login_error.is_some() {
                    errors.push(format!("unexpected login error {:?}", g.login_error));
                }
                if let Some(b) = step["waiting"].as_bool()
                    && g.browser_cancel.is_some() != b
                {
                    errors.push(format!("waiting for the browser is {}, expected {b}", g.browser_cancel.is_some()));
                }
                if step["no_error"].as_bool().unwrap_or(false) && g.error.is_some() {
                    errors.push(format!("unexpected error {:?}", g.error));
                }
                if let Some(k) = step["listing"].as_str() {
                    let now = match &g.listing {
                        None => "none".to_string(),
                        Some(crate::app_github::Listing::Mine) => "mine".to_string(),
                        Some(crate::app_github::Listing::User(u)) => format!("user:{u}"),
                    };
                    if now != k {
                        errors.push(format!("listing {now}, expected {k}"));
                    }
                }
                if let Some(s) = step["selected"].as_str() {
                    let now = if let Popup::Clone(c) = &self.popup { c.gh_selected.clone().unwrap_or_default() } else { String::new() };
                    if now != s {
                        errors.push(format!("selected {now:?}, expected {s:?}"));
                    }
                }
                if !errors.is_empty() {
                    self.fail(format!("assert_github: {}", errors.join("; ")));
                }
            }
            "settings_page" => {
                if let Popup::Settings { page, .. } = &mut self.popup {
                    *page = step["value"].as_u64().unwrap_or(0) as usize;
                } else {
                    self.fail("settings_page: the settings dialog is not open".to_string());
                }
            }
            "wsl_action" => {
                let arg = step["arg"].as_str().unwrap_or("").to_string();
                match sval.as_str() {
                    "probe" => self.wsl_probe(),
                    "install_wsl" => self.wsl_install_wsl(),
                    "install_distro" => self.wsl_install_distro(&arg),
                    "install_file" => self.wsl_install_file(Path::new(&arg)),
                    "install_sbcl" => self.wsl_install_sbcl(&arg),
                    "use" => self.wsl_use(&arg),
                    "use_windows" => self.use_windows(),
                    "check_tools" => self.wsl_check_tools(&arg),
                    "install_tools" => self.wsl_install_tools(&arg),
                    "git_setup" => self.wsl_git_setup(&arg),
                    other => self.fail(format!("wsl_action: unknown action {other}")),
                }
            }
            "assert_wsl_progress" => {
                // the running step: its stage, how far it is, and that the markers of the scripts are not in the log
                let Some(t) = self.wsl.task.as_ref() else {
                    self.fail("assert_wsl_progress: no setup step is running".to_string());
                    return;
                };
                let p = t.tracker.progress();
                let mut errors = vec![];
                if let Some(s) = step["stage"].as_str()
                    && format!("{:?}", p.stage) != s
                {
                    errors.push(format!("stage {:?}, expected {s}", p.stage));
                }
                let f = p.fraction;
                if let Some(min) = step["min"].as_f64()
                    && !f.is_some_and(|f| f as f64 >= min)
                {
                    errors.push(format!("fraction {f:?}, expected >= {min}"));
                }
                if let Some(max) = step["max"].as_f64()
                    && !f.is_some_and(|f| f as f64 <= max)
                {
                    errors.push(format!("fraction {f:?}, expected <= {max}"));
                }
                if self.wsl.log.iter().any(|l| l.contains("@@airlisp")) {
                    errors.push("the log shows the progress markers".to_string());
                }
                if !errors.is_empty() {
                    self.fail(format!("assert_wsl_progress: {}", errors.join("; ")));
                }
            }
            "assert_wsl" => {
                let w = &self.wsl;
                let presence = match w.probe.as_ref().map(|p| &p.presence) {
                    None => "unknown",
                    Some(lisp_runtime::wsl::Presence::Unsupported) => "unsupported",
                    Some(lisp_runtime::wsl::Presence::NotInstalled) => "not_installed",
                    Some(lisp_runtime::wsl::Presence::Installed { .. }) => "installed",
                };
                let mut errors = vec![];
                if let Some(p) = step["presence"].as_str()
                    && p != presence
                {
                    errors.push(format!("presence {presence}, expected {p}"));
                }
                if let Some(n) = step["distros"].as_u64()
                    && w.distros().len() != n as usize
                {
                    errors.push(format!("distros {:?}, expected {n}", w.distros()));
                }
                if let Some(s) = step["selected"].as_str()
                    && w.selected != s
                {
                    errors.push(format!("selected {:?}, expected {s}", w.selected));
                }
                if let Some(s) = step["sbcl"].as_str() {
                    let found = w.sbcl_path(&w.selected).is_some();
                    if (s == "found") != found {
                        errors.push(format!("sbcl in {:?}: {:?}, expected {s}", w.selected, w.sbcl.get(&w.selected)));
                    }
                }
                if let Some(s) = step["tools"].as_str() {
                    let got = match w.tools.get(&w.selected) {
                        Some(Ok(v)) if v.iter().all(|(_, f)| *f) => "ready",
                        Some(Ok(_)) => "missing",
                        Some(Err(_)) => "error",
                        None => "unknown",
                    };
                    if got != s {
                        errors.push(format!("tools in {:?}: {got} {:?}, expected {s}", w.selected, w.tools.get(&w.selected)));
                    }
                }
                if let Some(e) = step["run_env"].as_str() {
                    let now = if self.settings.run_env == ide_core::settings::RunEnv::Wsl { "wsl" } else { "windows" };
                    if now != e {
                        errors.push(format!("run_env {now}, expected {e}"));
                    }
                }
                if let Some(t) = step["outcome_contains"].as_str() {
                    let o = format!("{:?}", w.outcome);
                    if !o.contains(t) {
                        errors.push(format!("outcome {o}, expected it to contain {t:?}"));
                    }
                }
                if !errors.is_empty() {
                    self.fail(format!("assert_wsl: {}", errors.join("; ")));
                }
            }
            "assert_split" => {
                let (layout, focus, other) = match self.split.as_ref() {
                    None => ("none", "", None),
                    Some(sp) => (if sp.vertical { "vertical" } else { "horizontal" }, if sp.second_focused { "second" } else { "first" }, self.split_other_index().map(|i| self.docs[i].name())),
                };
                if layout != sval {
                    self.fail(format!("assert_split: expected layout {sval}, got {layout}"));
                }
                if let Some(f) = step["focus"].as_str() {
                    if focus != f {
                        self.fail(format!("assert_split: expected the {f} pane focused, got {focus}"));
                    }
                }
                if let Some(o) = step["other"].as_str() {
                    if other.as_deref() != Some(o) {
                        self.fail(format!("assert_split: expected {o} in the other pane, got {other:?}"));
                    }
                }
            }
            "pane_click" => {
                let k = step["pane"].as_u64().unwrap_or(1).saturating_sub(1) as usize;
                match self.pane_rects.get(k).copied() {
                    Some(r) => {
                        let pos = egui::pos2(r.center().x, r.min.y + r.height() * 0.6);
                        let s = self.script.as_mut().unwrap();
                        s.pending.push(Event::PointerMoved(pos));
                        s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
                        s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
                    }
                    None => self.fail("pane_click: the editor area is not split".to_string()),
                }
            }
            "diff_open" => self.open_diff_tab(&sval),
            "commit_click" => {
                let hit = self.commit_rects.iter().find(|(p, _, _)| *p == sval).map(|(_, r, cb)| if step["checkbox"].as_bool().unwrap_or(false) { cb.center() } else { egui::pos2(r.min.x + r.width() * 0.6, r.center().y) });
                match hit {
                    Some(pos) => {
                        let s = self.script.as_mut().unwrap();
                        s.pending.push(Event::PointerMoved(pos));
                        s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
                        s.pending.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
                    }
                    None => self.fail(format!("commit_click: {sval} is not a visible row of the Commit window")),
                }
            }
            "assert_commit_rows" => {
                let n = step["value"].as_u64().unwrap_or(0) as usize;
                if self.commit_rects.len() != n {
                    let names: Vec<_> = self.commit_rects.iter().map(|r| r.0.clone()).collect();
                    self.fail(format!("assert_commit_rows: expected {n} visible files, got {names:?}"));
                }
            }
            "assert_checked" => {
                let n = step["value"].as_u64().unwrap_or(0) as usize;
                if self.git.checked.len() != n {
                    self.fail(format!("assert_checked: expected {n} checked files, got {:?}", self.git.checked));
                }
                if let Some(p) = step["path"].as_str()
                    && self.git.checked.contains(p) != step["on"].as_bool().unwrap_or(true)
                {
                    self.fail(format!("assert_checked: {p} checked state is not {}", step["on"].as_bool().unwrap_or(true)));
                }
            }
            "assert_diff" => {
                let t = self.active_diff().map(|t| (t.rel.clone(), t.hunk_count(), t.cur, t.unified, t.old.clone(), t.new.clone(), t.sbs.added, t.sbs.removed));
                match t {
                    None => self.fail("assert_diff: the active tab is not a diff tab".to_string()),
                    Some((rel, hunks, cur, unified, old, new, added, removed)) => {
                        if let Some(f) = step["file"].as_str() {
                            if rel != f {
                                self.fail(format!("assert_diff: expected file {f}, got {rel}"));
                            }
                        }
                        if let Some(n) = step["hunks"].as_u64() {
                            if hunks != n as usize {
                                self.fail(format!("assert_diff: expected {n} differences, got {hunks}"));
                            }
                        }
                        if let Some(n) = step["current"].as_u64() {
                            if cur != Some(n as usize) {
                                self.fail(format!("assert_diff: expected current difference {n}, got {cur:?}"));
                            }
                        }
                        if let Some(m) = step["mode"].as_str() {
                            if unified != (m == "unified") {
                                self.fail(format!("assert_diff: expected mode {m}, unified={unified}"));
                            }
                        }
                        if let Some(n) = step["added"].as_u64() {
                            if added != n as usize {
                                self.fail(format!("assert_diff: expected {n} added lines, got {added}"));
                            }
                        }
                        if let Some(n) = step["removed"].as_u64() {
                            if removed != n as usize {
                                self.fail(format!("assert_diff: expected {n} removed lines, got {removed}"));
                            }
                        }
                        if let Some(s) = step["old_contains"].as_str() {
                            if !old.contains(&s.replace("\n", "
")) {
                                self.fail(format!("assert_diff: old text does not contain {s:?}: {old:?}"));
                            }
                        }
                        if let Some(s) = step["new_contains"].as_str() {
                            if !new.contains(&s.replace("\n", "
")) {
                                self.fail(format!("assert_diff: new text does not contain {s:?}: {new:?}"));
                            }
                        }
                    }
                }
            }
            "diff_mode" => {
                if let Some(t) = self.docs.get_mut(self.active).and_then(|d| d.diff.as_mut()) {
                    t.unified = sval == "unified";
                }
            }
            "local_restore" => {
                let id = step["value"].as_u64().unwrap_or(1);
                match step["file"].as_str() {
                    Some(f) => self.local_restore_file(id, f),
                    None => self.local_restore(id),
                }
            }
            "quit" => {
                let s = self.script.as_mut().unwrap();
                s.steps.clear();
            }
            other => self.fail(format!("unknown op '{other}'")),
        }
        if let Some(s) = self.script.as_mut() {
            // give the UI a couple of frames to react to the step
            if s.wait_frames == 0 {
                s.wait_frames = 2;
            }
        }
    }
}
