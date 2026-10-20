use std::path::Path;

/// Supported Lisp dialects. Common Lisp is the primary one (SBCL backed REPL).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dialect {
    #[default]
    CommonLisp,
    Scheme,
    Clojure,
    EmacsLisp,
}

impl Dialect {
    pub fn from_path(path: &Path) -> Option<Dialect> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "lisp" | "lsp" | "cl" | "asd" | "asdf" | "ros" | "ny" | "sbclrc" => Dialect::CommonLisp,
            "scm" | "ss" | "sld" | "sls" | "rkt" => Dialect::Scheme,
            "clj" | "cljs" | "cljc" | "edn" => Dialect::Clojure,
            "el" => Dialect::EmacsLisp,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Dialect::CommonLisp => "Common Lisp",
            Dialect::Scheme => "Scheme",
            Dialect::Clojure => "Clojure",
            Dialect::EmacsLisp => "Emacs Lisp",
        }
    }

    pub fn default_extension(self) -> &'static str {
        match self {
            Dialect::CommonLisp => "lisp",
            Dialect::Scheme => "scm",
            Dialect::Clojure => "clj",
            Dialect::EmacsLisp => "el",
        }
    }

    pub fn has_block_comments(self) -> bool {
        matches!(self, Dialect::CommonLisp | Dialect::Scheme)
    }
}

pub fn is_lisp_file(path: &Path) -> bool {
    Dialect::from_path(path).is_some()
}
