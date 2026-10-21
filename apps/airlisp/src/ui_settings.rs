//! The Settings dialog: a sidebar of pages and grouped rows (title + description on the left, control on the right).

use eframe::egui::{self, Align2, CornerRadius, Key, Margin, RichText, Sense, Vec2};
use ide_core::commands;
use ide_core::settings::Theme;

use crate::app::App;
use crate::fonts;
use crate::icons::{self, Icon};
use crate::state::*;
use crate::theme::Palette;
use crate::widgets;

const PAGES: [(&str, Icon); 5] = [("Appearance", Icon::Palette), ("Editor", Icon::Code), ("Lisp & REPL", Icon::Repl), ("Version Control", Icon::Branch), ("Keymap", Icon::Keyboard)];

/// One settings row inside a group card: title + description left, control right.
fn row(ui: &mut egui::Ui, pal: &Palette, title: &str, desc: &str, control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(46.0);
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.label(RichText::new(title).font(fonts::ui(13.5)).color(pal.text));
            if !desc.is_empty() {
                ui.label(RichText::new(desc).size(11.5).color(pal.text_dim));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
}

fn group(ui: &mut egui::Ui, pal: &Palette, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    widgets::caption(ui, pal, title);
    ui.add_space(6.0);
    egui::Frame::new().fill(pal.hover).corner_radius(CornerRadius::same(14)).inner_margin(Margin::symmetric(16, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
    ui.add_space(14.0);
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
        egui::Area::new(egui::Id::new("settings")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            let r = widgets::glass_frame(&pal).inner_margin(Margin::same(0)).show(ui, |ui| {
                ui.set_width(780.0);
                ui.set_height(520.0);
                ui.horizontal(|ui| {
                    // sidebar
                    ui.allocate_ui_with_layout(Vec2::new(200.0, 520.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(RichText::new("Settings").font(fonts::semibold(18.0)));
                        });
                        ui.add_space(14.0);
                        for (i, (name, icon)) in PAGES.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(176.0, 36.0), Sense::click());
                            let r = r.translate(Vec2::new(12.0, 0.0));
                            widgets::row_background(ui, &pal, r.expand2(Vec2::new(8.0, 0.0)), i == page, resp.hovered());
                            icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(r.min.x + 22.0, r.center().y), Vec2::splat(16.0)), *icon, if i == page { pal.accent } else { pal.text_dim });
                            ui.painter().text(egui::pos2(r.min.x + 40.0, r.center().y), Align2::LEFT_CENTER, name, if i == page { fonts::semibold(13.5) } else { fonts::ui(13.5) }, if i == page { pal.text } else { pal.text_dim });
                            if resp.clicked() {
                                page = i;
                            }
                        }
                    });
                    // content
                    ui.vertical(|ui| {
                        ui.set_width(540.0);
                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(PAGES[page].0).font(fonts::semibold(16.0)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(14.0);
                                if widgets::icon_button(ui, &pal, Icon::Close, 30.0, pal.text_dim, "Close (Esc)").clicked() {
                                    close = true;
                                }
                            });
                        });
                        ui.add_space(12.0);
                        egui::ScrollArea::vertical().max_height(450.0).auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(510.0);
                            match page {
                                0 => {
                                    group(ui, &pal, "THEME", |ui| {
                                        row(ui, &pal, "Color theme", "Switch between the dark and light look", |ui| {
                                            let cur = if self.settings.theme == Theme::Dark { 0 } else { 1 };
                                            if let Some(i) = widgets::segmented(ui, &pal, &["Dark", "Light"], cur) {
                                                self.settings.theme = if i == 0 { Theme::Dark } else { Theme::Light };
                                                changed_theme = true;
                                            }
                                        });
                                    });
                                    group(ui, &pal, "FONT", |ui| {
                                        row(ui, &pal, "Editor font size", "JetBrains Mono, in points", |ui| {
                                            ui.add(egui::Slider::new(&mut self.settings.font_size, ide_core::settings::MIN_FONT..=ide_core::settings::MAX_FONT).step_by(1.0));
                                        });
                                    });
                                }
                                1 => {
                                    group(ui, &pal, "DISPLAY", |ui| {
                                        row(ui, &pal, "Line numbers", "Show the gutter with line numbers", |ui| {
                                            widgets::toggle(ui, &pal, &mut self.settings.show_line_numbers);
                                        });
                                        row(ui, &pal, "Whitespace", "Draw dots for spaces and arrows for tabs", |ui| {
                                            widgets::toggle(ui, &pal, &mut self.settings.show_whitespace);
                                        });
                                        row(ui, &pal, "Rainbow brackets", "Color parentheses by nesting depth", |ui| {
                                            widgets::toggle(ui, &pal, &mut self.settings.rainbow_brackets);
                                        });
                                    });
                                    group(ui, &pal, "BEHAVIOR", |ui| {
                                        row(ui, &pal, "Automatic completion", "Open the popup while typing", |ui| {
                                            widgets::toggle(ui, &pal, &mut self.settings.auto_popup);
                                        });
                                        row(ui, &pal, "Save on focus loss", "Save modified files when the window loses focus", |ui| {
                                            widgets::toggle(ui, &pal, &mut self.settings.auto_save_on_focus_loss);
                                        });
                                    });
                                }
                                2 => {
                                    group(ui, &pal, "LISP IMPLEMENTATION", |ui| {
                                        row(ui, &pal, "SBCL executable", "Leave empty to use the bundled or installed SBCL", |ui| {
                                            if widgets::secondary_button(ui, &pal, "Browse...").clicked()
                                                && let Some(f) = rfd::FileDialog::new().pick_file()
                                            {
                                                lisp_path = f.to_string_lossy().into_owned();
                                                redetect = true;
                                            }
                                        });
                                        ui.add_space(2.0);
                                        let r = widgets::input_frame(ui, &pal, Some(Icon::Terminal), |ui| ui.add(egui::TextEdit::singleline(&mut lisp_path).hint_text("auto-detect").desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6))));
                                        if r.lost_focus() {
                                            redetect = true;
                                        }
                                        ui.add_space(8.0);
                                        let (txt, col) = if detected.is_empty() { ("No Lisp implementation detected".to_string(), pal.error) } else { (format!("Using {detected}"), pal.ok) };
                                        ui.label(RichText::new(txt).color(col).size(12.0));
                                        ui.add_space(4.0);
                                    });
                                }
                                3 => {
                                    group(ui, &pal, "GET FROM VERSION CONTROL", |ui| {
                                        row(ui, &pal, "Default clone directory", "Parent folder for cloned repositories", |ui| {
                                            if widgets::secondary_button(ui, &pal, "Browse...").clicked()
                                                && let Some(d) = rfd::FileDialog::new().pick_folder()
                                            {
                                                self.settings.clone_dir = d.to_string_lossy().into_owned();
                                            }
                                        });
                                        ui.add_space(2.0);
                                        widgets::input_frame(ui, &pal, Some(Icon::Folder), |ui| {
                                            ui.add(egui::TextEdit::singleline(&mut self.settings.clone_dir).hint_text("~/AirLispProjects").desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6)));
                                        });
                                        ui.add_space(8.0);
                                        if widgets::secondary_button(ui, &pal, "Clone a repository...").clicked() {
                                            self.run_command(ctx, "file.clone");
                                        }
                                        ui.add_space(4.0);
                                    });
                                }
                                _ => {
                                    widgets::input_frame(ui, &pal, Some(Icon::Search), |ui| {
                                        ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter actions").desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(Margin::symmetric(2, 6)));
                                    });
                                    ui.add_space(10.0);
                                    let f = filter.to_lowercase();
                                    for c in commands::COMMANDS.iter().filter(|c| !c.shortcuts.is_empty() && !c.id.starts_with("caret.") && !c.id.starts_with("select.") && (f.is_empty() || c.title.to_lowercase().contains(&f) || c.shortcuts.iter().any(|s| s.to_lowercase().contains(&f)))) {
                                        ui.horizontal(|ui| {
                                            ui.set_min_height(28.0);
                                            ui.label(RichText::new(c.title).color(pal.text));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                for s in c.shortcuts.iter().rev() {
                                                    widgets::chip(ui, &pal, &commands::display_shortcut(s));
                                                }
                                            });
                                        });
                                    }
                                }
                            }
                        });
                    });
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
            self.lisp = lisp_runtime::discover::find_sbcl(&self.settings.lisp_path);
            detected = self.lisp.as_ref().map(|l| l.path.to_string_lossy().into_owned()).unwrap_or_default();
        }
        if close {
            self.settings.lisp_path = lisp_path;
            self.lisp = lisp_runtime::discover::find_sbcl(&self.settings.lisp_path);
            self.save_settings();
            self.editor_focus = true;
        } else {
            self.popup = Popup::Settings { lisp_path, detected, filter, page };
        }
    }
}
