# Air

<img src="assets/icon/air-128.png" width="96" align="right" alt="Air icon">

Air is a shovel; the gold is yours to find. An IDE written in Rust (egui) and built the PhotoCraft way:
a UI-independent engine, one command registry that drives menus, palettes and key bindings,
a headless CLI, and end-to-end tests that drive the real window.

The core is the editor, projects, git, the terminal, search and Linux through WSL. **Languages come as modules** that
you download from inside the IDE (Modules tab, Ctrl+Alt+P): Lisp, Python, C and C++, Assembler, Markdown,
reStructuredText, Shell and make. Runs on Windows, macOS and Linux. Common Lisp (SBCL) is the most complete module:
REPL, compiler, structural editing; Scheme, Clojure and Emacs Lisp files get highlighting, structure and indentation.

## Modules

A module is a folder with a `module.json` and a cover picture. It can bring **languages** (a grammar written as data:
highlighting, outline, folding, brackets, indentation, comments), **run commands** (`python3 "{file}"`, a compile and
run line for C), **Linux tool groups** for the project's WSL distribution (`gcc`, `gdb`, `nasm`, QEMU...),
**detection rules** (a project with `.py` files or a `Makefile` is offered the matching modules) and **project templates**.

* **The Modules tab** (Ctrl+Alt+P, the puzzle piece in the left strip, or File > Modules) shows a cover card for every
  module of the registry with *Install*, *Update*, *Remove* and an on/off switch, search and category chips.
  The welcome screen offers it on a first start, and a banner offers what the open project looks like it needs.
* **Choose modules when you create or import a project.** *New Project* and *Get from Version Control* have a
  *Modules* section: ticked modules are downloaded if needed and connected to the project (`.air/project.json`);
  a new project can start from a module's template (an ASDF system, a Python package with tests, a C project with a
  makefile, an assembler boot sector...) and takes its language from it.
* **The core stays the core.** Working in Linux while on Windows (WSL), git, the terminal and search do not depend on any
  module; the modules only say which Linux tools a project wants. With the Lisp module off, the REPL, the ASDF
  configurations and the Lisp commands go away.
* **Run per language.** *Run Current File* (the *Current File* run configuration) runs a file with the run command of
  the module of its language, in the project's Linux or on Windows; output and compiler messages land in Run and Problems.
* **Preview of Markdown and reStructuredText.** The Markdown and reStructuredText modules open their files in three modes,
  as in PyCharm: *Editor*, *Editor and Preview* (side by side, scrolled together in both directions) and *Preview*. The
  three buttons are at the right end of the breadcrumbs, and View > *Editor Only* / *Editor and Preview* / *Preview Only*
  do the same. The preview draws headings, emphasis, links (a link to `#section` scrolls to it, a link to a file opens
  it, a web address opens the browser), lists and task lists, tables, quotes, GitHub alerts and reStructuredText
  admonitions, images next to the document (PNG), and code blocks coloured by the module of their language. The
  parsers (`air-markup`) cover CommonMark with the GitHub extensions, and the reStructuredText of Sphinx: sections,
  lists, literal blocks, grid / simple / list / csv tables and directives such as `note`, `code-block`, `image`,
  `toctree` and `py:function`.
* **Safe downloads.** The registry's `index.json` lists the size and SHA-256 of every file; a module is checked before it is
  put in place (atomically, in the settings folder under `modules/`). The list comes from
  `https://raw.githubusercontent.com/sherhn/AirLisp/main/registry`, or from a web address or folder named in
  Settings > Modules or in `AIR_REGISTRY`, with the copy shipped next to the program (`registry/`) as the fallback, so the
  distribution works offline too. Downloads go through `curl`.
