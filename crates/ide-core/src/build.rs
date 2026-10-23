//! Projects built with make: the targets of the top makefile (for the run configurations) and the compiler messages
//! in make's output (for the Problems tool window).

use std::path::{Path, PathBuf};

/// The makefile make would pick in `dir`, if there is one.
pub fn find_makefile(dir: &Path) -> Option<PathBuf> {
    ["GNUmakefile", "makefile", "Makefile"].iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

/// Targets a person would run: explicit rules of the makefile, in file order, without special targets
/// (`.PHONY`), pattern rules (`%.o`), variable assignments and file targets that are object files.
pub fn targets(makefile: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut continued = false;
    for raw in makefile.lines() {
        let was_continued = continued;
        continued = raw.ends_with('\\');
        if was_continued || raw.starts_with('\t') || raw.trim_start().starts_with('#') {
            continue;
        }
        let line = raw.split('#').next().unwrap_or("");
        let Some(colon) = line.find(':') else { continue };
        let (head, rest) = (&line[..colon], &line[colon..]);
        // `VAR := value`, `VAR ::= value` and `VAR = a:b` are assignments, not rules
        if rest.starts_with(":=") || rest.starts_with("::=") || head.contains('=') || head.trim_start().starts_with("define ") {
            continue;
        }
        for t in head.split_whitespace() {
            let ok = !t.starts_with('.') && !t.contains('%') && !t.contains('$') && !t.ends_with(".o") && !t.ends_with(".d") && !t.is_empty();
            if ok && !out.iter().any(|o| o == t) {
                out.push(t.to_string());
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
    Note,
}

/// A message of gcc, clang, ld or nasm found in make's output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The file as printed (relative paths are relative to `dir`).
    pub file: String,
    /// The directory make was in when the message was printed (`make -C` / "Entering directory"); empty means the
    /// directory the build started in.
    pub dir: String,
    /// 1-based line and column (column 0: not given).
    pub line: usize,
    pub col: usize,
    pub level: Level,
    pub text: String,
}

/// `file:line:col: error: text`, `file:line: warning: text`, nasm's `file:line: error: text`.
fn parse_line(l: &str) -> Option<(String, usize, usize, Level, String)> {
    // the file name may contain a drive letter (C:\...) - skip it when looking for the separators
    let skip = if l.len() > 2 && l.as_bytes()[1] == b':' && l.as_bytes()[0].is_ascii_alphabetic() { 2 } else { 0 };
    let mut parts = l[skip..].splitn(4, ':');
    let file = format!("{}{}", &l[..skip], parts.next()?);
    let line: usize = parts.next()?.trim().parse().ok()?;
    let third = parts.next()?;
    let (col, rest) = match third.trim().parse::<usize>() {
        Ok(c) => (c, parts.next()?.to_string()),
        Err(_) => (0, match parts.next() {
            Some(r) => format!("{third}:{r}"),
            None => third.to_string(),
        }),
    };
    let rest = rest.trim();
    let (level, text) = if let Some(t) = rest.strip_prefix("fatal error:").or_else(|| rest.strip_prefix("error:")) {
        (Level::Error, t)
    } else if let Some(t) = rest.strip_prefix("warning:") {
        (Level::Warning, t)
    } else {
        let t = rest.strip_prefix("note:")?;
        (Level::Note, t)
    };
    if file.trim().is_empty() || file.contains(' ') && !file.contains('/') && !file.contains('\\') {
        return None;
    }
    Some((file.trim().to_string(), line, col, level, text.trim().to_string()))
}

/// Compiler messages in `output`, with the directory make was in for each.
pub fn parse_output(output: &str) -> Vec<Message> {
    let mut dirs: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for l in output.lines() {
        let l = l.trim_end_matches('\r');
        // make[1]: Entering directory '/home/me/os-pi/portable'
        if l.contains(": Entering directory ") {
            if let Some(d) = l.split(['\'', '`']).nth(1) {
                dirs.push(d.trim_end_matches('\'').to_string());
            }
            continue;
        }
        if l.contains(": Leaving directory ") {
            dirs.pop();
            continue;
        }
        if let Some((file, line, col, level, text)) = parse_line(l) {
            out.push(Message { file, dir: dirs.last().cloned().unwrap_or_default(), line, col, level, text });
        }
    }
    out
}

/// Byte offset of 1-based `line`/`col` in `text` (clamped to the text).
pub fn offset_of(text: &str, line: usize, col: usize) -> usize {
    let mut off = 0;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i + 1 == line {
            let within: usize = l.chars().take(col.saturating_sub(1)).map(char::len_utf8).sum();
            return (off + within).min(off + l.trim_end_matches('\n').len());
        }
        off += l.len();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OS_PI: &str = "SUBDIRS = portable x86\n.PHONY: /bin/kernel $(SUBDIRS)\nall: bin/kernel\nbin/kernel: $(SUBDIRS)\n\tld -m elf_i386 -T link.ld -o bin/kernel x86/*.o\n$(SUBDIRS):\n\t$(MAKE) -C $@\nrun: bin/kernel\n\tqemu-system-i386 -m 512M\ndebug:\n\tqemu-system-i386 -s -S\nclean:\n\trm -f bin/kernel\n%.o: %.c\n\tgcc -c $<\nCFLAGS := -m32\nX = a:b\n# test: commented\nfs-test:\n\tcp disk.qcow2 /tmp/disk.qcow2.copy\n";

    #[test]
    fn targets_of_a_kernel_makefile() {
        assert_eq!(targets(OS_PI), ["all", "bin/kernel", "run", "debug", "clean", "fs-test"]);
        assert!(targets("a b: c\n\techo\nlong: x \\\n  y: z\n").starts_with(&["a".to_string(), "b".to_string(), "long".to_string()]));
    }

    #[test]
    fn compiler_messages_with_directories() {
        let out = "make -C portable\nmake[1]: Entering directory '/home/me/os-pi/portable'\ngcc -g -m32 -c eval.c\neval.c:42:7: error: 'x' undeclared (first use in this function)\neval.c:50:1: warning: control reaches end of non-void function [-Wreturn-type]\nIn file included from eval.c:3:\n../include/portable/eval.h:9:5: note: declared here\nmake[1]: Leaving directory '/home/me/os-pi/portable'\nboot.asm:12: error: label or instruction expected at start of line\nld: cannot find x86/start.o: No such file or directory\nC:\\proj\\a.c:3:1: error: oops\nmake: *** [makefile:4: portable] Error 2\n";
        let m = parse_output(out);
        assert_eq!(m.len(), 5, "{m:#?}");
        assert_eq!(m[0], Message { file: "eval.c".into(), dir: "/home/me/os-pi/portable".into(), line: 42, col: 7, level: Level::Error, text: "'x' undeclared (first use in this function)".into() });
        assert_eq!(m[1].level, Level::Warning);
        assert_eq!(m[2].file, "../include/portable/eval.h");
        assert_eq!(m[2].level, Level::Note);
        assert_eq!(m[3], Message { file: "boot.asm".into(), dir: String::new(), line: 12, col: 0, level: Level::Error, text: "label or instruction expected at start of line".into() });
        assert_eq!(m[4].file, "C:\\proj\\a.c");
        assert_eq!((m[4].line, m[4].col), (3, 1));
    }

    #[test]
    fn offsets() {
        let t = "ab\ncdé\nx";
        assert_eq!(offset_of(t, 1, 1), 0);
        assert_eq!(offset_of(t, 2, 3), 5);
        assert_eq!(offset_of(t, 2, 99), 7);
        assert_eq!(offset_of(t, 3, 0), 8);
        assert_eq!(offset_of(t, 9, 1), t.len());
    }

    #[test]
    fn makefile_lookup() {
        let d = crate::testutil::TempDir::new("build-mk");
        assert_eq!(find_makefile(d.path()), None);
        std::fs::write(d.path().join("makefile"), "all:\n").unwrap();
        assert_eq!(find_makefile(d.path()).unwrap().file_name().unwrap().to_ascii_lowercase(), "makefile");
    }
}
