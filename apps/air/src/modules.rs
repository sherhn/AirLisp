//! The modules of the IDE: what is installed, what the registry offers, and the work of downloading and removing them
//! (on background threads, collected every frame by [`App::poll_modules`]). What a module means for the editor is in
//! [`LanguageSet`](air_lang::LanguageSet): the languages of the enabled modules, rebuilt whenever the set changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use air_lang::{Language, LanguageSet};
use eframe::egui::{self, TextureHandle, TextureOptions};
use ide_core::modules::registry::FileEntry;
use ide_core::modules::{Detect, Index, IndexEntry, Installed, Manifest, ModuleStore, Source, default_sources, fetch_index, is_newer};
use lisp_syntax::dialect::is_lisp_file;

use crate::app::App;
use crate::doc::Doc;
use crate::editor_view::Sev;
use crate::i18n::{self, tr};

/// The Lisp module is part of the program (its engine is compiled in), so its files come with it: a person who used
/// Air for Lisp before modules existed has it from the first start, offline.
const LISP_MODULE_JSON: &[u8] = include_bytes!("../../../registry/modules/lisp/module.json");
const LISP_MODULE_COVER: &[u8] = include_bytes!("../../../registry/modules/lisp/cover.png");

/// What a module is doing right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Busy {
    pub removing: bool,
    pub done: usize,
    pub total: usize,
    pub file: String,
}

enum Event {
    Index(Result<(Index, Source), String>),
    /// A cover picture is in the cache.
    Cover,
    Progress { id: String, done: usize, total: usize, file: String },
    Finished { id: String, removed: bool, result: Result<Manifest, String> },
}

pub struct ModulesState {
    pub store: Option<ModuleStore>,
    pub installed: Vec<Installed>,
    /// Folders of installed modules that cannot be used, and why.
    pub broken: Vec<String>,
    pub langs: LanguageSet,
    pub lisp: bool,
    pub index: Option<Index>,
    pub index_source: Option<Source>,
    pub index_error: Option<String>,
    pub loading_index: bool,
    pub busy: HashMap<String, Busy>,
    /// Pictures already turned into textures.
    pub covers: HashMap<String, TextureHandle>,
    pub filter: String,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Default for ModulesState {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        ModulesState { store: None, installed: vec![], broken: vec![], langs: LanguageSet::new(), lisp: false, index: None, index_source: None, index_error: None, loading_index: false, busy: HashMap::new(), covers: HashMap::new(), filter: String::new(), tx, rx }
    }
}

/// A module as the Modules tab shows it.
#[derive(Clone, Debug)]
pub struct Card {
    pub id: String,
    pub name: String,
    pub category: String,
    pub summary: String,
    /// The version on offer (from the registry) or installed.
    pub version: String,
    pub size: u64,
    pub installed: Option<Installed>,
    /// A newer version than the installed one is on offer.
    pub update: Option<String>,
    pub detect: Detect,
}

/// The registry that is shipped next to the program (and, when running from the sources, the one in the repository).
pub fn bundled_registry() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("registry")));
    let sources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../registry");
    [beside, Some(sources)].into_iter().flatten().find(|p| p.join("index.json").is_file())
}

impl ModulesState {
    pub fn cache_dir(&self) -> Option<PathBuf> {
        self.store.as_ref().map(|s| s.dir().join(".covers"))
    }

    /// The cover of a module: the installed one, else the one in the cache.
    pub fn cover_file(&self, id: &str) -> Option<PathBuf> {
        if let Some(m) = self.installed.iter().find(|m| m.manifest.id == id)
            && let Some(p) = m.cover_path()
        {
            return Some(p);
        }
        let e = self.index.as_ref()?.entry(id)?;
        let f = e.files.iter().find(|f| f.name == e.cover)?;
        let p = self.cache_dir()?.join(cache_name(id, f));
        p.is_file().then_some(p)
    }

