//! Structure view: top-level definitions of a source file.

use crate::dialect::Dialect;
use crate::lexer::TokenKind;
use crate::parse::{Parsed, parse};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Macro,
    Generic,
    Method,
    Variable,
    Constant,
    Class,
    Struct,
    Condition,
    Type,
    Package,
    System,
    Other,
}

impl SymbolKind {
    pub fn label(self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Macro => "macro",
            SymbolKind::Generic => "generic",
            SymbolKind::Method => "method",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Condition => "condition",
            SymbolKind::Type => "type",
            SymbolKind::Package => "package",
            SymbolKind::System => "system",
            SymbolKind::Other => "definition",
        }
    }

    /// Single letter badge as in the IDE structure view.
    pub fn badge(self) -> &'static str {
        match self {
            SymbolKind::Function => "f",
            SymbolKind::Macro => "m",
            SymbolKind::Generic => "g",
            SymbolKind::Method => "M",
            SymbolKind::Variable => "v",
            SymbolKind::Constant => "c",
            SymbolKind::Class => "C",
            SymbolKind::Struct => "S",
            SymbolKind::Condition => "E",
            SymbolKind::Type => "T",
            SymbolKind::Package => "P",
            SymbolKind::System => "A",
            SymbolKind::Other => "o",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub kind: SymbolKind,
    pub name: String,
    /// The defining operator as written (`defun`, `defn`, ...).
    pub operator: String,
    /// Byte range of the whole top-level form.
    pub start: usize,
    pub end: usize,
    /// Byte range of the name token(s).
    pub name_start: usize,
    pub name_end: usize,
    /// 0-based line of the form start.
    pub line: usize,
}

fn classify(op: &str, dialect: Dialect) -> Option<SymbolKind> {
    let op = op.to_ascii_lowercase();
    let op = op.rsplit(':').next().unwrap_or(&op);
    Some(match op {
        "defun" | "defn" | "defn-" | "define-compiler-macro" | "defsetf" => SymbolKind::Function,
        "define" if dialect != Dialect::CommonLisp => SymbolKind::Function,
        "defmacro" | "define-syntax" | "define-modify-macro" | "define-symbol-macro" | "defsyntax" => SymbolKind::Macro,
        "defgeneric" | "defmulti" | "defprotocol" => SymbolKind::Generic,
        "defmethod" => SymbolKind::Method,
        "defvar" | "defparameter" | "def" | "define-global" => SymbolKind::Variable,
        "defconstant" => SymbolKind::Constant,
        "defclass" | "defrecord" | "define-record-type" => SymbolKind::Class,
        "defstruct" => SymbolKind::Struct,
        "define-condition" => SymbolKind::Condition,
        "deftype" => SymbolKind::Type,
        "defpackage" | "define-package" | "ns" | "in-package" | "define-library" => SymbolKind::Package,
        "defsystem" => SymbolKind::System,
        s if s.starts_with("def") || s.starts_with("define-") => SymbolKind::Other,
        _ => return None,
    })
}

pub fn outline(src: &str, dialect: Dialect) -> Vec<Definition> {
    let p = parse(src, dialect);
    outline_parsed(src, &p)
}