* **Write your own.** Add `tools/registry_src/<name>.py` (see `python.py`), list it in `tools/build_registry.py` and run
  `python tools/build_registry.py [--covers]`: it writes `registry/` (manifests, covers, the checksummed index). The
  grammar rules (`match`, `words`, `region`, `captures`, outline patterns) are the `air-lang` crate's JSON format.
  A `match` can contain a nested `grammar`, applied only inside that match (HTML attributes, CSS values), preserving
  bracket pairs and completion words. `identifier_chars` adds characters such as `-` to identifiers; `void_tags`
  lists elements without closing tags for `tags` indentation and folding. The
  tests of `crates/ide-core/tests/registry.rs` check every shipped module against real code samples. A language with `"preview": "markdown"` or `"rst"` gives its
  files the preview; `"check": ["brackets"]` asks for the bracket
  check, and `check` entries of the module (a command with the same placeholders as a run command and a `pattern` with the groups
  `line`, `msg` and optionally `col`, `sev`) are its syntax checks.

Covers and the icon are drawn by `tools/glass.py` and `tools/covers.py`.

## Features

The bullets about Lisp, the REPL and ASDF belong to the Lisp module.

* **Editor** - own text engine: Lisp-aware indentation (SLIME/Emacs rules), auto-pairs and overtyping,
  smart Enter, rainbow brackets, matching-bracket highlight, undo/redo with coalescing, multi-line
  Tab/Shift+Tab, line duplicate/move/join, comment toggle, extend/shrink selection (Ctrl+W).
* **Structural editing** - slurp/barf (both directions), raise, splice, wrap, kill sexp, sexp navigation.
* **Live REPL** - an SBCL image driven over a structured protocol: eval the form under the caret
  (Ctrl+Enter), errors with backtraces, package tracking, completion/arglist/docs/macroexpand/definition
  lookup straight from the running image.
* **Run & compile** - Shift+F10 runs the file, Ctrl+F9 compiles with the real compiler and maps
  warnings/errors back to source ranges (Problems tool window + squiggles + error stripe).
* **Documentation on hover** - rest the pointer on a name and a popup shows its arguments and what its author wrote: for
  Lisp from the project's own definition (lambda list and docstring) or, with the REPL running, from the image itself
  (`(car list)`, the documentation string); for the languages of modules from the definition found in the project, with
  its docstring (Python) or the comment block above it (`///`, `/** */`, `#`, `;`), also when it is in another file. A
  call is matched to a definition by its number of arguments, so `open(path, flags)` is not taken for a project's own
  one-argument `open`. Go to Declaration (Ctrl+B, Ctrl+click) and Find Usages use the same index in these languages.
* **Completion in the languages of modules** - the popup (Ctrl+Space, or as you type) offers the names the project
  defines (open files as they are now, the rest from the background index), the identifiers of the file, and the
  words of the language - keywords, built-in functions, types - which are the ones its grammar lists. No server and no
  model: the vocabulary is data of the module. Not in comments and strings. (Lisp keeps its own: the project, the
  file and the running image.)
* **Errors in every language** - a language can ask the editor to flag brackets that do not pair up, and a module can name a
  syntax check (`check` in its manifest: a command and a pattern for its output) that runs on the file when it is saved, in the
  project's Linux or on Windows, and marks what it finds in the code and in Problems. Python is compiled without being run
  (so `def f(:` and bad indentation are reported by the interpreter itself), shell scripts go through `sh -n`.
* **Marker comments** - `TODO`, `INFO`, `NOTE` and `FIXME`, `ALERT`, `HACK`, `BUG`, `WARNING` in a comment, in any language, are
  coloured and slanted from the marker to the end of the line (blue for what is to do or to know, orange for what is wrong).
* **Run scenarios** - next to the files, make targets and ASDF systems in the run list: named lists of console commands that run
  one after another in the project folder (in its Linux or on Windows), stopping at the first one that fails if you wish;
  `{file}`, `{dir}`, `{name}` and `{stem}` stand for the open file. Run > Edit Run Scenarios...; they are kept in `.air/project.json`.
* **Navigation** - Search Everywhere (Double Shift), Go to File/Symbol/Line, Recent Files,
  File Structure, Go to Declaration (Ctrl+B / Ctrl+click), Back/Forward, Find Usages, Find in Files.
