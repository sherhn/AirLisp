//! Locating a Lisp implementation on this machine.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Implementation {
    pub name: String,
    pub path: PathBuf,
}

impl Implementation {
    pub fn sbcl(path: impl Into<PathBuf>) -> Implementation {
        Implementation { name: "SBCL".into(), path: path.into() }
    }

    /// Query the version string, e.g. `SBCL 2.6.8`.
    pub fn version(&self) -> Option<String> {
        let mut cmd = Command::new(&self.path);
        crate::no_window(&mut cmd);
        let out = cmd.arg("--version").output().ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "sbcl.exe" } else { "sbcl" }
}

fn is_file(p: &Path) -> bool {
    p.is_file()
}

/// Candidate locations in priority order. `configured` is the user's explicit setting.
pub fn candidates(configured: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if !configured.trim().is_empty() {
        v.push(PathBuf::from(configured.trim()));
    }
    if let Ok(p) = std::env::var("LISPCRAFT_SBCL") {
        v.push(PathBuf::from(p));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            v.push(dir.join(exe_name()));
        }
    }
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Ok(base) = std::env::var(var) {
                v.push(Path::new(&base).join("Steel Bank Common Lisp").join("sbcl.exe"));
                v.push(Path::new(&base).join("Programs").join("Steel Bank Common Lisp").join("sbcl.exe"));
            }
        }
    } else {
        for p in ["/usr/bin/sbcl", "/usr/local/bin/sbcl", "/opt/homebrew/bin/sbcl", "/opt/local/bin/sbcl", "/snap/bin/sbcl"] {
            v.push(PathBuf::from(p));
        }
        if let Some(home) = std::env::var_os("HOME") {
            v.push(Path::new(&home).join(".roswell/bin/sbcl"));
            v.push(Path::new(&home).join(".guix-profile/bin/sbcl"));
            v.push(Path::new(&home).join(".nix-profile/bin/sbcl"));
        }
    }
    v
}

/// Find SBCL: the explicit setting, `LISPCRAFT_SBCL`, `PATH`, then well-known install locations.
pub fn find_sbcl(configured: &str) -> Option<Implementation> {
    candidates(configured).into_iter().find(|p| is_file(p)).map(Implementation::sbcl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_path_has_priority() {
        let c = candidates("C:/custom/sbcl.exe");
        assert_eq!(c[0], PathBuf::from("C:/custom/sbcl.exe"));
        assert!(candidates("").len() > 1);
    }

    #[test]
    fn missing_configured_path_falls_through() {
        // a bogus explicit path must not hide a working system install
        let found = find_sbcl("Z:/definitely/not/here/sbcl.exe");
        if let Some(f) = found {
            assert!(f.path.is_file());
        }
    }

    #[test]
    fn finds_sbcl_and_reports_version_when_installed() {
        if let Some(i) = find_sbcl("") {
            let v = i.version().expect("version output");
            assert!(v.to_lowercase().contains("sbcl"), "{v}");
        }
    }
}