pub fn outline_parsed(src: &str, p: &Parsed) -> Vec<Definition> {
    let mut out = Vec::new();
    let starts = crate::indent::line_starts(src);
    let line_of = |off: usize| starts.partition_point(|&s| s <= off) - 1;
    for d in p.top_level() {
        // `#+sbcl (defun ...)`, `'(...)`: the form is the list that closes the datum
        if p.tokens[d.last].kind != TokenKind::Close {
            continue;
        }
        let Some(first) = p.partner[d.last] else { continue };
        let kids = p.children(first);
        let Some(head) = kids.first() else { continue };
        if head.first != head.last || p.tokens[head.first].kind != TokenKind::Symbol {
            continue;
        }
        let op = p.tokens[head.first].text(src);
        let Some(kind) = classify(op, p.dialect) else { continue };
        let Some(name_d) = kids.get(1) else {
            continue;
        };
        // name token: a symbol, a string/keyword (defpackage :foo), `(setf foo)`, or `(name options...)`, or `(f args)` for scheme define
        let name_tok = p.tokens[name_d.first];
        let (name_start, name_end, name) = match name_tok.kind {
            TokenKind::Open => {
                let inner = p.children(name_d.first);
                match inner.first() {
                    Some(i) if p.tokens[i.first].text(src).eq_ignore_ascii_case("setf") && inner.len() > 1 => {
                        let s = p.start_of(*name_d);
                        let e = p.end_of(*name_d);
                        (s, e, src[s..e].to_string())
                    }
                    Some(i) => {
                        let s = p.start_of(*i);
                        let e = p.end_of(*i);
                        (s, e, src[s..e].to_string())
                    }
                    None => continue,
                }
            }
            TokenKind::Close => continue,
            _ => {
                let s = p.start_of(*name_d);
                let e = p.end_of(*name_d);
                let raw = &src[s..e];
                let name = raw.trim_start_matches(['\'', '#', ':']).trim_matches('"').to_string();
                (s, e, name)
            }
        };
        if name.is_empty() {
            continue;
        }
        // defmethod: append qualifier/specializer hint
        let mut display = name;
        if kind == SymbolKind::Method {
            let quals: Vec<&str> = kids[2..]
                .iter()
                .take_while(|k| p.tokens[k.first].kind == TokenKind::Keyword)
                .map(|k| &src[p.start_of(*k)..p.end_of(*k)])
                .collect();
            if !quals.is_empty() {
                display = format!("{display} {}", quals.join(" "));
            }
        }
        let start = p.tokens[d.first].start;
        out.push(Definition {
            kind,
            name: display,
            operator: op.to_string(),
            start,
            end: p.end_of(d),
            name_start,
            name_end,
            line: line_of(start),
        });
    }
    out
}

/// Find the definition whose form contains `offset`.
pub fn definition_at(defs: &[Definition], offset: usize) -> Option<&Definition> {
    // top-level definitions are ordered and disjoint: binary search
    let i = defs.partition_point(|d| d.end < offset);
    defs.get(i).filter(|d| d.start <= offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"(defpackage :demo (:use :cl))
(in-package :demo)

(defvar *count* 0)
(defconstant +max+ 10)

(defun greet (name)
  "Say hello."
  (format t "Hello, ~a~%" name))

(defmacro my-when (c &body b) `(if ,c (progn ,@b)))
(defgeneric area (shape))
(defmethod area :around ((s square)) 1)
(defclass square () ())
(defstruct (point (:constructor mk-point)) x y)
(defun (setf thing) (v o) v)
#+sbcl (defun only-sbcl () 1)
(print "not a definition")
"#;

    #[test]
    fn finds_definitions() {
        let defs = outline(SRC, Dialect::CommonLisp);
        let got: Vec<_> = defs.iter().map(|d| (d.kind, d.name.as_str())).collect();
        assert_eq!(
            got,
            vec![
                (SymbolKind::Package, "demo"),
                (SymbolKind::Package, "demo"),
                (SymbolKind::Variable, "*count*"),
                (SymbolKind::Constant, "+max+"),
                (SymbolKind::Function, "greet"),
                (SymbolKind::Macro, "my-when"),
                (SymbolKind::Generic, "area"),
                (SymbolKind::Method, "area :around"),
                (SymbolKind::Class, "square"),
                (SymbolKind::Struct, "point"),
                (SymbolKind::Function, "(setf thing)"),
                (SymbolKind::Function, "only-sbcl"),
            ]
        );
    }

    #[test]
    fn name_ranges_point_at_the_name() {
        let defs = outline(SRC, Dialect::CommonLisp);
        let g = defs.iter().find(|d| d.name == "greet").unwrap();
        assert_eq!(&SRC[g.name_start..g.name_end], "greet");
        assert_eq!(g.line, 6);
        assert!(SRC[g.start..g.end].starts_with("(defun greet"));
        assert!(SRC[g.start..g.end].ends_with("name))"));
    }

    #[test]
    fn scheme_and_clojure() {
        let d = outline("(define (square x) (* x x))\n(define pi 3)\n", Dialect::Scheme);
        assert_eq!(d.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["square", "pi"]);
        let d = outline("(ns app.core)\n(defn -main [& args] 1)\n(def x 1)\n", Dialect::Clojure);
        assert_eq!(d.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["app.core", "-main", "x"]);
    }

    #[test]
    fn definition_lookup() {
        let defs = outline(SRC, Dialect::CommonLisp);
        let off = SRC.find("Hello").unwrap();
        assert_eq!(definition_at(&defs, off).unwrap().name, "greet");
    }

    #[test]
    fn broken_code_does_not_panic() {
        for s in ["(defun", "(defun )", "(defun (", "(def", "#+sbcl", "#+", "(defmethod x :a"] {
            let _ = outline(s, Dialect::CommonLisp);
        }
    }
}
