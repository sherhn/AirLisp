//! `lispcraft-cli`: the IDE engine without a window. Useful for scripts, CI and automated tests.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use ide_core::commands::COMMANDS;
use ide_core::editor::Editor;
use lisp_runtime::{compiler, discover, runner, ReplSession};
use lisp_syntax::outline::outline;
use lisp_syntax::{Dialect, Severity, indent, parse};

const USAGE: &str = "\
lispcraft-cli - headless Lispcraft engine

USAGE:
  lispcraft-cli <command> [args]

COMMANDS:
  check <file>              report unbalanced brackets and other syntax problems (exit 1 on errors)
  fmt <file> [--write]      print the reformatted file (or rewrite it with --write); --check only verifies
  outline <file>            list top-level definitions
  eval <expr>               evaluate in a fresh REPL session and print the values
  run <file>                run a file with the Lisp implementation
  compile <file>            compile with the real compiler and list problems
  doc <symbol>              ask the live image about a symbol (arglist and description)
  complete <prefix>         completions from the live image
  exec <file> <cmd-id>...   apply editor commands to a file and print the result (see 'commands')
  commands                  list all registered commands with their shortcuts
  new-project <dir> <name>  scaffold an ASDF project
  which                     show the detected Lisp implementation
  help                      this text
";

fn read(path: &str) -> Result<(String, Dialect), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let d = Dialect::from_path(Path::new(path)).unwrap_or_default();
    Ok((text, d))
}

fn lisp() -> Result<lisp_runtime::Implementation, String> {
    discover::find_sbcl("").ok_or_else(|| "no Lisp implementation found (install SBCL or set LISPCRAFT_SBCL)".to_string())
}

fn line_col(text: &str, off: usize) -> (usize, usize) {
    let before = &text[..off.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some(cmd) = args.first() else {
        print!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
        }
        "which" => {
            let i = lisp()?;
            println!("{} at {} ({})", i.name, i.path.display(), i.version().unwrap_or_default());
        }
        "check" => {
            let file = rest.first().ok_or("usage: check <file>")?;
            let (text, d) = read(file)?;
            let p = parse(&text, d);
            for diag in &p.diagnostics {
                let (l, c) = line_col(&text, diag.start);
                let sev = if diag.severity == Severity::Error { "error" } else { "warning" };
                println!("{file}:{l}:{c}: {sev}: {}", diag.message);
            }
            if p.diagnostics.iter().any(|d| d.severity == Severity::Error) {
                return Ok(ExitCode::from(1));
            }
            println!("{file}: OK ({} tokens)", p.tokens.len());
        }
        "fmt" => {
            let file = rest.first().ok_or("usage: fmt <file> [--write|--check]")?;
            let (text, d) = read(file)?;
            let out = indent::format(&text, d);
            if rest.iter().any(|a| a == "--check") {
                if out != text {
                    eprintln!("{file}: needs formatting");
                    return Ok(ExitCode::from(1));
                }
                println!("{file}: already formatted");
            } else if rest.iter().any(|a| a == "--write") {
                if out != text {
                    std::fs::write(file, &out).map_err(|e| e.to_string())?;
                    println!("{file}: reformatted");
                } else {
                    println!("{file}: unchanged");
                }
            } else {
                print!("{out}");
            }
        }
        "outline" => {
            let file = rest.first().ok_or("usage: outline <file>")?;
            let (text, d) = read(file)?;
            for def in outline(&text, d) {
                println!("{:>5}  {:<9} {}", def.line + 1, def.kind.label(), def.name);
            }
        }
        "eval" => {
            let expr = rest.join(" ");
            if expr.trim().is_empty() {
                return Err("usage: eval <expr>".into());
            }
            let s = ReplSession::start(&lisp()?, None).map_err(|e| e.to_string())?;
            s.wait_ready(Duration::from_secs(60))?;
            let r = s.eval_sync(&expr, Duration::from_secs(120))?;
            print!("{}", r.output);
            if let Some(v) = &r.values {
                println!("{v}");
            }
            if let Some(e) = &r.error {
                eprintln!("error: {e}");
                return Ok(ExitCode::from(1));
            }
        }
        "doc" | "complete" => {
            let arg = rest.first().ok_or("missing argument")?;
            let s = ReplSession::start(&lisp()?, None).map_err(|e| e.to_string())?;
            s.wait_ready(Duration::from_secs(60))?;
            let t = Duration::from_secs(30);
            if cmd == "doc" {
                println!("{}", s.query_sync("arglist", arg, t)?);
                println!("{}", s.query_sync("doc", arg, t)?);
            } else {
                for l in s.query_sync("complete", arg, t)?.lines() {
                    println!("{}", l.replace('\t', "  "));
                }
            }
        }
        "run" => {
            let file = rest.first().ok_or("usage: run <file>")?;
            let h = runner::run_file(&lisp()?, &PathBuf::from(file), None).map_err(|e| e.to_string())?;
            let (out, code, timed_out) = h.wait_output(Duration::from_secs(600));
            print!("{out}");
            if timed_out {
                return Err("timed out".into());
            }
            return Ok(ExitCode::from(code.unwrap_or(1).clamp(0, 255) as u8));
        }
        "compile" => {
            let file = rest.first().ok_or("usage: compile <file>")?;
            let (text, d) = read(file)?;
            let r = compiler::compile_sync(&lisp()?, &PathBuf::from(file), Duration::from_secs(300)).map_err(|e| e.to_string())?;
            for p in &r.problems {
                let (l, c) = compiler::locate(p, &text, d).map_or((0, 0), |(s, _)| line_col(&text, s));
                println!("{file}:{l}:{c}: {:?}: {}", p.severity, p.message.lines().next().unwrap_or(""));
            }
            if r.success() {
                println!("{file}: compiled OK ({} problems)", r.problems.len());
            } else {
                return Ok(ExitCode::from(1));
            }
        }
        "exec" => {
            let file = rest.first().ok_or("usage: exec <file> <cmd-id>...")?;
            let (text, d) = read(file)?;
            let mut ed = Editor::new(&text, d);
            for id in &rest[1..] {
                if !ide_core::exec::apply(&mut ed, id) {
                    return Err(format!("command '{id}' is not an editor command"));
                }
            }
            print!("{}", ed.text());
        }
        "commands" => {
            for c in COMMANDS {
                println!("{:<26} {:<10} {:<34} {}", c.id, c.category.title(), c.title, c.shortcuts.join(", "));
            }
        }
        "new-project" => {
            let dir = rest.first().ok_or("usage: new-project <dir> <name>")?;
            let name = rest.get(1).ok_or("usage: new-project <dir> <name>")?;
            let root = ide_core::workspace::create_lisp_project(Path::new(dir), name).map_err(|e| e.to_string())?;
            println!("created {}", root.display());
        }
        other => return Err(format!("unknown command '{other}'. Try 'help'.")),
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
