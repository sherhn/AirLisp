//! The documentation popup: rest the pointer on a name and see what it is - its signature (the arguments) and what
//! its author wrote about it. Lisp names come from the project's own definitions or, when the REPL runs, from the
//! image itself; names of the languages modules bring come from the definition found in the project (its header and
//! the docstring or the comments around it).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use air_lang::{Language, LanguageSet};
use eframe::egui::{self, Align2, Color32, FontId, Margin, Pos2, Rect, RichText, Sense, pos2, vec2};
use ide_core::docinfo;
use lisp_syntax::highlight::Class;

use crate::app::App;
use crate::editor_view::ViewOut;
use crate::fonts;
use crate::state::Pending;

/// How long the pointer rests on a name before the popup comes.
const DELAY: f64 = 0.45;
/// How long an answer from the REPL is kept.
const CACHE_SECS: f64 = 30.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HoverInfo {
    pub title: String,
    /// `function`, `macro`, `variable`, `class`...
    pub kind: String,
    /// The call as written: `(greet name &optional greeting)`, `def greet(name):`.
    pub signature: String,
    pub doc: String,
    /// Where it comes from: `src/main.lisp:12`, `Common Lisp image`.
    pub source: String,
    /// A stand-in until something better is known (the REPL is not running): not kept.
    pub fallback: bool,
}

/// A definition of a name in a file of the project.
#[derive(Clone, Debug)]
pub struct DefEntry {
    pub path: PathBuf,
    pub line: usize,
    pub kind: String,
}

/// The definitions of the project in the languages modules bring: language and name -> where they are.
#[derive(Default)]
pub struct LangIndex {
    map: HashMap<(String, String), Vec<DefEntry>>,
    rx: Option<Receiver<Vec<(String, String, DefEntry)>>>,
    /// What the index was built for (the project and the modules that were on).
    built_for: String,
}

impl LangIndex {
    /// Build the index in the background, unless it already is (or is being) built for `key`.
    pub fn ensure(&mut self, key: String, files: Vec<PathBuf>, langs: LanguageSet) {
        if self.built_for == key {
            return;
        }
        self.built_for = key;
        self.map.clear();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let mut batch = vec![];
            for path in files.into_iter().take(40_000) {
                let Some(lang) = langs.find(&path, None) else { continue };
                if lang.is_plain() || crate::preview::markup_of(&lang).is_some() || lisp_syntax::dialect::is_lisp_file(&path) {
                    continue;
                }
                if std::fs::metadata(&path).map(|m| m.len() > 600_000).unwrap_or(true) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                for d in lang.outline(&text) {
                    batch.push((lang.id().to_string(), d.name.clone(), DefEntry { path: path.clone(), line: d.line, kind: d.kind.label().to_string() }));
                }
                if batch.len() >= 400 && tx.send(std::mem::take(&mut batch)).is_err() {
                    return;
                }
            }
            let _ = tx.send(batch);
        });
    }

    /// Take what the background build has found so far.
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        loop {
            match rx.try_recv() {
                Ok(batch) => {
                    for (lang, name, e) in batch {
                        self.map.entry((lang, name)).or_default().push(e);
                    }
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    return;
                }
            }
        }
    }

    /// The background build is still running.
    pub fn building(&self) -> bool {
        self.rx.is_some()
    }

    pub fn get(&self, lang: &str, name: &str) -> &[DefEntry] {
        self.map.get(&(lang.to_string(), name.to_string())).map_or(&[], Vec::as_slice)
    }
}

#[derive(Default)]
pub struct HoverState {
    /// The name the pointer rests on: (document id, start, end, revision).
    key: Option<(u64, usize, usize, u64)>,
    since: f64,
    pos: Pos2,
    pub info: Option<Arc<HoverInfo>>,
    asked: bool,
    /// The cache key of the question the REPL is working on.
    waiting: Option<String>,
    rect: Option<Rect>,
    cache: HashMap<String, (f64, Option<Arc<HoverInfo>>)>,
    pub index: LangIndex,
}

fn kind_name(code: &str) -> &'static str {
    match code.trim() {
        "o" => "special operator",
        "m" => "macro",
        "f" => "function",
        "c" => "constant",
        "v" => "variable",
        "C" => "class",
        _ => "symbol",
    }
}

impl App {
    pub fn hide_hover(&mut self) {
        self.hover.key = None;
        self.hover.info = None;
        self.hover.asked = false;
        self.hover.waiting = None;
        self.hover.rect = None;
    }

