//! The Settings dialog: a sidebar of pages and grouped rows (title + description on the left, control on the right).

use eframe::egui::{self, Align, Align2, CornerRadius, Key, Margin, Rect, RichText, Sense, Stroke, Vec2, pos2, vec2};
use ide_core::commands;
use ide_core::settings::{Language, Theme};

use crate::app::App;
use crate::fonts;
use crate::i18n::{self, tr};
use crate::icons::{self, Icon};
use crate::state::*;
use crate::theme::Palette;
use crate::widgets::{self, TextField};

/// The settings pages; `WSL_PAGE` is the index of the Linux (WSL) one.
pub const PAGES: [(&str, Icon); 6] = [("Appearance", Icon::Palette), ("Editor", Icon::Code), ("Lisp & REPL", Icon::Repl), ("Linux (WSL)", Icon::Linux), ("Version Control", Icon::Branch), ("Keymap", Icon::Keyboard)];
pub const WSL_PAGE: usize = 3;

const DIALOG_W: f32 = 860.0;
const DIALOG_H: f32 = 560.0;
const SIDEBAR_W: f32 = 224.0;

/// Rows of one group card; draws a hairline between consecutive rows.
pub(crate) struct Rows {
    n: usize,
}

impl Rows {
    /// One settings row: title + description on the left, `control` right-aligned and vertically centred.
    /// `ctrl_w` is the width reserved for the control (the texts wrap before it).
    pub(crate) fn row(&mut self, ui: &mut egui::Ui, pal: &Palette, title: &str, desc: &str, ctrl_w: f32, control: impl FnOnce(&mut egui::Ui)) {
        let w = ui.available_width();
        if self.n > 0 {
            let y = ui.cursor().min.y;
            ui.painter().line_segment([pos2(ui.cursor().min.x, y), pos2(ui.cursor().min.x + w, y)], Stroke::new(1.0, pal.border));
        }
        self.n += 1;
        let text_w = (w - ctrl_w - 24.0).max(140.0);
        let tg = ui.painter().layout(title.to_string(), fonts::ui(13.5), pal.text, text_w);
        let dg = (!desc.is_empty()).then(|| ui.painter().layout(desc.to_string(), fonts::ui(11.5), pal.text_dim, text_w));
        let content_h = tg.size().y + dg.as_ref().map_or(0.0, |g| 3.0 + g.size().y);
        let h = (content_h + 28.0).max(56.0);
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let top = rect.min.y + (h - content_h) / 2.0;
        let th = tg.size().y;
        ui.painter().galley(pos2(rect.min.x, top), tg, pal.text);
        if let Some(g) = dg {
            ui.painter().galley(pos2(rect.min.x, top + th + 3.0), g, pal.text_dim);
        }
        let ctrl_rect = Rect::from_min_max(pos2(rect.max.x - ctrl_w, rect.min.y), rect.max);
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(ctrl_rect).layout(egui::Layout::right_to_left(Align::Center)));
        control(&mut c);
    }
}

/// A captioned card holding settings rows.
pub(crate) fn group(ui: &mut egui::Ui, pal: &Palette, title: &str, add: impl FnOnce(&mut egui::Ui, &mut Rows)) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        widgets::caption(ui, pal, &title.to_uppercase());
    });
    ui.add_space(6.0);
    egui::Frame::new().fill(pal.hover).corner_radius(CornerRadius::same(14)).inner_margin(Margin::symmetric(18, 2)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui, &mut Rows { n: 0 });
    });
    ui.add_space(18.0);
}

