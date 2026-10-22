//! Files that are not Lisp: C and C headers, assembly, shell scripts, makefiles, linker scripts and plain text.
//!
//! A Lisp project for an operating system (os-pi and the like) is mostly C, assembly and makefiles. These get a
//! small lexer of their own: highlighting, bracket matching that knows about strings and comments, and the line
//! comment marker - but no Lisp reader, so a `;` in C or a `#include` never shows up as a Lisp error.

use std::path::Path;

use crate::highlight::{Class, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// C, C++, headers, linker scripts: `//` and `/* */` comments, braces.
    C,
    /// NASM/FASM style assembly: `;` comments.
    Nasm,
    /// GNU assembler (`.s`, `.S`): `#`, `//` and `/* */` comments.
    GnuAsm,
    /// Shell scripts: `#` comments.
    Shell,
    /// Makefiles: `#` comments, targets and `$(VARIABLES)`.
    Make,
    /// Anything else: no highlighting.
    Plain,
}

/// The language of a file that is not Lisp; `None` for Lisp files.
pub fn lang_of(path: &Path) -> Option<Lang> {
    if crate::dialect::is_lisp_file(path) {
        return None;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if matches!(name.as_str(), "makefile" | "gnumakefile") || name.ends_with(".mk") || name.ends_with(".mak") {
        return Some(Lang::Make);
    }
    let ext = path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    Some(match ext.to_ascii_lowercase().as_str() {
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "ld" | "lds" | "java" | "js" | "ts" | "cs" | "go" | "rs" | "glsl" => Lang::C,
        "asm" | "nasm" | "inc" | "fasm" => Lang::Nasm,
        "s" => Lang::GnuAsm,
        "sh" | "bash" | "zsh" | "ksh" | "bashrc" | "profile" | "py" | "rb" | "pl" | "yml" | "yaml" | "toml" | "conf" | "cfg" | "ini" | "cmake" | "dockerfile" | "gitignore" => Lang::Shell,
        _ => Lang::Plain,
    })
}

impl Lang {
    /// What "Comment Line" puts in front of a line.
    pub fn line_comment(self) -> &'static str {
        match self {
            Lang::C => "//",
            Lang::Nasm => ";",
            Lang::GnuAsm | Lang::Shell | Lang::Make | Lang::Plain => "#",
        }
    }

    /// Blocks are delimited by braces, so Enter after `{` indents.
    pub fn braces(self) -> bool {
        self == Lang::C
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::C => "C",
            Lang::Nasm => "Assembly (NASM)",
            Lang::GnuAsm => "Assembly (GNU)",
            Lang::Shell => "Shell",
            Lang::Make => "Makefile",
            Lang::Plain => "Plain text",
        }
    }
}

const C_KEYWORDS: &[&str] = &[
    "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return", "goto", "sizeof", "typedef", "struct", "union", "enum",
    "static", "extern", "const", "volatile", "inline", "register", "restrict", "asm", "__asm__", "__attribute__", "__volatile__", "auto", "class",
    "namespace", "template", "public", "private", "protected", "new", "delete", "fn", "let", "mut", "impl", "use", "pub", "match", "loop", "func",
    "var", "import", "package", "function", "this", "try", "catch", "throw",
];
const C_TYPES: &[&str] = &[
    "void", "char", "short", "int", "long", "float", "double", "signed", "unsigned", "bool", "_Bool", "size_t", "ssize_t", "uint8_t", "uint16_t",
    "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t", "uintptr_t", "intptr_t", "FILE", "String", "u8", "u16", "u32", "u64", "i32", "i64",
];
const C_CONSTANTS: &[&str] = &["NULL", "true", "false", "nullptr", "TRUE", "FALSE", "None", "null", "undefined"];
const SHELL_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac", "in", "function", "return", "export", "local", "readonly",
    "set", "unset", "shift", "exit", "source", "alias", "echo", "cd", "def", "class", "import", "from", "and", "or", "not",
];
const MAKE_KEYWORDS: &[&str] = &["ifeq", "ifneq", "ifdef", "ifndef", "else", "endif", "include", "-include", "sinclude", "define", "endef", "export", "unexport", "override", "vpath"];
const ASM_DIRECTIVES: &[&str] = &[
    "section", "segment", "global", "extern", "bits", "use16", "use32", "use64", "org", "align", "db", "dw", "dd", "dq", "dt", "resb", "resw", "resd", "resq",
    "times", "equ", "%define", "%macro", "%endmacro", "%include", "%if", "%ifdef", "%else", "%endif", "incbin", "struc", "endstruc", "istruc", "iend",
];
const ASM_REGISTERS: &[&str] = &[
    "eax", "ebx", "ecx", "edx", "esi", "edi", "esp", "ebp", "ax", "bx", "cx", "dx", "si", "di", "sp", "bp", "al", "ah", "bl", "bh", "cl", "ch", "dl", "dh",
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rsp", "rbp", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15", "cs", "ds", "es", "fs", "gs", "ss",
    "cr0", "cr2", "cr3", "cr4", "eip", "rip", "eflags",
];

