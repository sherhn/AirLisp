//! Lisp indentation: Emacs / SLIME style.
//!
//! * special forms (`defun`, `let`, `when`, ...) indent their body by 2 and their
//!   "distinguished" arguments by 4,
//! * ordinary calls align subsequent lines with the first argument,
//! * data lists (`'(a b)`) align with their first element.

use crate::dialect::Dialect;
use crate::lexer::TokenKind;
use crate::parse::{Datum, Parsed, parse};

/// Number of "distinguished" arguments of a body-style operator, if it is one.
pub fn body_indent(name: &str) -> Option<usize> {
    let name = name.rsplit([':']).next().unwrap_or(name).to_ascii_lowercase();
    let n = match name.as_str() {
        "defun" | "defmacro" | "defmethod" | "defgeneric" | "defn" | "defn-" | "define-compiler-macro" | "define-modify-macro"
        | "defclass" | "define-condition" | "destructuring-bind" | "multiple-value-bind" | "with-slots" | "with-accessors"
        | "do" | "do*" | "prog2" | "if" | "defsetf" | "deftype" | "define-method-combination" | "defrecord" | "defprotocol"
        | "if-let" | "when-let" | "defmulti" | "defmethod-" | "fn-" => 2,
        "lambda" | "let" | "let*" | "flet" | "labels" | "macrolet" | "symbol-macrolet" | "when" | "unless" | "dolist" | "dotimes"
        | "with-open-file" | "with-open-stream" | "handler-case" | "handler-bind" | "restart-case" | "restart-bind" | "unwind-protect"
        | "block" | "case" | "ecase" | "ccase" | "typecase" | "etypecase" | "ctypecase" | "defstruct" | "defpackage" | "prog1"
        | "with-output-to-string" | "with-input-from-string" | "with-standard-io-syntax" | "catch" | "return-from" | "let-values"
        | "define" | "define-syntax" | "define-record-type" | "fn" | "loop-while" | "ns" | "doseq" | "dorun" | "binding"
        | "letfn" | "locking" | "while" | "do-times" | "with-hash-table-iterator" | "with-package-iterator" | "define-library"
        | "defvar" | "defparameter" | "defconstant" | "defsystem" | "defsetf*" | "eval-when" | "multiple-value-prog1"
        | "dolist-with-index" | "case-lambda" | "let-syntax" | "letrec" | "letrec*" | "named-lambda" | "do-all-symbols"
        | "do-symbols" | "do-external-symbols" | "assert-equal" => 1,
        "progn" | "prog" | "tagbody" | "ignore-errors" | "without-interrupts" | "begin" | "do-body" | "loop-finish" | "comment" | "future" | "time" => 0,
        _ => {
            if name.starts_with("with-") || name.starts_with("do-") || name.starts_with("def") || name.starts_with("define-") {
                1
            } else {
                return None;
            }
        }
    };
    // `progn`-likes: no distinguished args, body indent 2.
    Some(n)
}

/// Line start offsets of `src`.
pub fn line_starts(src: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

pub struct Indenter<'a> {
    src: &'a str,
    p: &'a Parsed,
    starts: Vec<usize>,
    shift: Vec<i64>,
}

impl<'a> Indenter<'a> {
    pub fn new(src: &'a str, p: &'a Parsed) -> Self {
        let starts = line_starts(src);
        let shift = vec![0; starts.len()];
        Indenter { src, p, starts, shift }
    }

    pub fn set_shift(&mut self, line: usize, shift: i64) {
        self.shift[line] = shift;
    }

    pub fn starts(&self) -> &[usize] {
        &self.starts
    }

    fn line_of(&self, off: usize) -> usize {
        self.starts.partition_point(|&s| s <= off) - 1
    }

    fn col(&self, off: usize) -> i64 {
        let l = self.line_of(off);
        self.src[self.starts[l]..off].chars().count() as i64 + self.shift[l]
    }

    fn first_on_line(&self, off: usize) -> bool {
        let l = self.line_of(off);
        self.src[self.starts[l]..off].chars().all(char::is_whitespace)
    }

    fn tok_text(&self, i: usize) -> &str {
        self.p.tokens[i].text(self.src)
    }

