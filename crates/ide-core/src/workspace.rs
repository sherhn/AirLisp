//! Project model: directory listing, file operations, project scaffolding, VCS info.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::search::walk_files;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Directory listing for the project tree: folders first, then files, case-insensitive order.
pub fn list_dir(path: &Path) -> io::Result<Vec<Entry>> {
    let mut v: Vec<Entry> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if matches!(name.as_str(), ".git" | ".DS_Store" | "Thumbs.db" | ".idea") {
                return None;
            }
            let is_dir = e.file_type().ok()?.is_dir();
            Some(Entry { name, path: e.path(), is_dir })
        })
        .collect();
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(v)
}

/// All project files (relative paths use `/`), for Go to File.
pub fn list_files(root: &Path, limit: usize) -> Vec<PathBuf> {
    walk_files(root).take(limit).collect()
}

pub fn relative(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

pub fn create_file(path: &Path, content: &str) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already exists", path.display())));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

pub fn create_dir(path: &Path) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already exists", path.display())));
    }
    fs::create_dir_all(path)
}

pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already exists", to.display())));
    }
    fs::rename(from, to)
}

pub fn delete(path: &Path) -> io::Result<()> {
    if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }
}

/// Current git branch, read directly from `.git/HEAD` (no git binary needed).
pub fn git_branch(root: &Path) -> Option<String> {
    let mut dir = Some(root);
    while let Some(d) = dir {
        let head = d.join(".git").join("HEAD");
        if let Ok(s) = fs::read_to_string(&head) {
            let s = s.trim();
            return Some(match s.strip_prefix("ref: refs/heads/") {
                Some(b) => b.to_string(),
                None => s.chars().take(7).collect(),
            });
        }
        dir = d.parent();
    }
    None
}

/// Make a valid ASDF system / directory name from user input.
pub fn sanitize_project_name(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "untitled".into() } else { s }
}

/// Scaffold an ASDF based Common Lisp project. Returns the created project directory.
pub fn create_lisp_project(parent: &Path, name: &str) -> io::Result<PathBuf> {
    let name = sanitize_project_name(name);
    let root = parent.join(&name);
    if root.exists() && fs::read_dir(&root)?.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} is not empty", root.display())));
    }
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("test"))?;
    fs::write(
        root.join(format!("{name}.asd")),
        format!(
            "(asdf:defsystem \"{name}\"\n  :description \"{name}\"\n  :version \"0.1.0\"\n  :depends-on ()\n  :serial t\n  :components ((:module \"src\"\n                :components ((:file \"package\")\n                             (:file \"main\"))))\n  :in-order-to ((test-op (test-op \"{name}/test\"))))\n\n(asdf:defsystem \"{name}/test\"\n  :depends-on (\"{name}\")\n  :components ((:module \"test\"\n                :components ((:file \"tests\"))))\n  :perform (test-op (o c) (uiop:symbol-call :{name}/test :run)))\n"
        ),
    )?;
    fs::write(
        root.join("src/package.lisp"),
        format!("(defpackage :{name}\n  (:use :cl)\n  (:export #:main #:greet))\n"),
    )?;
    fs::write(
        root.join("src/main.lisp"),
        format!("(in-package :{name})\n\n(defun greet (name)\n  \"Return a greeting for NAME.\"\n  (format nil \"Hello, ~a!\" name))\n\n(defun main ()\n  (format t \"~a~%\" (greet \"world\")))\n"),
    )?;
    fs::write(
        root.join("test/tests.lisp"),
        format!("(defpackage :{name}/test\n  (:use :cl)\n  (:export #:run))\n(in-package :{name}/test)\n\n(defun run ()\n  (assert (string= (funcall (intern \"GREET\" :{name}) \"Lisp\") \"Hello, Lisp!\"))\n  (format t \"All tests passed.~%\"))\n"),
    )?;
    fs::write(root.join("README.md"), format!("# {name}\n\nA Common Lisp project created with Lispcraft.\n"))?;
    fs::write(root.join(".gitignore"), "*.fasl\n")?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn listing_sorts_dirs_first_and_hides_git() {
        let d = TempDir::new("list");
        fs::create_dir_all(d.path().join("b-dir")).unwrap();
        fs::create_dir_all(d.path().join(".git")).unwrap();
        fs::write(d.path().join("Zeta.lisp"), "").unwrap();
        fs::write(d.path().join("alpha.lisp"), "").unwrap();
        let names: Vec<_> = list_dir(d.path()).unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["b-dir", "alpha.lisp", "Zeta.lisp"]);
    }

    #[test]
    fn file_operations() {
        let d = TempDir::new("ops");
        let f = d.path().join("sub/new.lisp");
        create_file(&f, "(a)").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "(a)");
        assert!(create_file(&f, "x").is_err(), "must not overwrite");
        let g = d.path().join("sub/renamed.lisp");
        rename(&f, &g).unwrap();
        assert!(!f.exists() && g.exists());
        assert!(rename(&g, &g).is_err());
        delete(&d.path().join("sub")).unwrap();
        assert!(!d.path().join("sub").exists());
    }

    #[test]
    fn reads_git_branch() {
        let d = TempDir::new("git");
        fs::create_dir_all(d.path().join(".git")).unwrap();
        fs::write(d.path().join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(git_branch(d.path()).as_deref(), Some("feature/x"));
        fs::write(d.path().join(".git/HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(git_branch(d.path()).as_deref(), Some("0123456"));
        let other = TempDir::new("nogit");
        // may find a parent repo on dev machines, but must not panic
        let _ = git_branch(other.path());
    }

    #[test]
    fn name_sanitising() {
        assert_eq!(sanitize_project_name("My Cool App!"), "my-cool-app");
        assert_eq!(sanitize_project_name("  "), "untitled");
        assert_eq!(sanitize_project_name("a_b.c"), "a_b.c");
    }

    #[test]
    fn scaffold_project() {
        let d = TempDir::new("proj");
        let root = create_lisp_project(d.path(), "Hello World").unwrap();
        assert!(root.ends_with("hello-world"));
        for f in ["hello-world.asd", "src/package.lisp", "src/main.lisp", "test/tests.lisp", "README.md"] {
            assert!(root.join(f).exists(), "{f}");
        }
        // generated code must be balanced
        for f in ["hello-world.asd", "src/package.lisp", "src/main.lisp", "test/tests.lisp"] {
            let src = fs::read_to_string(root.join(f)).unwrap();
            let p = lisp_syntax::parse(&src, lisp_syntax::Dialect::CommonLisp);
            assert!(p.diagnostics.is_empty(), "{f}: {:?}", p.diagnostics);
        }
        assert!(create_lisp_project(d.path(), "hello world").is_err(), "non-empty target");
    }
}
