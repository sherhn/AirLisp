//! Project wide symbol index: Go to Symbol, Go to Declaration, usage search.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use lisp_syntax::outline::{Definition, outline};
use lisp_syntax::{Dialect, symbols};

use crate::search::{self, FuzzyMatch, TextQuery, FileHit};

#[derive(Clone, Debug)]
pub struct IndexedSymbol {
    pub path: PathBuf,
    pub def: Definition,
}

#[derive(Default)]
pub struct ProjectIndex {
    by_file: HashMap<PathBuf, Vec<Definition>>,
}

impl ProjectIndex {
    pub fn build(root: &Path) -> ProjectIndex {
        let mut idx = ProjectIndex::default();
        for p in search::walk_files(root) {
            idx.update_file_from_disk(&p);
        }
        idx
    }

    pub fn update_file_from_disk(&mut self, path: &Path) {
        let Some(d) = Dialect::from_path(path) else { return };
        match fs::read_to_string(path) {
            Ok(text) => self.update_file(path, &text, d),
            Err(_) => {
                self.by_file.remove(path);
            }
        }
    }

    pub fn update_file(&mut self, path: &Path, text: &str, d: Dialect) {
        let defs = outline(text, d);
        if defs.is_empty() {
            self.by_file.remove(path);
        } else {
            self.by_file.insert(path.to_path_buf(), defs);
        }
    }

    pub fn remove_file(&mut self, path: &Path) {
        self.by_file.remove(path);
    }

    pub fn symbol_count(&self) -> usize {
        self.all().len()
    }

    pub fn all(&self) -> Vec<IndexedSymbol> {
        let mut v: Vec<IndexedSymbol> = self
            .by_file
            .iter()
            .flat_map(|(p, ds)| ds.iter().filter(|d| d.operator.to_ascii_lowercase() != "in-package").map(|d| IndexedSymbol { path: p.clone(), def: d.clone() }))
            .collect();
        v.sort_by(|a, b| a.def.name.to_lowercase().cmp(&b.def.name.to_lowercase()).then(a.path.cmp(&b.path)).then(a.def.start.cmp(&b.def.start)));
        v
    }

    /// Definitions of `name` (package prefixes and case are ignored, as in Lisp).
    pub fn find_definitions(&self, name: &str) -> Vec<IndexedSymbol> {
        let want = normalize(name);
        self.all().into_iter().filter(|s| normalize(&s.def.name.split(' ').next().unwrap_or("").to_string()) == want).collect()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(IndexedSymbol, FuzzyMatch)> {
        let all = self.all();
        search::rank(query, &all, |s| s.def.name.clone(), limit).into_iter().map(|(i, m)| (all[i].clone(), m)).collect()
    }
}

fn normalize(name: &str) -> String {
    symbols::strip_package(name).trim_start_matches(['#', '\'', ':']).to_ascii_lowercase()
}

/// Whole-symbol usages of `name` across the project (text search, whole word).
pub fn find_usages(root: &Path, name: &str, limit: usize) -> Vec<FileHit> {
    search::find_in_files(root, name, TextQuery { case_sensitive: false, whole_word: true }, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn project() -> TempDir {
        let d = TempDir::new("index");
        fs::create_dir_all(d.path().join("src")).unwrap();
        fs::write(d.path().join("src/a.lisp"), "(in-package :a)\n(defun greet (n) n)\n(defvar *x* 1)\n").unwrap();
        fs::write(d.path().join("src/b.lisp"), "(defun other () (greet 1))\n(defmethod greet ((x string)) x)\n").unwrap();
        fs::write(d.path().join("notes.txt"), "greet").unwrap();
        d
    }

    #[test]
    fn builds_and_finds_definitions() {
        let d = project();
        let idx = ProjectIndex::build(d.path());
        assert_eq!(idx.symbol_count(), 4);
        let defs = idx.find_definitions("GREET");
        assert_eq!(defs.len(), 2);
        assert!(defs.iter().any(|s| s.path.ends_with("a.lisp")));
        assert_eq!(idx.find_definitions("pkg:greet").len(), 2, "package prefix ignored");
        assert!(idx.find_definitions("missing").is_empty());
    }

    #[test]
    fn fuzzy_symbol_search() {
        let d = project();
        let idx = ProjectIndex::build(d.path());
        let r = idx.search("gr", 5);
        assert!(r[0].0.def.name.starts_with("greet"));
        assert!(idx.search("zzzz", 5).is_empty());
    }

    #[test]
    fn incremental_update() {
        let d = project();
        let mut idx = ProjectIndex::build(d.path());
        let a = d.path().join("src/a.lisp");
        idx.update_file(&a, "(defun renamed ())", Dialect::CommonLisp);
        assert!(idx.find_definitions("renamed").len() == 1);
        assert_eq!(idx.find_definitions("greet").len(), 1);
        idx.remove_file(&a);
        assert!(idx.find_definitions("renamed").is_empty());
    }

    #[test]
    fn usages_are_whole_word() {
        let d = project();
        let hits = find_usages(d.path(), "greet", 100);
        // a.lisp def, b.lisp call, b.lisp defmethod, notes.txt
        assert_eq!(hits.len(), 4);
        fs::write(d.path().join("src/c.lisp"), "(greeting)").unwrap();
        assert_eq!(find_usages(d.path(), "greet", 100).len(), 4);
    }
}