fn ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

/// Highlighting spans for `text` in `lang` (sorted, non-overlapping; plain text is left out).
pub fn highlight(text: &str, lang: Lang) -> Vec<Span> {
    if lang == Lang::Plain {
        return vec![];
    }
    let b = text.as_bytes();
    let mut out: Vec<Span> = Vec::new();
    let mut i = 0;
    let mut depth: u32 = 0;
    let mut line_start = true;
    let push = |out: &mut Vec<Span>, start: usize, end: usize, class: Class, depth: u32| {
        if end > start {
            out.push(Span { start, end, class, depth });
        }
    };
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' {
            i += 1;
            continue;
        }
        let at_line_start = line_start;
        line_start = false;
        let rest = &b[i..];
        // ---- comments
        let line_comment = match lang {
            Lang::C => rest.starts_with(b"//"),
            Lang::Nasm => c == b';',
            Lang::GnuAsm => c == b'#' || rest.starts_with(b"//"),
            Lang::Shell | Lang::Make => c == b'#' && (i == 0 || !ident_char(b[i - 1]) && b[i - 1] != b'$'),
            Lang::Plain => false,
        };
        // a C preprocessor line is not a comment
        let preprocessor = matches!(lang, Lang::C) && c == b'#' && at_line_start;
        if line_comment && !preprocessor {
            let end = text[i..].find('\n').map_or(b.len(), |e| i + e);
            push(&mut out, i, end, Class::Comment, 0);
            i = end;
            continue;
        }
        if matches!(lang, Lang::C | Lang::GnuAsm) && rest.starts_with(b"/*") {
            let end = text[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2);
            push(&mut out, i, end, Class::Comment, 0);
            i = end;
            continue;
        }
        if preprocessor {
            // `#include <x.h>`, `#define NAME value`: the directive itself
            let mut j = i + 1;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                j += 1;
            }
            while j < b.len() && ident_char(b[j]) {
                j += 1;
            }
            push(&mut out, i, j, Class::Prefix, 0);
            i = j;
            if text[i..].trim_start_matches([' ', '\t']).starts_with('<') {
                let s = i + text[i..].find('<').unwrap_or(0);
                let end = text[s..].find(['>', '\n']).map_or(b.len(), |e| s + e + 1);
                push(&mut out, s, end.min(b.len()), Class::String, 0);
                i = end.min(b.len());
            }
            continue;
        }
        // ---- strings and characters
        if c == b'"' || (c == b'\'' && lang != Lang::Make) || (c == b'`' && lang == Lang::Shell) {
            let q = c;
            let mut j = i + 1;
            while j < b.len() && b[j] != q && b[j] != b'\n' {
                if b[j] == b'\\' && lang != Lang::Shell {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(b.len());
            push(&mut out, i, end, if q == b'\'' && matches!(lang, Lang::C) { Class::Char } else { Class::String }, 0);
            i = end;
            continue;
        }
        // ---- variables: $(VAR) ${VAR} $VAR $@
        if c == b'$' && matches!(lang, Lang::Shell | Lang::Make) {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'(' || b[j] == b'{') {
                let close = if b[j] == b'(' { b')' } else { b'}' };
                while j < b.len() && b[j] != close && b[j] != b'\n' {
                    j += 1;
                }
                j = (j + 1).min(b.len());
            } else if j < b.len() && !ident_char(b[j]) {
                j += 1;
            } else {
                while j < b.len() && ident_char(b[j]) {
                    j += 1;
                }
            }
            push(&mut out, i, j, Class::SpecialVar, 0);
            i = j;
            continue;
        }
        // ---- brackets
        if matches!(c, b'(' | b'[' | b'{') {
            push(&mut out, i, i + 1, Class::Bracket, depth);
            depth += 1;
            i += 1;
            continue;
        }
        if matches!(c, b')' | b']' | b'}') {
            depth = depth.saturating_sub(1);
            push(&mut out, i, i + 1, Class::Bracket, depth);
            i += 1;
            continue;
        }
        // ---- numbers
        if c.is_ascii_digit() {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') {
                j += 1;
            }
            push(&mut out, i, j, Class::Number, 0);
            i = j;
            continue;
        }
        // ---- words
        if ident_char(c) || c == b'.' || c == b'%' || (c == b'-' && lang == Lang::Make && at_line_start) {
            let mut j = i + 1;
            while j < b.len() && (ident_char(b[j]) || (matches!(lang, Lang::Make | Lang::Shell) && matches!(b[j], b'-' | b'.' | b'/'))) {
                j += 1;
            }
            let word = &text[i..j];
            let next = text[j..].trim_start_matches([' ', '\t']);
            let class = match lang {
                Lang::C => {
                    if C_KEYWORDS.contains(&word) {
                        Some(Class::SpecialForm)
                    } else if C_TYPES.contains(&word) {
                        Some(Class::Builtin)
                    } else if C_CONSTANTS.contains(&word) {
                        Some(Class::Constant)
                    } else if next.starts_with('(') && depth == 0 {
                        Some(Class::Definition)
                    } else if word.len() > 1 && word.bytes().all(|x| x.is_ascii_uppercase() || x.is_ascii_digit() || x == b'_') && word.bytes().any(|x| x.is_ascii_uppercase()) {
                        Some(Class::ConstName)
                    } else {
                        None
                    }
                }
                Lang::Nasm | Lang::GnuAsm => {
                    let lower = word.to_ascii_lowercase();
                    if next.starts_with(':') && at_line_start {
                        Some(Class::Definition)
                    } else if ASM_DIRECTIVES.contains(&lower.as_str()) || word.starts_with('.') || word.starts_with('%') {
                        Some(Class::SpecialForm)
                    } else if ASM_REGISTERS.contains(&lower.as_str()) {
                        Some(Class::Builtin)
                    } else if at_line_start {
                        Some(Class::Keyword)
                    } else {
                        None
                    }
                }
                Lang::Shell => SHELL_KEYWORDS.contains(&word).then_some(Class::SpecialForm),
                Lang::Make => {
                    if MAKE_KEYWORDS.contains(&word) {
                        Some(Class::SpecialForm)
                    } else if at_line_start && next.starts_with(':') && !next.starts_with(":=") {
                        Some(Class::Definition)
                    } else if at_line_start && (next.starts_with('=') || next.starts_with(":=") || next.starts_with("?=") || next.starts_with("+=")) {
                        Some(Class::SpecialVar)
                    } else {
                        None
                    }
                }
                Lang::Plain => None,
            };
            if let Some(cl) = class {
                push(&mut out, i, j, cl, 0);
            }
            i = j;
            continue;
        }
        // anything else (operators, punctuation): one character, or a whole UTF-8 sequence
        i += text[i..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

/// The bracket matching the one at or just before `off`, skipping strings and comments.
pub fn matching_bracket(text: &str, lang: Lang, off: usize) -> Option<(usize, usize)> {
    let spans = highlight(text, lang);
    let brackets: Vec<usize> = spans.iter().filter(|s| s.class == Class::Bracket).map(|s| s.start).collect();
    let b = text.as_bytes();
    let at = |o: usize| brackets.binary_search(&o).is_ok();
    let here = if off < b.len() && at(off) {
        off
    } else if off > 0 && at(off - 1) {
        off - 1
    } else {
        return None;
    };
    let idx = brackets.binary_search(&here).ok()?;
    let open = |c: u8| matches!(c, b'(' | b'[' | b'{');
    let partner = |c: u8| match c {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b')' => b'(',
        b']' => b'[',
        _ => b'{',
    };
    let c = b[here];
    let mut level = 0i32;
    if open(c) {
        for &p in &brackets[idx + 1..] {
            if b[p] == c {
                level += 1;
            } else if b[p] == partner(c) {
                if level == 0 {
                    return Some((here, p));
                }
                level -= 1;
            }
        }
    } else {
        for &p in brackets[..idx].iter().rev() {
            if b[p] == c {
                level += 1;
            } else if b[p] == partner(c) {
                if level == 0 {
                    return Some((p, here));
                }
                level -= 1;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes(text: &str, lang: Lang) -> Vec<(String, Class)> {
        highlight(text, lang).into_iter().map(|s| (text[s.start..s.end].to_string(), s.class)).collect()
    }

    #[test]
    fn languages_by_file_name() {
        assert_eq!(lang_of(Path::new("kernel/main.c")), Some(Lang::C));
        assert_eq!(lang_of(Path::new("include/x86/idt.h")), Some(Lang::C));
        assert_eq!(lang_of(Path::new("link.ld")), Some(Lang::C));
        assert_eq!(lang_of(Path::new("x86/boot.asm")), Some(Lang::Nasm));
        assert_eq!(lang_of(Path::new("x86/start.S")), Some(Lang::GnuAsm));
        assert_eq!(lang_of(Path::new("makefile")), Some(Lang::Make));
        assert_eq!(lang_of(Path::new("boot/common.mak")), Some(Lang::Make));
        assert_eq!(lang_of(Path::new("tools/run.sh")), Some(Lang::Shell));
        assert_eq!(lang_of(Path::new("README.md")), Some(Lang::Plain));
        assert_eq!(lang_of(Path::new("portable/lisp/boot/test.lsp")), None);
        assert_eq!(lang_of(Path::new("src/main.lisp")), None);
    }

    #[test]
    fn c_semicolons_and_includes_are_not_lisp() {
        let src = "#include <stdio.h>\n// note; really\nint main(void) {\n  char c = ';';\n  return NULL == 0; /* (x */\n}\n";
        let cl = classes(src, Lang::C);
        assert!(cl.contains(&("#include".into(), Class::Prefix)));
        assert!(cl.contains(&("<stdio.h>".into(), Class::String)));
        assert!(cl.contains(&("// note; really".into(), Class::Comment)));
        assert!(cl.contains(&("int".into(), Class::Builtin)));
        assert!(cl.contains(&("main".into(), Class::Definition)));
        assert!(cl.contains(&("';'".into(), Class::Char)));
        assert!(cl.contains(&("return".into(), Class::SpecialForm)));
        assert!(cl.contains(&("NULL".into(), Class::Constant)));
        assert!(cl.contains(&("/* (x */".into(), Class::Comment)));
        // the bracket inside the comment does not count: `{` matches the last `}`
        let open = src.find('{').unwrap();
        assert_eq!(matching_bracket(src, Lang::C, open), Some((open, src.rfind('}').unwrap())));
    }

    #[test]
    fn assembly_and_makefiles() {
        let asm = "section .text\nglobal start\nstart:\n  mov eax, 0x10 ; load\n";
        let cl = classes(asm, Lang::Nasm);
        assert!(cl.contains(&("section".into(), Class::SpecialForm)));
        assert!(cl.contains(&("start".into(), Class::Definition)));
        assert!(cl.contains(&("mov".into(), Class::Keyword)));
        assert!(cl.contains(&("eax".into(), Class::Builtin)));
        assert!(cl.contains(&("0x10".into(), Class::Number)));
        assert!(cl.contains(&("; load".into(), Class::Comment)));
        let mk = "SUBDIRS = portable x86\nall: bin/kernel\n\t$(MAKE) -C $@ # build\n";
        let cl = classes(mk, Lang::Make);
        assert!(cl.contains(&("SUBDIRS".into(), Class::SpecialVar)));
        assert!(cl.contains(&("all".into(), Class::Definition)));
        assert!(cl.contains(&("$(MAKE)".into(), Class::SpecialVar)));
        assert!(cl.contains(&("$@".into(), Class::SpecialVar)));
        assert!(cl.contains(&("# build".into(), Class::Comment)));
        assert!(highlight("anything (at all)", Lang::Plain).is_empty());
    }

    #[test]
    fn comment_markers() {
        assert_eq!(Lang::C.line_comment(), "//");
        assert_eq!(Lang::Nasm.line_comment(), ";");
        assert_eq!(Lang::Make.line_comment(), "#");
        assert!(Lang::C.braces() && !Lang::Shell.braces());
    }
}