* **Refactoring** - Rename symbol across the project (token based: strings and comments untouched).
* **Project** - project tree with file operations, ASDF project scaffolding, git branch in the toolbar.
* **Git** - branch switcher, update/commit/push in the toolbar, commit dialog (Ctrl+K, Ctrl+Enter to commit),
  Git tool window (changes with checkboxes, log), diff viewer, change markers in the gutter, file status colours in
  the tree and tabs, rollback. Uses the `git` executable.
* **Local commits** - snapshots of your working tree kept outside of git (`.air/`, excluded through
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
* **Projects** - opening a project while another is open asks: this window or a new window; in this window nothing of the old
  project stays (its tabs, search results, history, problems, terminal).
* **Get from Version Control** - File > Get from Version Control (or the welcome screen): paste a GitHub URL or `owner/repo`,
  pick a folder, optional branch / shallow clone, live progress; the repository is cloned and opened as a project.
* **Works out of the box** - the distribution (`python tools/package.py`) bundles SBCL (public domain/BSD, license included),
  so the REPL and Run work without installing anything.
* **Modern look** - a black ground with soft green and blue glows, "liquid glass" cards and dialogs (translucent, with a light
  rim and a sheen), a green in the spirit of Sber's as the accent and a blue as its partner. Lucide icons (ISC), a sidebar
  Settings dialog, dark/light themes, custom window frame (no OS title bar), Inter + JetBrains Mono fonts.
* **Your own theme** - Settings > Appearance > *Custom* is a third theme that starts from the dark or the light palette and lets
  you change every color of it: windows and panels, the glass and the glow, the editor (caret, selection, search hits, marker
  comments), messages and Git, the colors of the syntax and the six of the rainbow brackets. Each color has a swatch that opens
  a color picker, a field for its hex code (`#RRGGBB`, or `#RRGGBBAA` for the translucent ones; a valid code is taken as you type)
  and a button that gives the palette's own color back; a sample of code shows the result, and so does the whole window. The
  colors changed are kept in `settings.json` (`custom_theme`) and stay when you switch to another theme and back; View >
  *Toggle Dark/Light Theme* leaves the custom theme for the opposite of the palette it is made from.
* **Keymap** - IntelliJ-style. All key bindings come from one registry (`crates/ide-core/src/commands.rs`); see Settings > Keymap.

## GitHub: your repositories in "Get from Version Control"

The **GitHub** tab of File > Get from Version Control lists your repositories (private, collaborator and organization
ones included, newest push first) with search, fork / archived switches, language, stars and age; a click picks one,
a double click clones it into the chosen folder and opens it as a project. Signing in works like in PyCharm:

* **Sign in in the browser** - Git Credential Manager (part of Git for Windows) opens GitHub's own authorization page;
  you sign in there, two-factor included, and the account lands in Git's credential store. Air never sees your
  password and has no OAuth secret of its own.
* **Use the saved login** - the GitHub CLI's (`gh auth token`) or the one Git already saved for github.com.
* **Token** - paste a personal access token (scope `repo` for private repositories); optionally remember it in Git's
  credential store. **Public repositories of a user** need no sign-in at all.

Requests go through `curl` (no HTTP library is linked); the token reaches `curl` through its config on stdin and `git`
through environment variables of that one clone, and only for `https://github.com/...` URLs. Nothing is written to
Air's own files except the last login name. For the UI tests `AIR_GITHUB_API`, `AIR_FAKE_GCM` and
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
in Linux are tagged so Stop really ends them. Air never removes or changes an installed distribution.
For the UI tests `AIR_FAKE_WSL` (see `tests/fake_wsl.py`) replaces `wsl.exe`, so they never touch the real WSL.

### Linux projects (C, assembly, make - e.g. an operating system like os-pi)

A project can be *built and run in Linux* while you work on Windows, whatever its language: the run list (and the status bar)
has *Run in: On Windows / In Linux*, and a project in Python, C or assembly is offered it too (and the tools its language needs
there - `python3`, `gcc`, `nasm` - when they are missing), not only one with a makefile:

* **Choose it per project.** Opening a project with a makefile (or shell scripts) offers "Use Linux"; the status
  bar shows `Windows` / `Linux: <distro>` and switches with a click. The choice is kept in `.air/project.json`,
  which git never sees. The terminal, the make targets, Lisp and the REPL of that project then run in the distribution.
* **Or keep the project in Linux.** Get from Version Control > *Linux: <distro>* clones with the distribution's own git
  into `~/projects/<name>` and opens it as `\\wsl.localhost\<distro>\home\...`; git for such a project always runs
  inside Linux (Linux file modes, symlinks and line endings, no "dubious ownership"). Cloning into a Windows folder
  with *Build and run it in Linux* keeps Linux line endings (`core.autocrlf=false`) so scripts and makefiles work.
* **make targets are run configurations** (`make all`, `make run`, `make debug`, `make clean`, test targets...).
  The output goes to the Run tool window; gcc, ld and nasm errors land in Problems and in the editor, with
  `make -C` sub-directories resolved. `make run` starts QEMU in its own window (WSLg).
* **Developer tools in one click.** A project built with make whose distribution lacks them shows a banner with
  *Install developer tools* (and `make`/`command not found` in the terminal or Run says why). Settings > Linux (WSL)
  > *Developer tools* shows what is there group by group and installs the rest, with a progress bar. On Ubuntu and
  Debian that is: **C and C++** (build-essential, gdb, cmake, ninja, pkg-config, autotools, ccache, bear, 32-bit
  gcc/g++), **Clang and LLVM** (clang, clangd, lldb, clang-format, clang-tidy), **Debugging and analysis** (valgrind,
  strace, ltrace, gdb-multiarch, xxd), **Git and network** (git, curl, wget, ssh, rsync), **Files and text** (zip, unzip,
  xz, jq, ripgrep, fd, tree, vim, nano, htop), **Documentation** (man pages, doxygen), **Python** (python3, pip, venv)
  and **OS development** (nasm, QEMU, xorriso, GRUB tools) - about 230 MB on top of a fresh Ubuntu 24.04. Packages a
  release does not have are skipped. Other distributions get the C toolchain, nasm, gdb, git and QEMU only. Language
  runtimes (Node.js, Java, Go, Rust) are not included: install them with the distribution's package manager (or add
  them as a tool of your own, see below). *Set up git* gives Linux git your name, e-mail and the Windows credential
  manager (GitHub logins);