impl App {
    pub fn draw_settings(&mut self, ctx: &egui::Context) {
        if !matches!(self.popup, Popup::Settings { .. }) {
            return;
        }
        let pal = self.pal;
        let Popup::Settings { mut lisp_path, mut detected, mut filter, mut page } = std::mem::replace(&mut self.popup, Popup::None) else { return };
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let mut changed_theme = false;
        let mut redetect = false;
        let screen = ctx.content_rect();
        let (dw, dh) = (DIALOG_W.min(screen.width() - 40.0), DIALOG_H.min(screen.height() - 40.0));
        egui::Area::new(egui::Id::new("settings")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            let r = widgets::glass_frame(&pal).inner_margin(Margin::same(0)).show(ui, |ui| {
                let (full, _) = ui.allocate_exact_size(vec2(dw, dh), Sense::hover());

                // ---------------------------------------------------------------- sidebar
                ui.painter().text(pos2(full.min.x + 28.0, full.min.y + 38.0), Align2::LEFT_CENTER, tr("Settings"), fonts::semibold(18.0), pal.text);
                for (i, (name, icon)) in PAGES.iter().enumerate() {
                    let r = Rect::from_min_size(pos2(full.min.x + 12.0, full.min.y + 78.0 + i as f32 * 40.0), vec2(SIDEBAR_W - 24.0, 38.0));
                    let resp = ui.interact(r, egui::Id::new(("settings_page", i)), Sense::click());
                    widgets::row_background(ui, &pal, r.expand2(vec2(8.0, 1.0)), i == page, resp.hovered());
                    let c = if i == page { pal.accent } else { pal.text_dim };
                    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 22.0, r.center().y), Vec2::splat(16.0)), *icon, c);
                    ui.painter().text(pos2(r.min.x + 42.0, r.center().y), Align2::LEFT_CENTER, tr(name), if i == page { fonts::semibold(13.5) } else { fonts::ui(13.5) }, if i == page { pal.text } else { pal.text_dim });
                    if resp.clicked() {
                        page = i;
                    }
                }

                // ---------------------------------------------------------------- content
                let content = Rect::from_min_max(pos2(full.min.x + SIDEBAR_W + 8.0, full.min.y), pos2(full.max.x - 28.0, full.max.y));
                ui.painter().text(pos2(content.min.x + 6.0, full.min.y + 38.0), Align2::LEFT_CENTER, tr(PAGES[page].0), fonts::semibold(16.0), pal.text);
                let close_rect = Rect::from_center_size(pos2(full.max.x - 30.0, full.min.y + 38.0), Vec2::splat(32.0));
                let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(close_rect));
                if widgets::icon_button(&mut cui, &pal, Icon::Close, 32.0, pal.text_dim, tr("Close (Esc)")).clicked() {
                    close = true;
                }
                let body = Rect::from_min_max(pos2(content.min.x, full.min.y + 76.0), pos2(content.max.x, full.max.y - 14.0));
                let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(Align::Min)));
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut bui, |ui| {
                    ui.set_width(body.width() - 14.0);
                    match page {
                        0 => {
                            group(ui, &pal, tr("Theme"), |ui, rows| {
                                rows.row(ui, &pal, tr("Color theme"), tr("Switch between the dark and light look"), 190.0, |ui| {
                                    let cur = if self.settings.theme == Theme::Dark { 0 } else { 1 };
                                    if let Some(i) = widgets::segmented(ui, &pal, &[tr("Dark"), tr("Light")], cur) {
                                        self.settings.theme = if i == 0 { Theme::Dark } else { Theme::Light };
                                        changed_theme = true;
                                    }
                                });
                            });
                            group(ui, &pal, tr("Language"), |ui, rows| {
                                rows.row(ui, &pal, tr("Interface language"), tr("Menus, dialogs and messages"), 220.0, |ui| {
                                    let labels: Vec<&str> = i18n::LANGUAGES.iter().map(|(_, n)| *n).collect();
                                    let cur = i18n::LANGUAGES.iter().position(|(l, _)| *l == self.settings.language).unwrap_or(0);
                                    if let Some(i) = widgets::segmented(ui, &pal, &labels, cur) {
                                        let lang: Language = i18n::LANGUAGES[i].0;
                                        self.settings.language = lang;
                                        i18n::set(lang);
                                    }
                                });
                            });
                            group(ui, &pal, tr("Font"), |ui, rows| {
                                rows.row(ui, &pal, tr("Editor font size"), tr("JetBrains Mono, in points"), 230.0, |ui| {
                                    widgets::slider(ui, &pal, &mut self.settings.font_size, ide_core::settings::MIN_FONT..=ide_core::settings::MAX_FONT, 1.0);
                                });
                            });
                        }
                        1 => {
                            group(ui, &pal, tr("Display"), |ui, rows| {
                                rows.row(ui, &pal, tr("Line numbers"), tr("Show the gutter with line numbers"), 44.0, |ui| {
                                    widgets::toggle(ui, &pal, &mut self.settings.show_line_numbers);
                                });
                                rows.row(ui, &pal, tr("Whitespace"), tr("Draw dots for spaces and arrows for tabs"), 44.0, |ui| {
                                    widgets::toggle(ui, &pal, &mut self.settings.show_whitespace);
                                });
                                rows.row(ui, &pal, tr("Code overview"), tr("Minimap with changes next to the scrollbar"), 44.0, |ui| {
                                    widgets::toggle(ui, &pal, &mut self.settings.minimap);
                                });
                                rows.row(ui, &pal, tr("Rainbow brackets"), tr("Color parentheses by nesting depth"), 44.0, |ui| {
                                    widgets::toggle(ui, &pal, &mut self.settings.rainbow_brackets);
                                });
                            });
                            group(ui, &pal, tr("Behavior"), |ui, rows| {
                                rows.row(ui, &pal, tr("Automatic completion"), tr("Open the popup while typing"), 44.0, |ui| {
                                    widgets::toggle(ui, &pal, &mut self.settings.auto_popup);
                                });
                            });
                        }
                        2 => {
                            group(ui, &pal, tr("Lisp implementation"), |ui, rows| {
                                rows.row(ui, &pal, tr("SBCL executable"), tr("Leave empty to use the bundled or installed SBCL"), 130.0, |ui| {
                                    if widgets::secondary_button(ui, &pal, tr("Browse...")).clicked()
                                        && let Some(f) = rfd::FileDialog::new().pick_file()
                                    {
                                        lisp_path = f.to_string_lossy().into_owned();
                                        redetect = true;
                                    }
                                });
                                let r = TextField::new(&mut lisp_path).hint(tr("auto-detect")).icon(Icon::Terminal).show(ui, &pal);
                                if r.lost_focus() {
                                    redetect = true;
                                }
                                ui.add_space(10.0);
                                let (txt, col) = if detected.is_empty() { (tr("No Lisp implementation detected").to_string(), pal.error) } else { (i18n::fmt("Using {}", &[&detected]), pal.ok) };
                                ui.label(RichText::new(txt).color(col).size(12.0));
                                if self.settings.run_env == ide_core::settings::RunEnv::Wsl {
                                    ui.add_space(4.0);
                                    ui.label(RichText::new(tr("Lisp runs in Linux (WSL); the path above is used when you switch back to Windows.")).color(pal.text_dim).size(11.5));
                                    if widgets::secondary_button(ui, &pal, tr("Linux (WSL) settings...")).clicked() {
                                        page = WSL_PAGE;
                                    }
                                }
                                ui.add_space(14.0);
                            });
                        }
                        WSL_PAGE => self.draw_wsl_page(ui),
                        4 => {
                            group(ui, &pal, tr("Get from Version Control"), |ui, rows| {
                                rows.row(ui, &pal, tr("Default clone directory"), tr("Parent folder for cloned repositories"), 130.0, |ui| {
                                    if widgets::secondary_button(ui, &pal, tr("Browse...")).clicked()
                                        && let Some(d) = rfd::FileDialog::new().pick_folder()
                                    {
                                        self.settings.clone_dir = d.to_string_lossy().into_owned();
                                    }
                                });
                                TextField::new(&mut self.settings.clone_dir).hint("~/AirProjects").icon(Icon::Folder).show(ui, &pal);
                                ui.add_space(12.0);
                                if widgets::secondary_button(ui, &pal, tr("Clone a repository...")).clicked() {
                                    self.run_command(ctx, "file.clone");
                                }
                                ui.add_space(14.0);
                            });
                        }
                        _ => {
                            TextField::new(&mut filter).hint(tr("Filter actions")).icon(Icon::Search).show(ui, &pal);
                            ui.add_space(10.0);
                            let f = filter.to_lowercase();
                            for c in commands::COMMANDS.iter().filter(|c| !c.shortcuts.is_empty() && !c.id.starts_with("caret.") && !c.id.starts_with("select.") && (f.is_empty() || c.title.to_lowercase().contains(&f) || tr(c.title).to_lowercase().contains(&f) || c.shortcuts.iter().any(|s| s.to_lowercase().contains(&f)))) {
                                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
                                ui.painter().text(pos2(r.min.x + 6.0, r.center().y), Align2::LEFT_CENTER, tr(c.title), fonts::ui(13.5), pal.text);
                                let mut cr = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::right_to_left(Align::Center)));
                                for s in c.shortcuts.iter().rev() {
                                    widgets::chip(&mut cr, &pal, &commands::display_shortcut(s));
                                }
                            }
                        }
                    }
                });
            });
            widgets::sheen(ui.painter(), r.response.rect, &pal);
        });
        if changed_theme {
            self.theme_changed();
            self.pal.apply(ctx);
        }
        if redetect {
            self.settings.lisp_path = lisp_path.clone();
            self.lisp = self.resolve_lisp();
            detected = self.lisp.as_ref().map(|l| l.describe()).unwrap_or_default();
        }
        if close {
            self.settings.lisp_path = lisp_path;
            self.lisp = self.resolve_lisp();
            self.save_settings();
            self.editor_focus = true;
        } else {
            self.popup = Popup::Settings { lisp_path, detected, filter, page };
        }
    }
}