    /// Indentation column for a line that begins at `ls` and sits directly inside `open`.
    pub fn indent_inside(&self, open: Option<usize>, ls: usize) -> usize {
        let Some(o) = open else { return 0 };
        let tokens = &self.p.tokens;
        let open_col = self.col(tokens[o].start);
        let kids: Vec<Datum> = self.p.children(o).into_iter().filter(|d| self.p.start_of(*d) < ls).collect();
        let result = if kids.is_empty() {
            open_col + 1
        } else {
            let bracket = self.src.as_bytes()[tokens[o].start];
            let quoted = o > 0 && tokens[o - 1].kind == TokenKind::Prefix && tokens[o - 1].end == tokens[o].start && matches!(self.tok_text(o - 1), "'" | "`" | "#" | "#'");
            let head = kids[0];
            let head_is_symbol = head.first == head.last && tokens[head.first].kind == TokenKind::Symbol;
            if bracket != b'(' || quoted || !head_is_symbol {
                self.align_with_siblings(&kids, 0)
            } else {
                let name = self.tok_text(head.first);
                let idx = kids.len() - 1; // index of the argument being indented
                match body_indent(name) {
                    Some(n) => {
                        if idx < n { open_col + 4 } else { open_col + 2 }
                    }
                    None => {
                        if kids.len() == 1 {
                            open_col + 1
                        } else {
                            self.align_with_siblings(&kids, 1)
                        }
                    }
                }
            }
        };
        result.max(0) as usize
    }

    /// Align with the nearest previous sibling that starts its own line,
    /// falling back to the first sibling at index `from`.
    fn align_with_siblings(&self, kids: &[Datum], from: usize) -> i64 {
        for k in kids[from..].iter().rev() {
            let start = self.p.start_of(*k);
            if self.first_on_line(start) {
                return self.col(start);
            }
        }
        self.col(self.p.start_of(kids[from]))
    }
}

/// Indentation (in columns) for the line starting at byte offset `ls`.
/// `None` when the line starts inside a string or block comment.
pub fn indent_for_line_start(src: &str, dialect: Dialect, ls: usize) -> Option<usize> {
    let p = parse(src, dialect);
    indent_for_line_start_parsed(src, &p, ls)
}

pub fn indent_for_line_start_parsed(src: &str, p: &Parsed, ls: usize) -> Option<usize> {
    if p.in_string_or_block_comment(ls) {
        return None;
    }
    let ind = Indenter::new(src, p);
    let open = p.enclosing_open(ls);
    Some(ind.indent_inside(open, ls))
}