* **Choose what goes into Linux, and add your own.** Every group of the developer tools has a box: tick it to install the
  group, clear it to leave the group out, and open it (the arrow on its row) to tick the packages one by one - `vim`
  without `nano`, `jq` without the rest. *Tick all* and *Untick all* work on every package at once, and *Install the chosen
  tools* installs exactly the ticked packages (what is left out is not even asked of the package manager). The choice is
  kept in `settings.json` (`linux_tools`). **Your own tools:** *Add a tool...* takes a name, the packages (as the
  distribution's package manager names them: `nmap`, `libssl-dev`, `nodejs`...) and, optionally, the programs that show
  it is installed; without programs the packages themselves are looked for (with `dpkg` on Ubuntu and Debian). A tool of
  your own shows up as a group like the others (changed or removed with the pencil and the bin), is checked like them
  and is installed with every package manager Air knows (apt, dnf, zypper, pacman, apk), one package at a time, so
  one that cannot be installed is skipped and does not stop the rest. Names are limited to letters, digits and `. + - _ :`.
  A banner for a project that cannot be built without a tool (`gcc`, `make`...) installs that group whole, ticked or not.
* **C, assembly, shell and makefiles are not Lisp**: they get their own highlighting, bracket matching, brace
  indentation, folding and comment markers, and never show false Lisp errors.

## Copying from the consoles

The terminal, REPL and Run output are selectable: drag with the mouse (dragging past the edge scrolls), double click
a word or path, triple click a line, **Ctrl+A** selects everything, **Ctrl+C** or right-click > *Copy* copies,
*Copy all* takes the whole log; Esc lets go. After copying, the terminal's command line has the keys again.

## Layout

| crate / app | role |
|---|---|
| `crates/lisp-syntax` | the Lisp reader: lexer, bracket matching, indentation, structural edits, outline, highlighting (no deps) |
| `crates/air-lang` | languages as data: the grammar engine behind the modules (highlighting, outline, folding, indentation) |
| `crates/air-markup` | Markdown and reStructuredText parsed into one document model, for the preview (no deps) |
| `crates/ide-core` | editor model, command registry + keymap, project model, fuzzy/text search, symbol index, settings, the module store and registry client |
| `crates/lisp-runtime` | SBCL discovery, REPL protocol (`lisp/repl-server.lisp`), file runner, compiler diagnostics |
| `apps/air` | the GUI (eframe/egui) incl. the JSON automation harness |
| `apps/air-cli` | headless engine: `check`, `fmt`, `outline`, `eval`, `run`, `compile`, `exec`, `commands`, ... |
| `registry/` | the modules: `index.json` and `modules/<id>/{module.json,cover.png}`, generated by `tools/build_registry.py` |

## Build & run

```
cargo run --release -p air            # the IDE
cargo run --release -p air-cli -- help
```

Requires Rust (stable) and, for the REPL/run features, SBCL (bundled in packaged builds; otherwise on `PATH`
or set `AIR_SBCL` / Settings > Lisp & REPL). Linux needs the usual windowing libs
(`libxkbcommon`, `libgl`, wayland/x11 headers).

## Tests

```
cargo test --workspace          # unit + integration tests (REPL tests use a real SBCL when installed)
python tests/run_e2e.py         # drives the real window with scripts in tests/e2e/*.json
```

The e2e harness (`--script file.json`) injects keyboard/mouse events through egui's input path,
asserts on application state and takes screenshots *of the app window only*.

## Dependencies

Only `eframe`/`egui` (emilk), `rfd`, `serde`/`serde_json`, `ignore`, `dirs`, `png`, `regex`. Bundled fonts: JetBrains Mono and Inter (SIL OFL 1.1, see `assets/fonts`); icons: Lucide (ISC, `assets/icons`). Every crate in
`Cargo.lock` was checked against crates.io (publisher, downloads, yanked status, repository);
`deny.toml` pins the policy (crates.io only, no git sources).

## Русский

Air - лопата, золото вы найдёте сами. IDE, написанная на Rust. Ядро: редактор, проекты, git, терминал, поиск и Linux через WSL. Языки
подключаются модулями, которые скачиваются прямо из IDE (вкладка «Модули», Ctrl+Alt+P): Lisp, Python, C и C++,
ассемблер, Markdown, reStructuredText, shell и make. Модуль Lisp самый полный: структурное редактирование, живой REPL
на SBCL, запуск и компиляция с привязкой ошибок к коду, навигация, рефакторинг (rename). Вид: чёрный фон с зелёным
(в духе Сбера) и синим свечением и «жидкое стекло»; тёмная и светлая темы, раскладка клавиш IntelliJ.
**Своя тема.** Настройки > Внешний вид > «Своя» — третья тема: она берёт за основу тёмную или светлую палитру и даёт
изменить любой её цвет: окна и панели, стекло и свечение, редактор (курсор, выделение, найденное, комментарии-маркеры),
сообщения и Git, цвета синтаксиса и шесть цветов радужных скобок. У каждого цвета есть образец, открывающий палитру, поле
с hex-кодом (`#RRGGBB`, у полупрозрачных `#RRGGBBAA`; верный код применяется сразу при вводе) и кнопка, возвращающая цвет
палитры; результат виден на образце кода и во всём окне. Изменённые цвета хранятся в `settings.json` (`custom_theme`) и
остаются при переключении на другую тему и обратно; View > «Переключить тёмную/светлую тему» уходит из своей темы на
противоположную её основе палитру.
Сборка: `cargo run --release -p air`. Для REPL нужен SBCL в `PATH` (в сборке `tools/package.py` он уже есть).

**Модули.** Модуль — папка с `module.json` и обложкой: языки (грамматика данными: подсветка, структура, сворачивание,
скобки, отступы), команды запуска, группы Linux-инструментов для WSL, правила определения (проекту с `.py` или `Makefile`
предложат нужные модули) и шаблоны проектов. Во вкладке «Модули» у каждого модуля обложка и кнопки «Установить»,
«Обновить», «Удалить», выключатель; при создании проекта и в «Получить из системы контроля версий» модули можно отметить
сразу, а новый проект — начать с шаблона модуля. Скачанное проверяется по SHA-256 из `index.json`; реестр берётся с
GitHub (`registry/`), из адреса или папки в Настройки > Модули, либо из копии рядом с программой (работает без сети).
«Запуск текущего файла» запускает файл командой его языка из модуля — в Linux проекта или в Windows. Если модуль Lisp
выключен, REPL, ASDF и команды Lisp скрываются; WSL и остальное ядро от модулей не зависят.

**Просмотр Markdown и reStructuredText.** Файлы модулей Markdown и reStructuredText открываются в трёх режимах, как в
PyCharm: «Только редактор», «Редактор и просмотр» (рядом, прокрутка синхронна в обе стороны) и «Только просмотр».
Три кнопки — справа в «хлебных крошках», те же команды есть в меню View. Просмотр рисует заголовки, выделение, ссылки
(ссылка на `#раздел` прокручивает к нему, на файл — открывает его, веб-адрес — браузер), списки и задачи, таблицы,
цитаты, GitHub-предупреждения и блоки note/warning из rst, PNG-картинки рядом с документом и блоки кода, окрашенные
модулем их языка. Разбор — отдельный крейт `air-markup`: CommonMark с расширениями GitHub и rst из Sphinx (разделы,
списки, литеральные блоки, таблицы, директивы `note`, `code-block`, `image`, `toctree`, `py:function`).

**Документация при наведении.** Если задержать указатель на имени, всплывает окно с аргументами и описанием: для Lisp —
из определения в проекте (лямбда-список и докстрока) или, когда запущен REPL, из самого образа; для языков модулей —
из найденного в проекте определения с докстрокой (Python) или комментарием над ним (`///`, `/** */`, `#`, `;`), в том
числе из другого файла. Вызов сопоставляется с определением по числу аргументов: `open(path, flags)` не принимается за
собственную функцию проекта `open` с одним аргументом. «Перейти к объявлению» (Ctrl+B, Ctrl+клик) и «Найти
использования» в этих языках работают по тому же индексу.

**Автодополнение в языках модулей.** Список (Ctrl+Space или при наборе) предлагает имена, определённые в проекте
(открытые файлы — как они сейчас, остальные — из фонового индекса), идентификаторы файла и слова языка: ключевые слова,
встроенные функции, типы — те, что перечислены в грамматике модуля. Ни сервера, ни модели: словарь — данные модуля.
В комментариях и строках список не появляется.

**Репозитории GitHub.** Во вкладке GitHub диалога «Получить из системы контроля версий» виден список ваших
репозиториев (в том числе приватных): поиск, значки форков и архивов, язык, звёзды; клик выбирает, двойной клик
клонирует и открывает проект. Вход, как в PyCharm: в браузере откроется страница авторизации GitHub (через Git
Credential Manager), пароль Air не видит. Есть также вход сохранённым логином и по токену.

**Linux без виртуальной машины.** В Настройки > Linux (WSL) можно по шагам установить WSL, дистрибутив (Ubuntu 24.04
по умолчанию, или из файла `.wsl` / `.tar`) и SBCL внутри него, а затем одной кнопкой перевести SBCL, REPL, запуск,
компиляцию и терминал в Linux. Файлы проекта остаются на месте: пути переводятся автоматически. Образ Ubuntu Desktop
`.iso` для WSL не подходит: это установщик для ПК или виртуальной машины.

**Linux-проекты (C, ассемблер, make — например, ОС вроде os-pi).** Проект можно собирать и запускать в Linux, работая в
Windows: при открытии проекта с makefile Air предложит «Использовать Linux», а в строке состояния можно
переключить «Windows / Linux: <дистрибутив>» (выбор хранится в `.air/project.json`, git его не видит). Терминал,
цели make, Lisp и REPL проекта работают в дистрибутиве; цели make (`all`, `run`, `debug`, `clean`, тесты) — это
конфигурации запуска, ошибки gcc/ld/nasm попадают в Problems и в редактор. Клонировать можно прямо в Linux
(`~/projects`, git дистрибутива) или в папку Windows с окончаниями строк Linux. На странице Linux (WSL) одной кнопкой
ставятся инструменты разработчика (C/C++ с cmake и ninja, clang/clangd, valgrind, strace, git, curl, python3/pip, jq, ripgrep, doxygen, nasm и QEMU — около 230 МБ в Ubuntu 24.04), а git в Linux настраивается на ваши имя, почту и вход
в GitHub из Windows. Файлы C, ассемблера, shell и makefile подсвечиваются по-своему и не дают ложных Lisp-ошибок.
Если в дистрибутиве нет этих инструментов, сверху появляется плашка «Установить инструменты сборки», а `make` в терминале
или в Run объясняет причину (`make: Permission denied`, код 127 значит «не установлен»), с полосой прогресса установки.

**Какие инструменты ставить в Linux, и свои.** У каждой группы инструментов разработчика есть флажок: отмеченная группа
ставится, снятая пропускается; стрелка на строке группы раскрывает её пакеты, и их можно отмечать по одному (`vim` без
`nano`, `jq` без остального). «Отметить все» и «Снять все» действуют на все пакеты сразу, а «Установить выбранные
инструменты» ставит ровно отмеченное (то, что снято, менеджеру пакетов даже не передаётся). Выбор хранится в
`settings.json` (`linux_tools`). **Свои инструменты:** «Добавить инструмент...» просит название, пакеты (так, как их
называет менеджер пакетов дистрибутива: `nmap`, `libssl-dev`, `nodejs`...) и, по желанию, программы, по которым видно,
что инструмент установлен; без программ ищутся сами пакеты (через `dpkg` в Ubuntu и Debian). Свой инструмент появляется
в списке как обычная группа (меняется и удаляется карандашом и корзиной), проверяется так же и ставится любым из
известных Air менеджеров пакетов (apt, dnf, zypper, pacman, apk) по одному пакету, так что пакет, который не
установился, пропускается и не мешает остальным. В названиях допустимы буквы, цифры и `. + - _ :`. Плашка проекта, без
инструмента которого его не собрать (`gcc`, `make`...), ставит его группу целиком, отмечена она или нет.

**Ошибки в любых языках.** Язык может попросить редактор отмечать непарные скобки, а модуль — задать проверку синтаксиса
(`check` в манифесте: команда и шаблон разбора вывода). Проверка запускается на сохранённом файле, в Linux проекта или в
Windows, и отмечает находки в коде и в Problems. Python компилируется без запуска (ошибки вроде `def f(:` и неверных
отступов называет сам интерпретатор), скрипты shell проходят через `sh -n`.

**Комментарии-маркеры.** `TODO`, `INFO`, `NOTE` и `FIXME`, `ALERT`, `HACK`, `BUG`, `WARNING` в комментарии на любом языке
выделяются цветом и наклоном от маркера до конца строки (синий — что сделать или знать, оранжевый — что не так).

**Сценарии запуска.** В списке запуска рядом с файлами, целями make и системами ASDF: именованные списки консольных команд,
которые выполняются одна за другой в папке проекта (в его Linux или в Windows) и при желании останавливаются на первой
неудачной; `{file}`, `{dir}`, `{name}`, `{stem}` — открытый файл. Run > «Сценарии запуска...»; хранятся в `.air/project.json`.

**Linux для любого языка.** Где проект собирается и запускается — в списке запуска (пункты «Запускать в: Windows / Linux»)
и в строке состояния; Linux предлагается и проектам на Python, C, ассемблере (вместе с нужными инструментами — `python3`,
`gcc`, `nasm`), а не только проектам с makefile. Если открыть другой проект в том же окне, от старого не остаётся ничего:
вкладки, результаты поиска, история, проблемы, терминал.

**Копирование из консолей.** Текст в терминале, REPL и окне Run можно выделять мышью (протяжка за край прокручивает),
двойной клик — слово или путь, тройной — строка, Ctrl+A — всё, Ctrl+C или правая кнопка > «Копировать» копирует,
«Копировать всё» берёт весь журнал, Esc снимает выделение.
