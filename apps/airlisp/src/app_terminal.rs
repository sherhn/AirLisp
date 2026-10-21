//! A simple line-oriented terminal: each command runs in the platform shell with streamed output.
//! `cd` is handled internally so the working directory persists between commands.

use std::path::{Path, PathBuf};
use std::process::Command;

use lisp_runtime::runner::{self, RunEvent, RunHandle};

use crate::app::App;
use crate::state::*;

pub struct TerminalState {
    pub cwd: PathBuf,
    pub log: Vec<LogLine>,
    pub input: String,
    pub history: Vec<String>,
    pub hist_pos: Option<usize>,
    pub handle: Option<RunHandle>,
    pub focus_input: bool,
}

impl Default for TerminalState {
    fn default() -> Self {
        TerminalState { cwd: PathBuf::from("."), log: vec![], input: String::new(), history: vec![], hist_pos: None, handle: None, focus_input: false }
    }
}

fn shell_command(line: &str, cwd: &Path) -> Command {
    #[cfg(windows)]
    let mut c = {
        let mut c = Command::new("cmd");
        c.args(["/D", "/Q", "/C", line]);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = Command::new("sh");
        c.args(["-c", line]);
        c
    };
    c.current_dir(cwd);
    c
}

/// Resolve `cd <arg>` relative to `cwd`.
pub fn change_dir(cwd: &Path, arg: &str) -> Result<PathBuf, String> {
    let arg = arg.trim().trim_matches('"');
    let target = if arg.is_empty() || arg == "~" {
        std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).ok_or("no home directory")?
    } else {
        let p = Path::new(arg);
        if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) }
    };
    let t = target.canonicalize().map_err(|_| format!("The system cannot find the path specified: {arg}"))?;
    if !t.is_dir() {
        return Err(format!("Not a directory: {arg}"));
    }
    // avoid the verbatim `\\?\` prefix on Windows
    let s = t.to_string_lossy();
    Ok(PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()))
}

impl App {
    pub fn terminal_start_dir(&mut self) {
        if self.term.cwd == Path::new(".")
            && let Some(r) = self.project_root()
        {
            self.term.cwd = r.to_path_buf();
        }
        if self.term.log.is_empty() {
            self.term.log.push(LogLine { kind: LogKind::Info, text: "Terminal - commands run in your system shell. 'cd' changes the working directory.".into() });
        }
    }

    pub fn terminal_run(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        self.terminal_start_dir();
        self.term.log.push(LogLine { kind: LogKind::Input, text: format!("{}> {line}", self.term.cwd.display()) });
        if self.term.history.last().map(String::as_str) != Some(line) {
            self.term.history.push(line.to_string());
        }
        self.term.hist_pos = None;
        let lower = line.to_lowercase();
        if lower == "clear" || lower == "cls" {
            self.term.log.clear();
            return;
        }
        if lower == "cd" || lower.starts_with("cd ") || lower.starts_with("cd\t") {
            match change_dir(&self.term.cwd, &line[2..]) {
                Ok(p) => self.term.cwd = p,
                Err(e) => self.term.log.push(LogLine { kind: LogKind::Error, text: e }),
            }
            return;
        }
        if self.term.handle.is_some() {
            self.term.log.push(LogLine { kind: LogKind::Error, text: "A command is still running (press Stop).".into() });
            return;
        }
        match runner::spawn_streaming(shell_command(line, &self.term.cwd)) {
            Ok(h) => self.term.handle = Some(h),
            Err(e) => self.term.log.push(LogLine { kind: LogKind::Error, text: format!("cannot run: {e}") }),
        }
    }

    pub fn terminal_stop(&mut self) {
        if let Some(h) = self.term.handle.take() {
            h.kill();
            self.term.log.push(LogLine { kind: LogKind::Info, text: "Process terminated".into() });
        }
    }

    pub fn poll_terminal(&mut self) {
        let mut done = false;
        let mut lines: Vec<(String, bool)> = vec![];
        if let Some(h) = &self.term.handle {
            while let Some(ev) = h.try_recv() {
                match ev {
                    RunEvent::Output { text, is_err } => lines.push((text, is_err)),
                    RunEvent::Exited(code) => {
                        done = true;
                        if code != Some(0) {
                            lines.push((format!("[exit code {}]\n", code.map_or("?".into(), |c| c.to_string())), true));
                        }
                    }
                }
            }
        }
        for (text, is_err) in lines {
            let text = text.replace('\r', "");
            let kind = if is_err { LogKind::Error } else { LogKind::Output };
            for seg in text.split_inclusive('\n') {
                match self.term.log.last_mut() {
                    Some(l) if l.kind == kind && !l.text.ends_with('\n') => l.text.push_str(seg),
                    _ => self.term.log.push(LogLine { kind, text: seg.to_string() }),
                }
            }
        }
        if done {
            self.term.handle = None;
            self.git.last_refresh = -100.0; // commands may have changed the repository
        }
        if self.term.log.len() > 5000 {
            self.term.log.drain(..1000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cd_resolves_relative_and_rejects_missing() {
        let base = std::env::temp_dir().join(format!("airlisp-term-{}", std::process::id()));
        std::fs::create_dir_all(base.join("sub")).unwrap();
        let base = change_dir(&base, ".").unwrap();
        let sub = change_dir(&base, "sub").unwrap();
        assert!(sub.ends_with("sub"));
        assert_eq!(change_dir(&sub, "..").unwrap(), base);
        assert!(change_dir(&base, "does-not-exist").is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn runs_a_command_and_captures_output() {
        let mut cmd = shell_command("echo airlisp-terminal-test", Path::new("."));
        let out = cmd.output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("airlisp-terminal-test"));
    }
}
