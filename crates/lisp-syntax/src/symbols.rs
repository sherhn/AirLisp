//! Static knowledge about the standard Common Lisp vocabulary (and common Scheme/Clojure forms),
//! used for highlighting and as a completion fallback when no REPL is running.

use std::collections::HashSet;
use std::sync::OnceLock;

/// Special operators, macros and definers: highlighted like keywords.
pub const KEYWORDS: &[&str] = &[
    "block", "catch", "eval-when", "flet", "function", "go", "if", "labels", "let", "let*", "load-time-value", "locally",
    "macrolet", "multiple-value-call", "multiple-value-prog1", "progn", "progv", "quote", "return-from", "setq",
    "symbol-macrolet", "tagbody", "the", "throw", "unwind-protect",
    "and", "assert", "case", "ccase", "check-type", "cond", "ctypecase", "declaim", "declare", "decf", "defclass",
    "defconstant", "defgeneric", "define-compiler-macro", "define-condition", "define-method-combination",
    "define-modify-macro", "define-setf-expander", "define-symbol-macro", "defmacro", "defmethod", "defpackage",
    "defparameter", "defsetf", "defstruct", "deftype", "defun", "defvar", "destructuring-bind", "do", "do*",
    "do-all-symbols", "do-external-symbols", "do-symbols", "dolist", "dotimes", "ecase", "etypecase", "formatter",
    "handler-bind", "handler-case", "ignore-errors", "in-package", "incf", "lambda", "loop", "loop-finish",
    "multiple-value-bind", "multiple-value-list", "multiple-value-setq", "nth-value", "or", "pop", "pprint-logical-block",
    "print-unreadable-object", "prog", "prog*", "prog1", "prog2", "psetf", "psetq", "push", "pushnew", "remf",
    "restart-bind", "restart-case", "return", "rotatef", "setf", "shiftf", "step", "time", "trace", "typecase",
    "unless", "untrace", "when", "with-accessors", "with-compilation-unit", "with-condition-restarts",
    "with-hash-table-iterator", "with-input-from-string", "with-open-file", "with-open-stream",
    "with-output-to-string", "with-package-iterator", "with-simple-restart", "with-slots", "with-standard-io-syntax",
    "defsystem", "define-syntax", "define", "begin", "set!", "cond-expand", "let-values", "case-lambda", "named-lambda",
    "defn", "defn-", "def", "fn", "ns", "defrecord", "defprotocol", "defmulti", "doseq", "recur", "try", "catch-all",
    "finally", "require", "import", "use", "when-let", "if-let", "binding", "letfn",
];

pub const CONSTANTS: &[&str] = &["nil", "t", "pi", "#t", "#f", "true", "false", "null"];

