# AirLisp

<img src="assets/icon/airlisp-128.png" width="96" align="right" alt="AirLisp icon">

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
* **Local commits** - snapshots of your working tree kept outside of git (`.airlisp/`, excluded through
  `.git/info/exclude`). Made for working on top of somebody else's repository without a fork: create a local
  commit and the diff, gutter markers and changes list start from it; switch the comparison base between `HEAD`
  and any local commit, restore or delete checkpoints. Works in projects without git too.
* **Big projects** - tested on 110 000 files and a 6 MB source file: `.gitignore`-aware tree (ignored entries are
  dimmed, excluded from search and indexing), cached outline and line index, background search and git status.
* **Terminal, folding, run configurations** - a built-in terminal, code folding of forms (Ctrl+-/Ctrl+=),
  load/test ASDF systems from the run configuration picker.
* **No manual saving** - files are saved automatically a second after the last edit, when the window loses focus, before running, when a tab is closed and on exit (Ctrl+S still works). New files without a project are scratch files kept in the settings folder.
* **Branch graph** - the Git log draws all branches as coloured lanes with merges and branch/tag labels.
* **Code overview** - a minimap next to the scrollbar shows the whole file with changed lines, problems and search hits; click or drag to scroll.
* **Projects** - opening a project while another is open asks: this window or a new window.
* **Get from Version Control** - File > Get from Version Control (or the welcome screen): paste a GitHub URL or `owner/repo`,
  pick a folder, optional branch / shallow clone, live progress; the repository is cloned and opened as a project.
* **Works out of the box** - the distribution (`python tools/package.py`) bundles SBCL (public domain/BSD, license included),
  so the REPL and Run work without installing anything.
* **Modern look** - borderless "glass" design: tonal greys over a soft gradient, floating rounded cards, translucent dialogs,
  Lucide icons (ISC), a sidebar Settings dialog. Dark/light themes, custom window frame (no OS title bar), Inter + JetBrains Mono fonts.
* **Keymap** - IntelliJ-style. All key bindings come from one registry (`crates/ide-core/src/commands.rs`); see Settings > Keymap.

## GitHub: your repositories in "Get from Version Control"

The **GitHub** tab of File > Get from Version Control lists your repositories (private, collaborator and organization
ones included, newest push first) with search, fork / archived switches, language, stars and age; a click picks one,
a double click clones it into the chosen folder and opens it as a project. Signing in works like in PyCharm:

* **Sign in in the browser** - Git Credential Manager (part of Git for Windows) opens GitHub's own authorization page;
  you sign in there, two-factor included, and the account lands in Git's credential store. AirLisp never sees your
  password and has no OAuth secret of its own.
* **Use the saved login** - the GitHub CLI's (`gh auth token`) or the one Git already saved for github.com.
* **Token** - paste a personal access token (scope `repo` for private repositories); optionally remember it in Git's
  credential store. **Public repositories of a user** need no sign-in at all.

Requests go through `curl` (no HTTP library is linked); the token reaches `curl` through its config on stdin and `git`
through environment variables of that one clone, and only for `https://github.com/...` URLs. Nothing is written to
AirLisp's own files except the last login name. For the UI tests `AIRLISP_GITHUB_API`, `AIRLISP_FAKE_GCM` and
`tests/fake_github.py` / `tests/fake_gcm.py` replace GitHub and Git Credential Manager.

## Linux on Windows (WSL)

Settings > **Linux (WSL)** runs SBCL, the REPL, Run/Compile and the terminal inside a Linux distribution of the
Windows Subsystem for Linux - Linux without managing a virtual machine. The page checks what is there and walks
through the setup, each step one button:

1. **Install WSL** (`wsl --install --no-distribution`; Windows asks for administrator rights and may want a restart).
2. **Install a distribution** - Ubuntu 24.04 LTS by default or any from `wsl --list --online`, optionally into a folder
   on another drive (free space is shown); or **from a file**: a `.wsl` image or a root file system `.tar`.
   An Ubuntu *desktop* `.iso` is an installer for a PC or a VM and is refused with an explanation.
