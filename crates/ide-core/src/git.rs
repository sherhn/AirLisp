//! Git integration through the `git` executable.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl ChangeKind {
    pub fn letter(self) -> &'static str {
        match self {
            ChangeKind::Modified => "M",
            ChangeKind::Added => "A",
            ChangeKind::Deleted => "D",
            ChangeKind::Renamed => "R",
            ChangeKind::Untracked => "?",
            ChangeKind::Conflicted => "!",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// Path relative to the repository root, `/` separated.
    pub path: String,
    pub kind: ChangeKind,
    pub staged: bool,
    pub unstaged: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub remote: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub when: String,
    pub subject: String,
}

#[derive(Clone, Debug)]
pub struct Repo {
    pub root: PathBuf,
}

pub fn git_available() -> bool {
    run_in(Path::new("."), &["--version"]).is_ok()
}

fn run_in(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-c").arg("core.quotepath=false").arg("-c").arg("color.ui=false").args(args).current_dir(dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let o = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Err(if err.is_empty() { o } else { err })
    }
}

/// Find the repository containing `start` by walking up to a `.git` entry.
pub fn find_repo(start: &Path) -> Option<Repo> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Some(Repo { root: d.to_path_buf() });
        }
        dir = d.parent();
    }
    None
}

/// `git init` in `dir`.
pub fn init(dir: &Path) -> Result<Repo, String> {
    run_in(dir, &["init", "-q"])?;
    find_repo(dir).ok_or_else(|| "git init did not create a repository".to_string())
}