    /// Called after the editor of tab `idx` was drawn: where the pointer rests decides what the popup is about.
    pub fn update_hover(&mut self, ctx: &egui::Context, idx: usize, out: &ViewOut) {
        let busy = self.fake_hover.is_none() && ctx.input(|i| i.pointer.any_down() || i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. } | egui::Event::Text(_) | egui::Event::MouseWheel { .. })));
        if busy || self.popup.is_open() || self.completion.active {
            self.hide_hover();
            return;
        }
        // the pointer on the popup keeps it
        if let Some(r) = self.hover.rect
            && ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| r.expand(8.0).contains(p))
        {
            return;
        }
        let Some(d) = self.docs.get(idx) else { return };
        let Some(off) = out.hover_char.or(if self.active == idx { self.fake_hover } else { None }).filter(|_| d.diff.is_none()) else {
            self.hide_hover();
            return;
        };
        let lisp = d.ed.is_lisp();
        let lang_ok = lisp || d.ed.language().is_some_and(|l| !l.is_plain() && crate::preview::markup_of(&l).is_none());
        if !lang_ok {
            self.hide_hover();
            return;
        }
        let (s, e) = if lisp { d.ed.symbol_range_at(off) } else { d.ed.identifier_range_at(off) };
        let word = d.ed.text().get(s..e).unwrap_or("");
        let skip = word.is_empty()
            || off < s
            || off >= e
            || (!lisp && word.chars().next().is_some_and(|c| c.is_ascii_digit()))
            || (lisp && (word.starts_with(':') || word.trim_matches(|c: char| !c.is_alphanumeric()).is_empty()))
            || matches!(d.view.class_at(off), Some(Class::Comment | Class::String | Class::Number | Class::Char));
        if skip {
            self.hide_hover();
            return;
        }
        let key = (d.id, s, e, d.ed.revision());
        let name = word.to_string();
        if self.hover.key != Some(key) {
            self.hide_hover();
            self.hover.key = Some(key);
            self.hover.since = self.now;
            self.hover.pos = out.hover_pos.or(out.text_rect.map(|r| r.center())).unwrap_or(Pos2::ZERO);
        }
        if self.hover.info.is_none() && !self.hover.asked {
            if self.now - self.hover.since >= DELAY {
                self.hover.asked = true;
                self.resolve_hover(idx, &name);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(80));
            }
        }
    }

    fn hover_cache_key(&self, idx: usize, name: &str) -> String {
        let lang = self.docs[idx].ed.language().map(|l| l.id().to_string()).unwrap_or_else(|| "lisp".into());
        format!("{lang}:{name}")
    }

    fn resolve_hover(&mut self, idx: usize, name: &str) {
        let ck = self.hover_cache_key(idx, name);
        if let Some((at, info)) = self.hover.cache.get(&ck)
            && self.now - *at < CACHE_SECS
        {
            self.hover.info = info.clone();
            return;
        }
        let lisp = self.docs[idx].ed.is_lisp();
        let found = if lisp { self.lisp_hover(name, &ck) } else { self.language_hover(idx, name) };
        match found {
            Some(info) => {
                let info = Arc::new(info);
                if !info.fallback {
                    self.hover.cache.insert(ck, (self.now, Some(info.clone())));
                }
                self.hover.info = Some(info);
            }
            // the project is still being read: ask again in a moment
            None if !lisp && self.hover.index.building() => self.hover.asked = false,
            None => {}
        }
    }

    /// A Lisp name: the project's own definition, else (in the next frames) the running image's answer.
    fn lisp_hover(&mut self, name: &str, ck: &str) -> Option<HoverInfo> {
        self.reindex_open_docs();
        if let Some(p) = self.local_definition_parts(name) {
            let root = self.project_root().map(Path::to_path_buf);
            let source = match &root {
                Some(r) => format!("{}:{}", ide_core::workspace::relative(r, &p.path), p.line + 1),
                None => format!("{}:{}", p.path.display(), p.line + 1),
            };
            return Some(HoverInfo { title: name.to_string(), kind: p.kind, signature: p.signature, doc: p.doc.unwrap_or_default(), source, fallback: false });
        }
        if self.repl_request("hover", name, Pending::Hover { cache_key: ck.to_string(), name: name.to_string() }) {
            self.hover.waiting = Some(ck.to_string());
            return None;
        }
        // no REPL: say what the standard names are, and where the rest of the story is
        let bare = lisp_syntax::symbols::strip_package(name);
        let kind = if lisp_syntax::symbols::is_keyword(bare) {
            "special form / macro"
        } else if lisp_syntax::symbols::is_builtin_function(bare) {
            "function"
        } else {
            return None;
        };
        Some(HoverInfo { title: bare.to_string(), kind: kind.into(), signature: String::new(), doc: crate::i18n::tr("Common Lisp standard name. Start the REPL (Alt+8) to see its arguments and documentation.").to_string(), source: String::new(), fallback: true })
    }

    /// The REPL answered a hover question: kind, arguments and documentation, one per line.
    pub fn hover_reply(&mut self, cache_key: &str, name: &str, payload: &str) {
        let info = if payload.trim().is_empty() {
            None
        } else {
            let mut lines = payload.splitn(3, '\n');
            let kind = kind_name(lines.next().unwrap_or(""));
            let signature = lines.next().unwrap_or("").trim().to_string();
            let doc = lines.next().unwrap_or("").trim().to_string();
            Some(Arc::new(HoverInfo { title: name.to_string(), kind: kind.into(), signature, doc, source: crate::i18n::tr("Common Lisp image").to_string(), fallback: false }))
        };
        self.hover.cache.insert(cache_key.to_string(), (self.now, info.clone()));
        if self.hover.waiting.as_deref() == Some(cache_key) {
            self.hover.waiting = None;
            self.hover.info = info;
        }
    }

    /// A name of a language from a module: its definition in this file, the open files or the project.
    fn language_hover(&mut self, idx: usize, name: &str) -> Option<HoverInfo> {
        let lang: Arc<Language> = self.docs[idx].ed.language()?.clone();
        let lang_id = lang.id().to_string();
        let root = self.project_root().map(Path::to_path_buf);
        let show = |path: &Path, line: usize, kind: &str, text: &str| -> HoverInfo {
            let d = docinfo::definition_doc(text, line, &lang);
            let source = match &root {
                Some(r) if path.starts_with(r) => format!("{}:{}", ide_core::workspace::relative(r, path), line + 1),
                _ => format!("{}:{}", path.display(), line + 1),
            };
            HoverInfo { title: name.to_string(), kind: kind.to_string(), signature: d.signature, doc: d.doc, source, fallback: false }
        };
        // this file first, then the other open ones of the same language
        let order: Vec<usize> = std::iter::once(idx).chain((0..self.docs.len()).filter(|i| *i != idx)).collect();
        for i in order {
            let d = &mut self.docs[i];
            if d.diff.is_some() || d.ed.language().is_none_or(|l| l.id() != lang_id) {
                continue;
            }
            let path = d.path.clone().unwrap_or_default();
            let hit = d.defs().iter().find(|def| def.name == name).map(|def| (def.line, def.kind.label().to_string()));
            if let Some((line, kind)) = hit {
                return Some(show(&path, line, &kind, d.ed.text()));
            }
        }
        // the rest of the project, from the background index
        if let Some(p) = self.project.as_ref() {
            let key = format!("{}|{}", p.root.display(), self.mods.installed.iter().filter(|m| m.enabled).map(|m| format!("{}@{}", m.manifest.id, m.manifest.version)).collect::<Vec<_>>().join(","));
            let (files, langs) = (p.files.clone(), self.mods.langs.clone());
            self.hover.index.ensure(key, files, langs);
        }
        self.hover.index.poll();
        let here = self.docs[idx].path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf));
        let mut cands: Vec<DefEntry> = self.hover.index.get(&lang_id, name).to_vec();
        cands.sort_by_key(|c| (c.path.parent().map(Path::to_path_buf) != here, c.path.clone()));
        let c = cands.first()?;
        let text = std::fs::read_to_string(&c.path).ok()?;
        Some(show(&c.path, c.line, &c.kind, &text))
    }

    /// The popup itself, near the pointer.
    pub fn draw_hover(&mut self, ctx: &egui::Context) {
        if self.hover.index_poll_needed() {
            self.hover.index.poll();
        }
        let Some(info) = self.hover.info.clone() else {
            self.hover.rect = None;
            return;
        };
        let pal = self.pal;
        let screen = ctx.content_rect();
        let width = 560.0_f32.min(screen.width() - 24.0);
        let pos = pos2((self.hover.pos.x + 10.0).min(screen.max.x - width - 12.0).max(screen.min.x + 8.0), self.hover.pos.y + 22.0);
        let resp = egui::Area::new(egui::Id::new("hover_doc")).order(egui::Order::Tooltip).fixed_pos(pos).constrain(true).fade_in(false).show(ctx, |ui| {
            // opaque: it lies over code
            egui::Frame::new().fill(pal.popup_bg).corner_radius(egui::CornerRadius::same(12)).stroke(egui::Stroke::new(1.0, pal.border)).inner_margin(Margin::symmetric(14, 11)).show(ui, |ui| {
                ui.set_max_width(width - 30.0);
                ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
                // what it is
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(RichText::new(&info.title).font(fonts::semibold(14.0)).color(pal.text));
                    if !info.kind.is_empty() {
                        let g = ui.painter().layout_no_wrap(info.kind.clone(), fonts::ui(11.0), pal.accent);
                        let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 12.0, 18.0), Sense::hover());
                        ui.painter().rect_filled(r, egui::CornerRadius::same(5), pal.accent.gamma_multiply(0.16));
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, &info.kind, fonts::ui(11.0), pal.accent);
                    }
                });
                // its arguments
                if !info.signature.is_empty() {
                    egui::Frame::new().fill(pal.input_bg).corner_radius(egui::CornerRadius::same(6)).inner_margin(Margin::symmetric(10, 7)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(signature_job(&info.signature, &info.title, &pal)).wrap_mode(egui::TextWrapMode::Wrap));
                    });
                }
                // what its author wrote
                if !info.doc.is_empty() {
                    let mut shown = 0;
                    for para in info.doc.split("\n\n") {
                        if shown >= 14 {
                            ui.label(RichText::new("...").color(pal.text_dim));
                            break;
                        }
                        shown += para.lines().count();
                        ui.add(egui::Label::new(RichText::new(para.trim()).font(FontId::proportional(13.0)).color(pal.text)).wrap_mode(egui::TextWrapMode::Wrap));
                    }
                }
                if !info.source.is_empty() {
                    ui.label(RichText::new(&info.source).font(fonts::ui(11.0)).color(pal.text_dim));
                }
            });
        });
        self.hover.rect = Some(resp.response.rect);
    }
}

