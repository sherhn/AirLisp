//! Locating a Lisp implementation on this machine.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Implementation {
    pub name: String,
    pub path: PathBuf,
    /// `SBCL_HOME` for the bundled distribution (core and contribs live next to the executable).
    pub home: Option<PathBuf>,
}

impl Implementation {
    pub fn sbcl(path: impl Into<PathBuf>) -> Implementation {
        Implementation { name: "SBCL".into(), path: path.into(), home: None }
    }

    /// An SBCL shipped inside `dir` (`dir/sbcl[.exe]`, `dir/sbcl.core`, `dir/contrib`).
    pub fn bundled(dir: &Path) -> Option<Implementation> {
        let exe = dir.join(exe_name());
        (is_file(&exe) && dir.join("sbcl.core").is_file()).then(|| Implementation { name: "SBCL".into(), path: exe, home: Some(dir.to_path_buf()) })
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
    if let Ok(p) = std::env::var("AIRLISP_SBCL") {
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

/// Directory of the SBCL that ships with AirLisp: `AIRLISP_BUNDLE_DIR`, else `lisp/` next to the executable.
pub fn bundle_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("AIRLISP_BUNDLE_DIR") {
        return Some(PathBuf::from(d));
    }
    std::env::current_exe().ok()?.parent().map(|p| p.join("lisp"))
}

/// Find SBCL: the explicit setting, `AIRLISP_SBCL`, the bundled one, `PATH`, then well-known install locations.
pub fn find_sbcl(configured: &str) -> Option<Implementation> {
    let mut explicit = Vec::new();
    if !configured.trim().is_empty() {
        explicit.push(PathBuf::from(configured.trim()));
    }
    if let Ok(p) = std::env::var("AIRLISP_SBCL") {
        explicit.push(PathBuf::from(p));
    }
    if let Some(p) = explicit.into_iter().find(|p| is_file(p)) {
        return Some(Implementation::sbcl(p));
    }
    if let Some(b) = bundle_dir().and_then(|d| Implementation::bundled(&d)) {
        return Some(b);
    }
    candidates("").into_iter().find(|p| is_file(p)).map(Implementation::sbcl)
}

/// Environment every SBCL process needs (applies `SBCL_HOME` for the bundled distribution).
pub fn apply_env(cmd: &mut Command, imp: &Implementation) {
    if let Some(h) = &imp.home {
        cmd.env("SBCL_HOME", h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_requires_executable_and_core() {
        let dir = std::env::temp_dir().join(format!("airlisp-bundle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(Implementation::bundled(&dir).is_none());
        std::fs::write(dir.join(exe_name()), b"x").unwrap();
        assert!(Implementation::bundled(&dir).is_none(), "the core is required too");
        std::fs::write(dir.join("sbcl.core"), b"x").unwrap();
        let b = Implementation::bundled(&dir).unwrap();
        assert_eq!(b.home.as_deref(), Some(dir.as_path()));
        assert_eq!(b.path, dir.join(exe_name()));
        let _ = std::fs::remove_dir_all(&dir);
    }

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