impl Repo {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        run_in(&self.root, args)
    }

    pub fn branch(&self) -> Option<String> {
        let b = self.run(&["rev-parse", "--abbrev-ref", "HEAD"]).ok()?.trim().to_string();
        if b == "HEAD" {
            // detached or no commits yet
            return self.run(&["rev-parse", "--short", "HEAD"]).ok().map(|s| s.trim().to_string()).or_else(|| self.run(&["symbolic-ref", "--short", "HEAD"]).ok().map(|s| s.trim().to_string()));
        }
        (!b.is_empty()).then_some(b)
    }

    pub fn head_hash(&self) -> Option<String> {
        self.run(&["rev-parse", "HEAD"]).ok().map(|s| s.trim().to_string())
    }

    pub fn status(&self) -> Vec<Change> {
        let Ok(out) = self.run(&["status", "--porcelain=v1", "-z", "--untracked-files=all"]) else { return vec![] };
        let mut v = Vec::new();
        let mut parts = out.split('\0').filter(|s| !s.is_empty());
        while let Some(rec) = parts.next() {
            if rec.len() < 4 {
                continue;
            }
            let (x, y) = (rec.as_bytes()[0] as char, rec.as_bytes()[1] as char);
            let path = rec[3..].to_string();
            if x == 'R' || x == 'C' {
                parts.next(); // original path
            }
            let kind = if x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
                ChangeKind::Conflicted
            } else if x == '?' {
                ChangeKind::Untracked
            } else if x == 'R' || y == 'R' {
                ChangeKind::Renamed
            } else if x == 'A' || y == 'A' {
                ChangeKind::Added
            } else if x == 'D' || y == 'D' {
                ChangeKind::Deleted
            } else {
                ChangeKind::Modified
            };
            v.push(Change { path, kind, staged: x != ' ' && x != '?', unstaged: y != ' ' || x == '?' });
        }
        v.sort_by(|a, b| a.path.cmp(&b.path));
        v
    }

    pub fn branches(&self) -> Vec<Branch> {
        let Ok(out) = self.run(&["branch", "-a", "--format=%(HEAD)|%(refname:short)"]) else { return vec![] };
        out.lines()
            .filter_map(|l| {
                let (h, name) = l.split_once('|')?;
                if name.ends_with("/HEAD") || name.contains("HEAD detached") || name.starts_with('(') {
                    return None;
                }
                Some(Branch { name: name.to_string(), current: h == "*", remote: name.contains('/') && name.split('/').next().is_some_and(|r| self.is_remote(r)) })
            })
            .collect()
    }

    fn is_remote(&self, name: &str) -> bool {
        self.run(&["remote"]).map(|o| o.lines().any(|l| l == name)).unwrap_or(false)
    }

    pub fn has_remote(&self) -> bool {
        self.run(&["remote"]).map(|o| !o.trim().is_empty()).unwrap_or(false)
    }

    /// Content of `rel` at HEAD (None for untracked files / no commits).
    pub fn head_content(&self, rel: &str) -> Option<String> {
        self.run(&["show", &format!("HEAD:{rel}")]).ok()
    }

    pub fn rel(&self, p: &Path) -> Option<String> {
        p.strip_prefix(&self.root).ok().map(|r| r.to_string_lossy().replace('\\', "/"))
    }

    pub fn log(&self, n: usize) -> Vec<Commit> {
        let fmt = "--format=%H%x1f%h%x1f%an%x1f%ar%x1f%s";
        let Ok(out) = self.run(&["log", &format!("-n{n}"), fmt]) else { return vec![] };
        out.lines()
            .filter_map(|l| {
                let mut p = l.split('\u{1f}');
                Some(Commit { hash: p.next()?.into(), short: p.next()?.into(), author: p.next()?.into(), when: p.next()?.into(), subject: p.next().unwrap_or("").into() })
            })
            .collect()
    }

    pub fn show(&self, hash: &str) -> Result<String, String> {
        self.run(&["show", "--stat", "--format=%h  %an  %ar%n%n%B", hash])
    }

    pub fn stage(&self, paths: &[String]) -> Result<(), String> {
        let mut args = vec!["add", "--"];
        args.extend(paths.iter().map(String::as_str));
        self.run(&args).map(|_| ())
    }

    pub fn unstage(&self, paths: &[String]) -> Result<(), String> {
        let mut args = vec!["reset", "-q", "--"];
        args.extend(paths.iter().map(String::as_str));
        self.run(&args).map(|_| ())
    }

    /// Commit exactly `paths` (staging them first); other staged changes stay out of this commit only if
    /// they were not already staged, so callers normally pass the full selection.
    pub fn commit(&self, message: &str, paths: &[String]) -> Result<String, String> {
        if message.trim().is_empty() {
            return Err("Commit message is empty".into());
        }
        if paths.is_empty() {
            return Err("Nothing selected to commit".into());
        }
        self.stage(paths)?;
        let mut args = vec!["commit", "-q", "-m", message, "--"];
        args.extend(paths.iter().map(String::as_str));
        self.run(&args)?;
        Ok(self.run(&["rev-parse", "--short", "HEAD"]).unwrap_or_default().trim().to_string())
    }

    pub fn checkout(&self, branch: &str) -> Result<(), String> {
        self.run(&["checkout", "-q", branch]).map(|_| ())
    }

    pub fn create_branch(&self, name: &str) -> Result<(), String> {
        self.run(&["checkout", "-q", "-b", name]).map(|_| ())
    }

    /// Discard local changes to a tracked file; untracked files are deleted.
    pub fn rollback(&self, change: &Change) -> Result<(), String> {
        if change.kind == ChangeKind::Untracked {
            return std::fs::remove_file(self.root.join(&change.path)).map_err(|e| e.to_string());
        }
        if change.staged {
            self.unstage(std::slice::from_ref(&change.path))?;
        }
        if change.kind == ChangeKind::Added {
            return std::fs::remove_file(self.root.join(&change.path)).map_err(|e| e.to_string());
        }
        self.run(&["checkout", "-q", "HEAD", "--", &change.path]).map(|_| ())
    }

    pub fn pull(&self) -> Result<String, String> {
        self.run(&["pull", "--ff-only"]).map(|s| s.trim().to_string())
    }

    pub fn push(&self) -> Result<String, String> {
        let b = self.branch().ok_or("no current branch")?;
        self.run(&["push", "-u", "origin", &b]).map(|s| s.trim().to_string())
    }

    pub fn diff_text(&self, path: &str) -> String {
        self.run(&["diff", "HEAD", "--", path]).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn repo() -> Option<(TempDir, Repo)> {
        if !git_available() {
            return None;
        }
        let d = TempDir::new("git");
        let r = init(d.path()).ok()?;
        r.run(&["config", "user.email", "t@example.com"]).ok()?;
        r.run(&["config", "user.name", "Tester"]).ok()?;
        r.run(&["config", "commit.gpgsign", "false"]).ok()?;
        Some((d, r))
    }

    #[test]
    fn status_commit_log_and_head_content() {
        let Some((d, r)) = repo() else { return };
        std::fs::write(d.path().join("a.lisp"), "(a)\n").unwrap();
        std::fs::write(d.path().join("b.lisp"), "(b)\n").unwrap();
        let st = r.status();
        assert_eq!(st.len(), 2);
        assert!(st.iter().all(|c| c.kind == ChangeKind::Untracked && c.unstaged && !c.staged));
        assert_eq!(r.commit("", &["a.lisp".into()]), Err("Commit message is empty".into()));
        let short = r.commit("first", &["a.lisp".into(), "b.lisp".into()]).unwrap();
        assert!(!short.is_empty());
        assert!(r.status().is_empty());
        assert_eq!(r.head_content("a.lisp").as_deref(), Some("(a)\n"));
        assert_eq!(r.head_content("nope.lisp"), None);
        let log = r.log(5);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].subject, "first");
        assert_eq!(log[0].author, "Tester");
        assert!(r.head_hash().is_some());
        assert!(r.show(&log[0].hash).unwrap().contains("first"));
    }

    #[test]
    fn modifications_deletions_and_rollback() {
        let Some((d, r)) = repo() else { return };
        std::fs::write(d.path().join("a.lisp"), "one\n").unwrap();
        std::fs::write(d.path().join("b.lisp"), "two\n").unwrap();
        r.commit("init", &["a.lisp".into(), "b.lisp".into()]).unwrap();
        std::fs::write(d.path().join("a.lisp"), "changed\n").unwrap();
        std::fs::remove_file(d.path().join("b.lisp")).unwrap();
        std::fs::write(d.path().join("c.lisp"), "new\n").unwrap();
        let st = r.status();
        let kind = |p: &str| st.iter().find(|c| c.path == p).unwrap().kind;
        assert_eq!(kind("a.lisp"), ChangeKind::Modified);
        assert_eq!(kind("b.lisp"), ChangeKind::Deleted);
        assert_eq!(kind("c.lisp"), ChangeKind::Untracked);
        assert!(r.diff_text("a.lisp").contains("+changed"));
        let a = st.iter().find(|c| c.path == "a.lisp").unwrap().clone();
        r.rollback(&a).unwrap();
        assert_eq!(std::fs::read_to_string(d.path().join("a.lisp")).unwrap(), "one\n");
        let c = st.iter().find(|c| c.path == "c.lisp").unwrap().clone();
        r.rollback(&c).unwrap();
        assert!(!d.path().join("c.lisp").exists());
        r.stage(&["b.lisp".into()]).unwrap();
        assert!(r.status().iter().any(|c| c.path == "b.lisp" && c.staged));
        r.unstage(&["b.lisp".into()]).unwrap();
        assert!(r.status().iter().any(|c| c.path == "b.lisp" && !c.staged && c.unstaged));
    }

    #[test]
    fn branches_checkout_and_create() {
        let Some((d, r)) = repo() else { return };
        std::fs::write(d.path().join("a.lisp"), "x\n").unwrap();
        r.commit("init", &["a.lisp".into()]).unwrap();
        let first = r.branch().unwrap();
        r.create_branch("feature/x").unwrap();
        assert_eq!(r.branch().as_deref(), Some("feature/x"));
        let bs = r.branches();
        assert!(bs.iter().any(|b| b.name == "feature/x" && b.current && !b.remote));
        assert!(bs.iter().any(|b| b.name == first && !b.current));
        r.checkout(&first).unwrap();
        assert_eq!(r.branch().as_deref(), Some(first.as_str()));
        assert!(r.checkout("does-not-exist").is_err());
        assert!(!r.has_remote());
    }

    #[test]
    fn find_repo_walks_up() {
        let Some((d, r)) = repo() else { return };
        std::fs::create_dir_all(d.path().join("sub/deeper")).unwrap();
        let found = find_repo(&d.path().join("sub/deeper")).unwrap();
        assert_eq!(found.root, r.root);
        assert_eq!(r.rel(&d.path().join("sub/x.lisp")).as_deref(), Some("sub/x.lisp"));
    }

    #[test]
    fn spaces_and_unicode_paths() {
        let Some((d, r)) = repo() else { return };
        std::fs::write(d.path().join("my file.lisp"), "x\n").unwrap();
        std::fs::write(d.path().join("файл.lisp"), "y\n").unwrap();
        let st = r.status();
        assert!(st.iter().any(|c| c.path == "my file.lisp"));
        assert!(st.iter().any(|c| c.path == "файл.lisp"), "{st:?}");
        r.commit("m", &["my file.lisp".into(), "файл.lisp".into()]).unwrap();
        assert!(r.status().is_empty());
    }

    #[test]
    fn push_without_remote_reports_error() {
        let Some((d, r)) = repo() else { return };
        std::fs::write(d.path().join("a"), "x").unwrap();
        r.commit("c", &["a".into()]).unwrap();
        assert!(r.push().is_err());
    }
}
