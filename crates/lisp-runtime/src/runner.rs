//! Running files and one-off expressions as separate processes with streamed output.

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::discover::Implementation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEvent {
    Output { text: String, is_err: bool },
    Exited(Option<i32>),
}

pub struct RunHandle {
    rx: Receiver<RunEvent>,
    child: Arc<Mutex<Option<Child>>>,
}

impl RunHandle {
    pub fn try_recv(&self) -> Option<RunEvent> {
        self.rx.try_recv().ok()
    }

    pub fn recv_timeout(&self, d: Duration) -> Option<RunEvent> {
        self.rx.recv_timeout(d).ok()
    }

    pub fn is_running(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }

    pub fn kill(&self) {
        if let Some(mut c) = self.child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    /// Block until the process exits; returns (merged output, exit code). `None` code means killed/timeout.
    pub fn wait_output(&self, timeout: Duration) -> (String, Option<i32>, bool) {
        let deadline = Instant::now() + timeout;
        let mut out = String::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                self.kill();
                return (out, None, true);
            }
            match self.recv_timeout(left) {
                Some(RunEvent::Output { text, .. }) => out.push_str(&text),
                Some(RunEvent::Exited(code)) => {
                    // drain anything still queued
                    while let Some(RunEvent::Output { text, .. }) = self.try_recv() {
                        out.push_str(&text);
                    }
                    return (out, code, false);
                }
                None => {}
            }
        }
    }
}

impl Drop for RunHandle {
    fn drop(&mut self) {
        self.kill();
    }
}

pub fn spawn_streaming(mut cmd: Command) -> io::Result<RunHandle> {
    crate::no_window(&mut cmd);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    let (tx, rx) = mpsc::channel();
    let mut readers = Vec::new();
    for (mut stream, is_err) in [(Box::new(stdout) as Box<dyn Read + Send>, false), (Box::new(stderr) as Box<dyn Read + Send>, true)] {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut tail: Vec<u8> = Vec::new();
            loop {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        tail.extend_from_slice(&buf[..n]);
                        let valid = match std::str::from_utf8(&tail) {
                            Ok(_) => tail.len(),
                            Err(e) if e.error_len().is_none() => e.valid_up_to(),
                            Err(_) => tail.len(),
                        };
                        let rest = tail.split_off(valid);
                        let text = String::from_utf8_lossy(&tail).into_owned();
                        tail = rest;
                        if !text.is_empty() {
                            let _ = tx.send(RunEvent::Output { text, is_err });
                        }
                    }
                }
            }
            if !tail.is_empty() {
                let _ = tx.send(RunEvent::Output { text: String::from_utf8_lossy(&tail).into_owned(), is_err });
            }
        }));
    }
    let child = Arc::new(Mutex::new(Some(child)));
    let waiter = child.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(30));
            let mut g = waiter.lock().unwrap();
            match g.as_mut() {
                None => return,
                Some(c) => {
                    if let Ok(Some(status)) = c.try_wait() {
                        *g = None;
                        drop(g);
                        for r in readers {
                            let _ = r.join();
                        }
                        let _ = tx.send(RunEvent::Exited(status.code()));
                        return;
                    }
                }
            }
        }
    });
    Ok(RunHandle { rx, child })
}

/// Lisp string literal for `s`.
pub fn lisp_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        if c == '"' || c == '\\' {
            o.push('\\');
        }
        o.push(c);
    }
    o.push('"');
    o
}

pub fn lisp_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn base_command(imp: &Implementation, cwd: Option<&Path>) -> Command {
    let mut cmd = Command::new(&imp.path);
    cmd.args(["--noinform", "--non-interactive", "--no-userinit", "--no-sysinit"]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd
}

/// `sbcl --load FILE`; the working directory defaults to the file's folder.
pub fn run_file(imp: &Implementation, file: &Path, cwd: Option<&Path>) -> io::Result<RunHandle> {
    let dir = cwd.or_else(|| file.parent().filter(|p| !p.as_os_str().is_empty()));
    let mut cmd = base_command(imp, dir);
    cmd.arg("--load").arg(file);
    spawn_streaming(cmd)
}

/// Evaluate forms in a fresh process.
pub fn run_eval(imp: &Implementation, forms: &str, cwd: Option<&Path>) -> io::Result<RunHandle> {
    let mut cmd = base_command(imp, cwd);
    cmd.arg("--eval").arg(forms);
    spawn_streaming(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lisp_string_escaping() {
        assert_eq!(lisp_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(lisp_path(Path::new(r"C:\a\b.lisp")), "C:/a/b.lisp");
    }

    fn sbcl() -> Option<Implementation> {
        crate::discover::find_sbcl("")
    }

    fn tmp(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lispcraft-runner-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn runs_a_file_and_streams_output() {
        let Some(imp) = sbcl() else { return };
        let f = tmp("hello.lisp", "(format t \"hello ~a~%\" (+ 1 2))\n(format t \"Привет, мир!~%\")\n(sb-ext:exit :code 5)\n");
        let h = run_file(&imp, &f, None).unwrap();
        let (out, code, timed_out) = h.wait_output(Duration::from_secs(30));
        assert!(!timed_out);
        assert_eq!(code, Some(5));
        assert!(out.contains("hello 3"), "{out:?}");
        assert!(out.contains("Привет, мир!"), "unicode output: {out:?}");
        assert!(!h.is_running());
    }

    #[test]
    fn reports_runtime_errors_with_nonzero_exit() {
        let Some(imp) = sbcl() else { return };
        let f = tmp("boom.lisp", "(error \"boom ~a\" 1)\n");
        let (out, code, _) = run_file(&imp, &f, None).unwrap().wait_output(Duration::from_secs(30));
        assert_ne!(code, Some(0));
        assert!(out.contains("boom 1"), "{out:?}");
    }

    #[test]
    fn can_kill_a_running_process() {
        let Some(imp) = sbcl() else { return };
        let f = tmp("loop.lisp", "(loop (sleep 1))\n");
        let h = run_file(&imp, &f, None).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert!(h.is_running());
        h.kill();
        assert!(!h.is_running());
    }

    #[test]
    fn reads_script_relative_to_its_directory() {
        let Some(imp) = sbcl() else { return };
        let _ = tmp("data.txt", "payload-123");
        let f = tmp("rel.lisp", "(with-open-file (s \"data.txt\") (write-line (read-line s)))\n");
        let (out, code, _) = run_file(&imp, &f, None).unwrap().wait_output(Duration::from_secs(30));
        assert_eq!(code, Some(0), "{out}");
        assert!(out.contains("payload-123"));
    }
}