    /// The cards of the Modules tab: installed modules and everything the registry offers, by name.
    pub fn cards(&self) -> Vec<Card> {
        let mut cards: Vec<Card> = Vec::new();
        for m in &self.installed {
            let entry = self.index.as_ref().and_then(|i| i.entry(&m.manifest.id));
            let update = entry.filter(|e| is_newer(&e.version, &m.manifest.version)).map(|e| e.version.clone());
            cards.push(Card {
                id: m.manifest.id.clone(),
                name: m.manifest.name.clone(),
                category: m.manifest.category.clone(),
                summary: m.manifest.summary.clone(),
                version: m.manifest.version.clone(),
                size: entry.map_or(0, |e| e.size()),
                installed: Some(m.clone()),
                update,
                detect: m.manifest.detect.clone(),
            });
        }
        if let Some(i) = &self.index {
            let known: Vec<String> = cards.iter().map(|c| c.id.clone()).collect();
            for e in i.modules.iter().filter(|e| !known.contains(&e.id)) {
                cards.push(Card { id: e.id.clone(), name: e.name.clone(), category: e.category.clone(), summary: e.summary.clone(), version: e.version.clone(), size: e.size(), installed: None, update: None, detect: e.detect.clone() });
            }
        }
        cards.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        cards
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.installed.iter().any(|m| m.manifest.id == id && m.enabled)
    }

    /// `(id, how to detect it)` of every module that is known, installed or on offer.
    pub fn detectors(&self) -> Vec<(String, Detect)> {
        self.cards().into_iter().map(|c| (c.id, c.detect)).collect()
    }
}

fn cache_name(id: &str, f: &FileEntry) -> String {
    format!("{id}-{}.png", f.sha256.chars().take(12).collect::<String>())
}

impl App {
    // ------------------------------------------------------------------ start

    /// Open the module store, make the first-run choice and load what is installed.
    pub fn modules_init(&mut self) {
        let Some(dir) = ModuleStore::default_dir() else { return };
        self.mods.store = Some(ModuleStore::at(dir.clone()));
        let store = ModuleStore::at(dir);
        if !store.has_state() {
            // The first start with modules. Whoever used the IDE before gets Lisp, which was all it did; a new user
            // is asked what they work with.
            let used_before = !self.settings.recent_projects.is_empty() || !self.settings.open_files.is_empty();
            let mut state = store.state();
            state.seeded = true;
            if used_before {
                match store.install_bytes("lisp", &[("module.json", LISP_MODULE_JSON), ("cover.png", LISP_MODULE_COVER)]) {
                    Ok(_) => {}
                    Err(e) => self.toast(i18n::fmt("Cannot set up the Lisp module: {}", &[&e]), Sev::Error),
                }
                self.settings.modules_welcome_done = true;
            }
            let _ = store.save_state(&state);
        }
        // tests and scripts: modules named here are installed before anything else (from the registry, no window)
        if let Ok(list) = std::env::var("AIR_PRELOAD_MODULES") {
            self.modules_preload(&list);
        }
        self.modules_reload();
    }