impl HoverState {
    fn index_poll_needed(&self) -> bool {
        self.index.rx.is_some()
    }
}

/// The signature in the code font, with the name picked out.
fn signature_job(sig: &str, name: &str, pal: &crate::theme::Palette) -> egui::text::LayoutJob {
    let font = FontId::monospace(12.5);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let plain = |c: Color32| egui::TextFormat { line_height: Some(19.0), ..egui::TextFormat::simple(font.clone(), c) };
    match sig.find(name) {
        Some(p) => {
            job.append(&sig[..p], 0.0, plain(pal.plain));
            job.append(name, 0.0, plain(pal.definition));
            job.append(&sig[p + name.len()..], 0.0, plain(pal.plain));
        }
        None => job.append(sig, 0.0, plain(pal.plain)),
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repl_kinds_have_names() {
        assert_eq!(kind_name("f\n"), "function");
        assert_eq!(kind_name("m"), "macro");
        assert_eq!(kind_name("C"), "class");
        assert_eq!(kind_name("?"), "symbol");
    }

    #[test]
    fn the_index_finds_definitions_of_the_project() {
        let dir = std::env::temp_dir().join(format!("air-hover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("m.c");
        std::fs::write(&file, "// Adds.\nint add(int a, int b) {\n  return a + b;\n}\n").unwrap();
        let mut langs = LanguageSet::new();
        langs
            .add(air_lang::LangDef {
                id: "c".into(),
                name: "C".into(),
                extensions: vec!["c".into()],
                line_comment: "//".into(),
                outline: vec![air_lang::OutlineDef { pattern: r"^[A-Za-z_][\w \t\*]*?[ \t\*]([A-Za-z_]\w*)\s*\([^;{)]*\)\s*\{".into(), kind: "function".into(), name: 1, level: 0, level_mode: "length".into() }],
                ..Default::default()
            })
            .unwrap();
        let mut index = LangIndex::default();
        index.ensure("k".into(), vec![file.clone()], langs);
        for _ in 0..100 {
            index.poll();
            if !index.get("c", "add").is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let hits = index.get("c", "add");
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].line, hits[0].kind.as_str()), (1, "function"));
        assert!(index.get("c", "sub").is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
