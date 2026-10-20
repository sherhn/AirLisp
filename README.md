# Lispcraft

A JetBrains-style IDE for Lisp, written in Rust (egui) and built the PhotoCraft way:
a UI-independent engine, one command registry that drives menus, palettes and key bindings,
a headless CLI, and end-to-end tests that drive the real window.

Runs on Windows, macOS and Linux. Common Lisp (SBCL) is the first-class target;
Scheme, Clojure and Emacs Lisp files get highlighting, structure and indentation.

## Features

* **Editor** - own text engine: Lisp-aware indentation (SLIME/Emacs rules), auto-pairs and overtyping,
  smart Enter, rainbow brackets, matching-bracket highlight, undo/redo with coalescing, multi-line
  Tab/Shift+Tab, line duplicate/move/join, comment toggle, extend/shrink selection (Ctrl+W).
* **Structural editing** - slurp/barf (both directions), raise, splice, wrap, kill sexp, sexp navigation.
* **Live REPL** - an SBCL image driven over a structured protocol: eval the form under the caret
  (Ctrl+Enter), errors with backtraces, package tracking, completion/arglist/docs/macroexpand/definition
  lookup straight from the running image.
* **Run & compile** - Shift+F10 runs the file, Ctrl+F9 compiles with the real compiler and maps
  warnings/errors back to source ranges (Problems tool window + squiggles + error stripe).
* **Navigation** - Search Everywhere (Double Shift), Go to File/Symbol/Line, Recent Files,
  File Structure, Go to Declaration (Ctrl+B / Ctrl+click), Back/Forward, Find Usages, Find in Files.
* **Refactoring** - Rename symbol across the project (token based: strings and comments untouched).
* **Project** - project tree with file operations, ASDF project scaffolding, git branch in the toolbar.
* **Git** - branch switcher, update/commit/push in the toolbar, commit dialog (Ctrl+K, Ctrl+Enter to commit),
  Git tool window (changes with checkboxes, log), diff viewer, change markers in the gutter, file status colours in
  the tree and tabs, rollback. Uses the `git` executable.
* **Terminal, folding, run configurations** - a built-in terminal, code folding of forms (Ctrl+-/Ctrl+=),
  load/test ASDF systems from the run configuration picker.
* **JetBrains look & keymap** - New UI dark/light themes, tool windows, tabs, breadcrumbs, status bar.
  Custom window frame (no OS title bar), Inter + JetBrains Mono fonts. All key bindings come from one registry (`crates/ide-core/src/commands.rs`); see Settings > Keymap.

## Layout

| crate / app | role |
|---|---|
| `crates/lisp-syntax` | lexer, bracket matching, indentation, structural edits, outline, highlighting (no deps) |
| `crates/ide-core` | editor model, command registry + keymap, project model, fuzzy/text search, symbol index, settings |
| `crates/lisp-runtime` | SBCL discovery, REPL protocol (`lisp/repl-server.lisp`), file runner, compiler diagnostics |
| `apps/lispcraft` | the GUI (eframe/egui) incl. the JSON automation harness |
| `apps/lispcraft-cli` | headless engine: `check`, `fmt`, `outline`, `eval`, `run`, `compile`, `exec`, `commands`, ... |

## Build & run

```
cargo run --release -p lispcraft            # the IDE
cargo run --release -p lispcraft-cli -- help
```

Requires Rust (stable) and, for the REPL/run features, SBCL on `PATH`
(or set `LISPCRAFT_SBCL` / Settings > Lisp). Linux needs the usual windowing libs
(`libxkbcommon`, `libgl`, wayland/x11 headers).

## Tests

```
cargo test --workspace          # unit + integration tests (REPL tests use a real SBCL when installed)
python tests/run_e2e.py         # drives the real window with scripts in tests/e2e/*.json
```

The e2e harness (`--script file.json`) injects keyboard/mouse events through egui's input path,
asserts on application state and takes screenshots *of the app window only*.

## Dependencies

Only `eframe`/`egui` (emilk), `rfd`, `serde`/`serde_json`, `ignore`, `dirs`, `png`. Bundled fonts: JetBrains Mono and Inter (SIL OFL 1.1, see `assets/fonts`). Every crate in
`Cargo.lock` was checked against crates.io (publisher, downloads, yanked status, repository);
`deny.toml` pins the policy (crates.io only, no git sources).

## Русский

Lispcraft - IDE для Lisp в стиле JetBrains (PyCharm/WebStorm), написанная на Rust. Структурное
редактирование, живой REPL на SBCL, запуск и компиляция с привязкой ошибок к коду, навигация,
рефакторинг (rename), тёмная и светлая темы и привычная раскладка клавиш IntelliJ.
Сборка: `cargo run --release -p lispcraft`. Для REPL нужен SBCL в `PATH`.