/// Common standard functions (used for completion and subtle highlighting).
pub const FUNCTIONS: &[&str] = &[
    "*", "+", "-", "/", "/=", "<", "<=", "=", ">", ">=", "1+", "1-", "abs", "acons", "adjoin", "alpha-char-p", "append",
    "apply", "apropos", "aref", "assoc", "atom", "boundp", "butlast", "car", "cdr", "caar", "cadr", "cdar", "cddr",
    "caddr", "ceiling", "char", "char-code", "char-downcase", "char-upcase", "char=", "character", "class-of",
    "clrhash", "coerce", "compile", "compile-file", "complement", "concatenate", "cons", "consp", "constantly", "copy-list",
    "copy-seq", "copy-tree", "count", "count-if", "decode-universal-time", "delete", "delete-duplicates", "delete-if",
    "describe", "digit-char-p", "directory", "documentation", "dpb", "elt", "endp", "eq", "eql", "equal", "equalp", "error",
    "eval", "evenp", "every", "exp", "expt", "fboundp", "fdefinition", "fifth", "fill", "find", "find-class", "find-if",
    "find-package", "find-symbol", "finish-output", "first", "float", "floor", "fmakunbound", "format", "fourth", "funcall",
    "gensym", "gethash", "hash-table-count", "identity", "integerp", "intern", "intersection", "isqrt", "keywordp", "last",
    "lcm", "ldiff", "length", "list", "list*", "list-length", "listp", "load", "log", "logand", "logior", "logxor",
    "lognot", "make-array", "make-hash-table", "make-instance", "make-list", "make-package", "make-string", "make-symbol",
    "mapc", "mapcan", "mapcar", "mapcon", "maphash", "mapl", "maplist", "max", "member", "member-if", "merge", "min", "minusp",
    "mod", "nconc", "nreverse", "nsubstitute", "nth", "nthcdr", "null", "numberp", "oddp", "open", "package-name",
    "parse-integer", "pathname", "plusp", "position", "position-if", "pprint", "prin1", "prin1-to-string", "princ",
    "princ-to-string", "print", "probe-file", "proclaim", "provide", "random", "rassoc", "rational", "read",
    "read-char", "read-from-string", "read-line", "reduce", "rem", "remhash", "remove", "remove-duplicates", "remove-if",
    "remove-if-not", "replace", "require", "rest", "reverse", "room", "round", "rplaca", "rplacd", "search", "second",
    "set", "set-difference", "seventh", "signal", "signum", "sin", "cos", "tan", "sleep", "some", "sort", "stable-sort", "sqrt",
    "stringp", "string", "string-downcase", "string-upcase", "string=", "string<", "subseq", "subsetp", "substitute", "symbol-function",
    "symbol-name", "symbol-package", "symbol-plist", "symbol-value", "symbolp", "tenth", "terpri", "third", "truncate",
    "type-of", "typep", "union", "values", "values-list", "variable", "vector", "vector-pop", "vector-push",
    "vector-push-extend", "warn", "write", "write-char", "write-line", "write-string", "write-to-string", "zerop",
    "slot-value", "slot-boundp", "make-condition", "invoke-restart", "invoke-debugger", "break", "continue", "abort",
    "print-object", "initialize-instance", "shared-initialize", "call-next-method", "next-method-p", "ensure-generic-function",
    "map", "map-into", "mismatch", "notany", "notevery", "nunion", "nintersection", "pairlis", "sublis", "subst",
    "tree-equal", "getf", "get", "remprop", "char-name", "name-char", "string-trim", "string-left-trim", "string-right-trim",
    "parse-namestring", "merge-pathnames", "namestring", "file-length", "file-position", "close", "ensure-directories-exist",
    "get-universal-time", "get-internal-real-time", "machine-type", "lisp-implementation-type", "lisp-implementation-version",
    "map-tree", "mapcar*", "println", "str", "inc", "dec", "conj", "assoc-in", "update", "swap!", "deref", "atom",
    "display", "newline", "vector-ref", "string-append", "list-ref", "hash-ref", "for-each", "call/cc",
];

fn set(cell: &'static OnceLock<HashSet<&'static str>>, items: &'static [&'static str]) -> &'static HashSet<&'static str> {
    cell.get_or_init(|| items.iter().copied().collect())
}

pub fn is_keyword(name: &str) -> bool {
    static S: OnceLock<HashSet<&'static str>> = OnceLock::new();
    set(&S, KEYWORDS).contains(strip_package(name).to_ascii_lowercase().as_str())
}

pub fn is_constant(name: &str) -> bool {
    static S: OnceLock<HashSet<&'static str>> = OnceLock::new();
    set(&S, CONSTANTS).contains(strip_package(name).to_ascii_lowercase().as_str())
}

pub fn is_builtin_function(name: &str) -> bool {
    static S: OnceLock<HashSet<&'static str>> = OnceLock::new();
    set(&S, FUNCTIONS).contains(strip_package(name).to_ascii_lowercase().as_str())
}

/// `cl:car` -> `car`, `sb-ext:*posix-argv*` -> `*posix-argv*`.
pub fn strip_package(name: &str) -> &str {
    match name.rfind(':') {
        Some(i) if i + 1 < name.len() && i > 0 => &name[i + 1..],
        _ => name,
    }
}

/// `*special*` variables.
pub fn is_special_variable(name: &str) -> bool {
    name.len() > 2 && name.starts_with('*') && name.ends_with('*')
}

/// `+constant+` names.
pub fn is_constant_name(name: &str) -> bool {
    name.len() > 2 && name.starts_with('+') && name.ends_with('+')
}

/// Static completion candidates for `prefix` (case-insensitive prefix match), sorted.
pub fn complete(prefix: &str) -> Vec<&'static str> {
    let p = prefix.to_ascii_lowercase();
    let mut v: Vec<&'static str> = KEYWORDS.iter().chain(FUNCTIONS.iter()).chain(CONSTANTS.iter()).copied().filter(|s| s.starts_with(&p)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        assert!(is_keyword("defun"));
        assert!(is_keyword("CL:DEFUN"));
        assert!(!is_keyword("my-function"));
        assert!(is_builtin_function("mapcar"));
        assert!(is_constant("NIL"));
        assert!(is_special_variable("*standard-output*"));
        assert!(is_constant_name("+pi+"));
        assert!(!is_special_variable("*"));
    }

    #[test]
    fn completion_is_sorted_and_unique() {
        let c = complete("def");
        assert!(c.contains(&"defun") && c.contains(&"defmacro") && c.contains(&"defstruct"));
        assert!(c.windows(2).all(|w| w[0] < w[1]));
        assert!(complete("zzzz").is_empty());
    }
}