3. **Install SBCL** inside it (as root, with apt, dnf, zypper, pacman or apk).
4. **Use it.** The status bar then shows `Linux: <distro>`; Settings > Linux (WSL) > Windows switches back.

Your project stays where it is: Windows paths are translated (`C:\x` -> `/mnt/c/x`, `\\wsl.localhost\Ubuntu\home`
-> `/home`) for everything handed to Lisp, and paths Lisp reports (go to definition) come back. Processes started
in Linux are tagged so Stop really ends them. AirLisp never removes or changes an installed distribution.
For the UI tests `AIRLISP_FAKE_WSL` (see `tests/fake_wsl.py`) replaces `wsl.exe`, so they never touch the real WSL.

### Linux projects (C, assembly, make - e.g. an operating system like os-pi)

A project can be *built and run in Linux* while you work on Windows:

* **Choose it per project.** Opening a project with a makefile (or shell scripts) offers "Use Linux"; the status
  bar shows `Windows` / `Linux: <distro>` and switches with a click. The choice is kept in `.airlisp/project.json`,
  which git never sees. The terminal, the make targets, Lisp and the REPL of that project then run in the distribution.
* **Or keep the project in Linux.** Get from Version Control > *Linux: <distro>* clones with the distribution's own git
  into `~/projects/<name>` and opens it as `\\wsl.localhost\<distro>\home\...`; git for such a project always runs
  inside Linux (Linux file modes, symlinks and line endings, no "dubious ownership"). Cloning into a Windows folder
  with *Build and run it in Linux* keeps Linux line endings (`core.autocrlf=false`) so scripts and makefiles work.
* **make targets are run configurations** (`make all`, `make run`, `make debug`, `make clean`, test targets...).
  The output goes to the Run tool window; gcc, ld and nasm errors land in Problems and in the editor, with
  `make -C` sub-directories resolved. `make run` starts QEMU in its own window (WSLg).
* **Build tools in one click.** A project built with make whose distribution lacks them shows a banner with
  *Install build tools* (and `make`/`command not found` in the terminal or Run says why). Settings > Linux (WSL) >
  *Linux projects* checks make, gcc (with 32-bit support), nasm, gdb, git and QEMU and installs what is missing,
  with a progress bar; *Set up git* gives Linux git your name, e-mail and the
  Windows credential manager (GitHub logins).
* **C, assembly, shell and makefiles are not Lisp**: they get their own highlighting, bracket matching, brace
  indentation, folding and comment markers, and never show false Lisp errors.

## Copying from the consoles

The terminal, REPL and Run output are selectable: drag with the mouse (dragging past the edge scrolls), double click
a word or path, triple click a line, **Ctrl+A** selects everything, **Ctrl+C** or right-click > *Copy* copies,
*Copy all* takes the whole log; Esc lets go. After copying, the terminal's command line has the keys again.

## Layout

| crate / app | role |
|---|---|
| `crates/lisp-syntax` | lexer, bracket matching, indentation, structural edits, outline, highlighting (no deps) |
| `crates/ide-core` | editor model, command registry + keymap, project model, fuzzy/text search, symbol index, settings |
| `crates/lisp-runtime` | SBCL discovery, REPL protocol (`lisp/repl-server.lisp`), file runner, compiler diagnostics |
| `apps/airlisp` | the GUI (eframe/egui) incl. the JSON automation harness |
| `apps/airlisp-cli` | headless engine: `check`, `fmt`, `outline`, `eval`, `run`, `compile`, `exec`, `commands`, ... |

## Build & run

```
cargo run --release -p airlisp            # the IDE
cargo run --release -p airlisp-cli -- help
```

Requires Rust (stable) and, for the REPL/run features, SBCL (bundled in packaged builds; otherwise on `PATH`
or set `AIRLISP_SBCL` / Settings > Lisp & REPL). Linux needs the usual windowing libs
(`libxkbcommon`, `libgl`, wayland/x11 headers).

