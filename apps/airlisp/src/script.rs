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
            "term_idle" => self.term.handle.is_none(),
            "term_contains" => {
                let t = cond["value"].as_str().unwrap_or("");
                self.term.log.iter().any(|l| l.text.contains(t))
            }
            "popup" => !matches!(self.popup, Popup::None),
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
                    Popup::About => "about",
                    Popup::Info { .. } => "info",
                    Popup::Confirm { .. } => "confirm",
                    Popup::RenameFile { .. } => "rename_file",
                    Popup::Commit { .. } => "commit",
                    Popup::Diff { .. } => "diff",
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
            "assert_toast" => {
                if !self.toasts.iter().any(|t| t.text.contains(&sval)) {
                    let all: Vec<_> = self.toasts.iter().map(|t| t.text.clone()).collect();
                    self.fail(format!("assert_toast: {sval:?} not in {all:?}"));
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