    fn modules_preload(&mut self, list: &str) {
        let Some(store) = self.mods.store.as_ref() else { return };
        let Ok((index, source)) = fetch_index(&self.registry_sources()) else { return };
        for id in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(e) = index.entry(id) {
                let _ = store.install(e, &source, &mut |_, _, _| {});
            }
        }
    }

    pub fn registry_sources(&self) -> Vec<Source> {
        default_sources(&self.settings.registry_url, bundled_registry().as_deref())
    }

    /// Read the installed modules again and rebuild the languages from the enabled ones.
    pub fn modules_reload(&mut self) {
        let Some(store) = self.mods.store.as_ref() else { return };
        let (installed, mut broken) = store.installed_with_errors();
        let mut langs = LanguageSet::new();
        let mut lisp = false;
        for m in installed.iter().filter(|m| m.enabled) {
            lisp |= m.manifest.builtin == "lisp";
            for l in &m.manifest.languages {
                if let Err(e) = langs.add(l.clone()) {
                    broken.push(format!("{}: {e}", m.manifest.id));
                }
            }
        }
        self.mods.installed = installed;
        self.mods.broken = broken;
        self.mods.langs = langs;
        self.mods.lisp = lisp;
        self.apply_languages();
        self.refresh_needed_modules();
        self.refresh_make_targets();
    }

    // ------------------------------------------------------------------ languages

    pub fn lisp_enabled(&self) -> bool {
        self.mods.lisp
    }

    /// The language a file is read in: `None` is Lisp, which has an engine of its own.
    pub fn language_for(&self, path: Option<&Path>, first_line: Option<&str>) -> Option<Arc<Language>> {
        match path {
            Some(p) if is_lisp_file(p) => self.lisp_language(),
            Some(p) => Some(self.mods.langs.for_path(p, first_line)),
            None => self.lisp_language(),
        }
    }

    /// Lisp when the Lisp module is on, else plain text.
    fn lisp_language(&self) -> Option<Arc<Language>> {
        if self.mods.lisp { None } else { Some(self.mods.langs.plain()) }
    }

    /// Give a document the language its file calls for.
    pub fn assign_language(&self, d: &mut Doc) {
        let first = d.ed.text().lines().next().map(str::to_string);
        let lang = self.language_for(d.path.as_deref(), first.as_deref());
        d.set_language(lang);
    }

    /// The set of languages changed: every open document is read again in the right one.
    pub fn apply_languages(&mut self) {
        let mut docs = std::mem::take(&mut self.docs);
        for d in docs.iter_mut().filter(|d| d.diff.is_none()) {
            self.assign_language(d);
        }
        self.docs = docs;
    }

    /// The id of the language of the active document (for run commands).
    pub fn active_language_id(&self) -> Option<String> {
        self.doc().and_then(|d| d.ed.language()).map(|l| l.id().to_string())
    }

    // ------------------------------------------------------------------ the registry

    /// Ask the registry what it offers (in the background); its covers are fetched after.
    pub fn modules_fetch_index(&mut self) {
        if self.mods.loading_index {
            return;
        }
        self.mods.loading_index = true;
        self.mods.index_error = None;
        let sources = self.registry_sources();
        let tx = self.mods.tx.clone();
        let cache = self.mods.cache_dir();
        std::thread::spawn(move || {
            let result = fetch_index(&sources);
            if let Ok((index, source)) = &result {
                let _ = tx.send(Event::Index(Ok((index.clone(), source.clone()))));
                fetch_covers(index, source, cache.as_deref(), &tx);
            } else if let Err(e) = result {
                let _ = tx.send(Event::Index(Err(e)));
            }
        });
    }

    // ------------------------------------------------------------------ install, remove, switch

    pub fn module_install(&mut self, id: &str) {
        let (Some(store), Some(index), Some(source)) = (self.mods.store.as_ref(), self.mods.index.as_ref(), self.mods.index_source.clone()) else {
            self.toast(tr("The list of modules has not been loaded yet"), Sev::Weak);
            return;
        };
        let Some(entry) = index.entry(id).cloned() else {
            self.toast(i18n::fmt("The registry has no module {}", &[&id]), Sev::Error);
            return;
        };
        if self.mods.busy.contains_key(id) {
            return;
        }
        self.mods.busy.insert(id.to_string(), Busy { removing: false, done: 0, total: entry.files.len(), file: String::new() });
        let (dir, tx, id) = (store.dir().to_path_buf(), self.mods.tx.clone(), id.to_string());
        std::thread::spawn(move || {
            let store = ModuleStore::at(dir);
            let progress_tx = tx.clone();
            let pid = id.clone();
            let result = store.install(&entry, &source, &mut |done, total, file| {
                let _ = progress_tx.send(Event::Progress { id: pid.clone(), done, total, file: file.to_string() });
            });
            let _ = tx.send(Event::Finished { id, removed: false, result });
        });
    }

    pub fn module_remove(&mut self, id: &str) {
        let Some(store) = self.mods.store.as_ref() else { return };
        if self.mods.busy.contains_key(id) {
            return;
        }
        self.mods.busy.insert(id.to_string(), Busy { removing: true, done: 0, total: 1, file: String::new() });
        let (dir, tx, id) = (store.dir().to_path_buf(), self.mods.tx.clone(), id.to_string());
        std::thread::spawn(move || {
            let result = ModuleStore::at(dir).remove(&id).map(|_| Manifest::default());
            let _ = tx.send(Event::Finished { id, removed: true, result });
        });
    }

    pub fn module_set_enabled(&mut self, id: &str, on: bool) {
        let Some(store) = self.mods.store.as_ref() else { return };
        match store.set_enabled(id, on) {
            Ok(()) => self.modules_reload(),
            Err(e) => self.toast(i18n::fmt("Cannot switch the module: {}", &[&e]), Sev::Error),
        }
    }

    /// Collect what the background threads have done.
    pub fn poll_modules(&mut self, ctx: &egui::Context) {
        let mut reload = false;
        while let Ok(ev) = self.mods.rx.try_recv() {
            match ev {
                Event::Index(Ok((index, source))) => {
                    self.mods.index = Some(index);
                    self.mods.index_source = Some(source);
                    self.mods.loading_index = false;
                    self.refresh_needed_modules();
                }
                Event::Index(Err(e)) => {
                    self.mods.index_error = Some(e);
                    self.mods.loading_index = false;
                }
                // a picture that arrived is loaded when the tab next draws it
                Event::Cover => {}
                Event::Progress { id, done, total, file } => {
                    if let Some(b) = self.mods.busy.get_mut(&id) {
                        b.done = done;
                        b.total = total;
                        b.file = file;
                    }
                }
                Event::Finished { id, removed, result } => {
                    self.mods.busy.remove(&id);
                    self.mods.covers.remove(&id);
                    let name = self.mods.index.as_ref().and_then(|i| i.entry(&id)).map_or(id.clone(), |e| e.name.clone());
                    match result {
                        Ok(_) if removed => self.toast(i18n::fmt("{} is removed", &[&name]), Sev::Weak),
                        Ok(m) => self.toast(i18n::fmt("{0} {1} is installed", &[&m.name, &m.version]), Sev::Weak),
                        Err(e) => self.toast(i18n::fmt("Cannot install {0}: {1}", &[&name, &e]), Sev::Error),
                    }
                    reload = true;
                }
            }
        }
        if reload {
            self.modules_reload();
        }
        if !self.mods.busy.is_empty() || self.mods.loading_index {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// The cover texture of a module, loaded from its file once (`None` while there is none).
    pub fn cover_texture(&mut self, ctx: &egui::Context, id: &str) -> Option<TextureHandle> {
        if let Some(t) = self.mods.covers.get(id) {
            return Some(t.clone());
        }
        let file = self.mods.cover_file(id)?;
        let bytes = std::fs::read(file).ok()?;
        let (w, h, rgba) = crate::brand::decode_png(&bytes)?;
        let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let t = ctx.load_texture(format!("cover-{id}"), img, TextureOptions::LINEAR);
        self.mods.covers.insert(id.to_string(), t.clone());
        Some(t)
    }

    // ------------------------------------------------------------------ what a project needs

    /// The modules the open project asks for or looks like it needs and that are not on yet.
    pub fn refresh_needed_modules(&mut self) {
        self.needed_modules.clear();
        let Some(p) = self.project.as_ref() else { return };
        let (exts, names) = ide_core::modules::detect::scan_files(p.files.iter().map(PathBuf::as_path));
        let mut wanted = self.project_cfg.modules.clone();
        for id in ide_core::modules::detect::needed_in(&exts, &names, &self.mods.detectors()) {
            if !wanted.contains(&id) {
                wanted.push(id);
            }
        }
        self.needed_modules = wanted.into_iter().filter(|id| !self.mods.is_enabled(id)).collect();
    }
}

/// Fetch the cover pictures the cache does not have yet.
fn fetch_covers(index: &Index, source: &Source, cache: Option<&Path>, tx: &Sender<Event>) {
    let Some(cache) = cache else { return };
    let _ = std::fs::create_dir_all(cache);
    for e in &index.modules {
        let Some(f) = cover_entry(e) else { continue };
        let file = cache.join(cache_name(&e.id, f));
        if file.is_file() {
            continue;
        }
        if let Ok(bytes) = source.fetch(&format!("{}/{}", e.path, f.name))
            && bytes.len() as u64 == f.size
            && ide_core::modules::sha256::sha256_hex(&bytes).eq_ignore_ascii_case(&f.sha256)
            && std::fs::write(&file, bytes).is_ok()
        {
            let _ = tx.send(Event::Cover);
        }
    }
}

fn cover_entry(e: &IndexEntry) -> Option<&FileEntry> {
    e.files.iter().find(|f| f.name == e.cover)
}