## Tests

```
cargo test --workspace          # unit + integration tests (REPL tests use a real SBCL when installed)
python tests/run_e2e.py         # drives the real window with scripts in tests/e2e/*.json
```

The e2e harness (`--script file.json`) injects keyboard/mouse events through egui's input path,
asserts on application state and takes screenshots *of the app window only*.

## Dependencies

Only `eframe`/`egui` (emilk), `rfd`, `serde`/`serde_json`, `ignore`, `dirs`, `png`. Bundled fonts: JetBrains Mono and Inter (SIL OFL 1.1, see `assets/fonts`); icons: Lucide (ISC, `assets/icons`). Every crate in
`Cargo.lock` was checked against crates.io (publisher, downloads, yanked status, repository);
`deny.toml` pins the policy (crates.io only, no git sources).

## Русский

AirLisp - IDE для Lisp в стиле JetBrains (PyCharm/WebStorm), написанная на Rust. Структурное
редактирование, живой REPL на SBCL, запуск и компиляция с привязкой ошибок к коду, навигация,
рефакторинг (rename), тёмная и светлая темы и привычная раскладка клавиш IntelliJ.
Сборка: `cargo run --release -p airlisp`. Для REPL нужен SBCL в `PATH`.

**Репозитории GitHub.** Во вкладке GitHub диалога «Получить из системы контроля версий» виден список ваших
репозиториев (в том числе приватных): поиск, значки форков и архивов, язык, звёзды; клик выбирает, двойной клик
клонирует и открывает проект. Вход, как в PyCharm: в браузере откроется страница авторизации GitHub (через Git
Credential Manager), пароль AirLisp не видит. Есть также вход сохранённым логином и по токену.

**Linux без виртуальной машины.** В Настройки > Linux (WSL) можно по шагам установить WSL, дистрибутив (Ubuntu 24.04
по умолчанию, или из файла `.wsl` / `.tar`) и SBCL внутри него, а затем одной кнопкой перевести SBCL, REPL, запуск,
компиляцию и терминал в Linux. Файлы проекта остаются на месте: пути переводятся автоматически. Образ Ubuntu Desktop
`.iso` для WSL не подходит: это установщик для ПК или виртуальной машины.

**Linux-проекты (C, ассемблер, make — например, ОС вроде os-pi).** Проект можно собирать и запускать в Linux, работая в
Windows: при открытии проекта с makefile AirLisp предложит «Использовать Linux», а в строке состояния можно
переключить «Windows / Linux: <дистрибутив>» (выбор хранится в `.airlisp/project.json`, git его не видит). Терминал,
цели make, Lisp и REPL проекта работают в дистрибутиве; цели make (`all`, `run`, `debug`, `clean`, тесты) — это
конфигурации запуска, ошибки gcc/ld/nasm попадают в Problems и в редактор. Клонировать можно прямо в Linux
(`~/projects`, git дистрибутива) или в папку Windows с окончаниями строк Linux. На странице Linux (WSL) одной кнопкой
ставятся make, gcc с 32-битной поддержкой, nasm, gdb, git и QEMU, а git в Linux настраивается на ваши имя, почту и вход
в GitHub из Windows. Файлы C, ассемблера, shell и makefile подсвечиваются по-своему и не дают ложных Lisp-ошибок.
Если в дистрибутиве нет этих инструментов, сверху появляется плашка «Установить инструменты сборки», а `make` в терминале
или в Run объясняет причину (`make: Permission denied`, код 127 значит «не установлен»), с полосой прогресса установки.

**Копирование из консолей.** Текст в терминале, REPL и окне Run можно выделять мышью (протяжка за край прокручивает),
двойной клик — слово или путь, тройной — строка, Ctrl+A — всё, Ctrl+C или правая кнопка > «Копировать» копирует,
«Копировать всё» берёт весь журнал, Esc снимает выделение.