/// Reindent the whole document and strip trailing whitespace.
pub fn format(src: &str, dialect: Dialect) -> String {
    let p = parse(src, dialect);
    let mut ind = Indenter::new(src, &p);
    let starts = ind.starts().to_vec();
    let mut out = String::with_capacity(src.len());
    let mut stack: Vec<usize> = Vec::new();
    let mut ti = 0usize; // token cursor for tracking the open stack
    for (li, &ls) in starts.iter().enumerate() {
        let le = starts.get(li + 1).copied().unwrap_or(src.len());
        let raw = &src[ls..le];
        let (body, nl) = match raw.strip_suffix('\n') {
            Some(b) => (b, "\n"),
            None => (raw, ""),
        };
        let (body, cr) = match body.strip_suffix('\r') {
            Some(b) => (b, "\r"),
            None => (body, ""),
        };
        // Advance the open-bracket stack up to this line start.
        while ti < p.tokens.len() && p.tokens[ti].start < ls {
            match p.tokens[ti].kind {
                TokenKind::Open => stack.push(ti),
                TokenKind::Close => {
                    stack.pop();
                }
                _ => {}
            }
            ti += 1;
        }
        let inside_literal = p.in_string_or_block_comment(ls);
        if inside_literal {
            out.push_str(raw);
            continue;
        }
        let trimmed_end = body.trim_end();
        // A line that ends inside a string/block comment keeps its trailing whitespace.
        let ends_in_literal = !body.is_empty() && p.in_string_or_block_comment(ls + body.len()) && trimmed_end.len() != body.len();
        let content_full = if ends_in_literal { body } else { trimmed_end };
        let content = content_full.trim_start();
        if content.is_empty() {
            out.push_str(nl);
            continue;
        }
        let old_indent = content_full.chars().count() - content.chars().count();
        let new_indent = if content.starts_with(";;;") { 0 } else { ind.indent_inside(stack.last().copied(), ls) };
        ind.set_shift(li, new_indent as i64 - old_indent as i64);
        for _ in 0..new_indent {
            out.push(' ');
        }
        out.push_str(content);
        if ends_in_literal {
            // preserved trailing whitespace is already part of `content_full`
        }
        out.push_str(cr);
        out.push_str(nl);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(s: &str) -> String {
        format(s, Dialect::CommonLisp)
    }

    #[test]
    fn defun_body() {
        let src = "(defun foo (x)\n(+ x 1))\n";
        assert_eq!(fmt(src), "(defun foo (x)\n  (+ x 1))\n");
    }

    #[test]
    fn call_aligns_with_first_arg() {
        let src = "(foo a\nb\nc)\n";
        assert_eq!(fmt(src), "(foo a\n     b\n     c)\n");
    }

    #[test]
    fn call_with_args_on_next_line() {
        let src = "(foo\na\nb)\n";
        assert_eq!(fmt(src), "(foo\n a\n b)\n");
    }

    #[test]
    fn let_bindings_and_body() {
        let src = "(let ((a 1)\n(b 2))\n(+ a b))\n";
        assert_eq!(fmt(src), "(let ((a 1)\n      (b 2))\n  (+ a b))\n");
    }

    #[test]
    fn if_special() {
        let src = "(if (> a b)\na\nb)\n";
        assert_eq!(fmt(src), "(if (> a b)\n    a\n  b)\n");
    }

    #[test]
    fn data_list_aligns_with_first_element() {
        let src = "'(a b\nc)\n";
        assert_eq!(fmt(src), "'(a b\n  c)\n");
    }

    #[test]
    fn nested_and_cond() {
        let src = "(defun f (x)\n(cond ((= x 1) 'one)\n((= x 2) 'two)\n(t 'many)))\n";
        assert_eq!(fmt(src), "(defun f (x)\n  (cond ((= x 1) 'one)\n        ((= x 2) 'two)\n        (t 'many)))\n");
    }

    #[test]
    fn strings_and_block_comments_untouched() {
        let src = "(defun f ()\n  \"doc\n    keep   \n  me\"\n#|\n  x\n|#\n  nil)\n";
        let out = fmt(src);
        assert_eq!(out, "(defun f ()\n  \"doc\n    keep   \n  me\"\n  #|\n  x\n|#\n  nil)\n");
    }

    #[test]
    fn trims_trailing_whitespace_and_keeps_crlf() {
        assert_eq!(fmt("(a b)   \n(c)\t\n"), "(a b)\n(c)\n");
        assert_eq!(fmt("(a\r\nb)\r\n"), "(a\r\n b)\r\n");
    }

    #[test]
    fn triple_semicolon_goes_to_column_zero() {
        assert_eq!(fmt("(a\n  ;;; x\n b)\n"), "(a\n;;; x\n b)\n");
    }

    #[test]
    fn idempotent() {
        let src = "(defun foo (a b &optional (c 1))\n\"Doc.\"\n(let* ((x (+ a b))\n(y (* x c)))\n(when (> y 10)\n(format t \"big ~a~%\" y)\n(list x\ny))\n))\n(with-open-file (s \"f\")\n(read s))\n";
        let once = fmt(src);
        assert_eq!(fmt(&once), once);
    }

    #[test]
    fn indent_for_new_line() {
        let src = "(defun foo (x)\n";
        assert_eq!(indent_for_line_start(src, Dialect::CommonLisp, src.len()), Some(2));
        let src = "(foo a\n";
        assert_eq!(indent_for_line_start(src, Dialect::CommonLisp, src.len()), Some(5));
        let src = "(let ((a 1)\n";
        assert_eq!(indent_for_line_start(src, Dialect::CommonLisp, src.len()), Some(6));
        assert_eq!(indent_for_line_start("(a)\n", Dialect::CommonLisp, 4), Some(0));
    }

    #[test]
    fn clojure_vector_alignment() {
        let src = "(defn f [a\nb]\n(+ a b))\n";
        assert_eq!(format(src, Dialect::Clojure), "(defn f [a\n         b]\n  (+ a b))\n");
    }

    #[test]
    fn unbalanced_input_does_not_panic() {
        for s in ["(((", ")))", "(a\n\n\n", "\"x\n(", "#| x\n(", "(a . b\n"] {
            let _ = fmt(s);
        }
    }
}
