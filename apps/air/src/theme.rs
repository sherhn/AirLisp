//! Colour schemes modelled on JetBrains "New UI" Dark and Light.

use eframe::egui::{self, Color32};
use ide_core::settings::Theme;
use lisp_syntax::highlight::Class;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    // chrome
    pub window_bg: Color32,   // main window / tool windows
    pub panel_bg: Color32,    // side panels
    pub editor_bg: Color32,
    pub gutter_bg: Color32,
    pub border: Color32,
    pub hover: Color32,
    pub selected: Color32,    // selected row in lists / trees
    pub accent: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_on_accent: Color32,
    pub tab_active_bg: Color32,
    pub popup_bg: Color32,
    pub input_bg: Color32,
    // glass look
    pub base_top: Color32,
    pub base_bottom: Color32,
    pub glass: Color32,
    pub sheen: Color32,
    // editor
    pub caret: Color32,
    pub line_number: Color32,
    pub line_number_active: Color32,
    pub current_line: Color32,
    pub selection: Color32,
    pub match_bracket: Color32,
    pub find_match: Color32,
    pub find_current: Color32,
    pub indent_guide: Color32,
    pub whitespace: Color32,
    // semantic
    pub error: Color32,
    pub warning: Color32,
    pub weak_warning: Color32,
    pub ok: Color32,
    pub modified: Color32,
    pub git_modified: Color32,
    pub git_added: Color32,
    pub git_untracked: Color32,
    // syntax
    pub plain: Color32,
    pub comment: Color32,
    pub string: Color32,
    pub number: Color32,
    pub keyword_form: Color32,
    pub builtin: Color32,
    pub definition: Color32,
    pub keyword_arg: Color32,
    pub constant: Color32,
    pub special_var: Color32,
    pub prefix: Color32,
    pub type_name: Color32,
    pub preproc: Color32,
    pub heading: Color32,
    pub strong: Color32,
    pub emphasis: Color32,
    pub code: Color32,
    pub link: Color32,
    pub quote: Color32,
    pub rainbow: [Color32; 6],
}

const fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Palette {
    pub fn dark() -> Palette {
        Palette {
            dark: true,
            window_bg: c(0x15161A),
            panel_bg: c(0x202127),
            editor_bg: c(0x17181C),
            gutter_bg: c(0x1E1F25),
            border: Color32::from_white_alpha(10),
            hover: Color32::from_white_alpha(17),
            selected: Color32::from_rgba_unmultiplied(110, 155, 255, 58),
            accent: c(0x5A86F5),
            text: c(0xE8EAEE),
            text_dim: c(0x8D92A0),
            text_on_accent: c(0xFFFFFF),
            tab_active_bg: Color32::from_white_alpha(20),
            popup_bg: c(0x25272E),
            input_bg: c(0x2A2C34),
            base_top: c(0x1B1C22),
            base_bottom: c(0x0E0F12),
            glass: Color32::from_rgba_unmultiplied(40, 42, 50, 250),
            sheen: Color32::from_white_alpha(22),
            caret: c(0xE8EAEE),
            line_number: c(0x4F5360),
            line_number_active: c(0xA3A8B5),
            current_line: Color32::from_white_alpha(9),
            selection: Color32::from_rgba_unmultiplied(90, 134, 245, 84),
            match_bracket: Color32::from_white_alpha(34),
            find_match: c(0x32593D),
            find_current: c(0x3D6C4A),
            indent_guide: Color32::from_white_alpha(12),
            whitespace: Color32::from_white_alpha(36),
            error: c(0xF75464),
            warning: c(0xF2C55C),
            weak_warning: c(0x6F7A8C),
            ok: c(0x5FB865),
            modified: c(0x3574F0),
            git_modified: c(0x6C95EB),
            git_added: c(0x73BD79),
            git_untracked: c(0xD1675A),
            plain: c(0xBCBEC4),
            comment: c(0x7A7E85),
            string: c(0x6AAB73),
            number: c(0x2AACB8),
            keyword_form: c(0xCF8E6D),
            builtin: c(0x56A8F5),
            definition: c(0xFFC66D),
            keyword_arg: c(0xC77DBB),
            constant: c(0xCF8E6D),
            special_var: c(0xC77DBB),
            prefix: c(0xCF8E6D),
            type_name: c(0x4EC9B0),
            preproc: c(0xB5A33C),
            heading: c(0x5FD38A),
            strong: c(0xF2F4F8),
            emphasis: c(0x9CC3FF),
            code: c(0x6FD3C0),
            link: c(0x5A9BFF),
            quote: c(0x8D92A0),
            rainbow: [c(0xE8BA36), c(0xC77DBB), c(0x56A8F5), c(0x6AAB73), c(0xCF8E6D), c(0x2AACB8)],
        }
    }

    pub fn light() -> Palette {
        Palette {
            dark: false,
            window_bg: c(0xE4E6EC),
            panel_bg: c(0xF5F6F9),
            editor_bg: c(0xFFFFFF),
            gutter_bg: c(0xF3F4F8),
            border: Color32::from_black_alpha(14),
            hover: Color32::from_black_alpha(14),
            selected: Color32::from_rgba_unmultiplied(61, 108, 240, 40),
            accent: c(0x3D6CF0),
            text: c(0x1B1D22),
            text_dim: c(0x6B707C),
            text_on_accent: c(0xFFFFFF),
            tab_active_bg: Color32::from_black_alpha(16),
            popup_bg: c(0xFFFFFF),
            input_bg: c(0xECEDF2),
            base_top: c(0xEEF0F5),
            base_bottom: c(0xDCDFE7),
            glass: Color32::from_rgba_unmultiplied(255, 255, 255, 250),
            sheen: Color32::from_white_alpha(200),
            caret: c(0x1B1D22),
            line_number: c(0xB4B8C4),
            line_number_active: c(0x5F6470),
            current_line: Color32::from_rgba_unmultiplied(61, 108, 240, 14),
            selection: Color32::from_rgba_unmultiplied(61, 108, 240, 70),
            match_bracket: Color32::from_rgba_unmultiplied(61, 108, 240, 50),
            find_match: c(0xCDEBC9),
            find_current: c(0xA9DBA3),
            indent_guide: Color32::from_black_alpha(14),
            whitespace: Color32::from_black_alpha(40),
            error: c(0xDB3B4B),
            warning: c(0xC29E0B),
            weak_warning: c(0x8C8C8C),
            ok: c(0x208A3C),
            modified: c(0x3574F0),
            git_modified: c(0x2F5FC4),
            git_added: c(0x208A3C),
            git_untracked: c(0xB5483B),
            plain: c(0x080808),
            comment: c(0x8C8C8C),
            string: c(0x067D17),
            number: c(0x1750EB),
            keyword_form: c(0x0033B3),
            builtin: c(0x00627A),
            definition: c(0x00627A),
            keyword_arg: c(0x871094),
            constant: c(0x0033B3),
            special_var: c(0x871094),
            prefix: c(0x0033B3),
            type_name: c(0x00756A),
            preproc: c(0x8A6D00),
            heading: c(0x12823F),
            strong: c(0x0B0D10),
            emphasis: c(0x1F57B8),
            code: c(0x0B7A6B),
            link: c(0x1750EB),
            quote: c(0x6B707C),
            rainbow: [c(0xB8860B), c(0x9B2D9B), c(0x1F6FBF), c(0x2E8B3D), c(0xC25E00), c(0x00808A)],
        }
    }

    pub fn for_theme(t: Theme) -> Palette {
        match t {
            Theme::Dark => Palette::dark(),
            Theme::Light => Palette::light(),
        }
    }

    pub fn syntax(&self, class: Class, depth: u32, rainbow: bool) -> Color32 {
        match class {
            Class::Plain => self.plain,
            Class::Comment => self.comment,
            Class::String | Class::Char => self.string,
            Class::Number => self.number,
            Class::Keyword => self.keyword_arg,
            Class::SpecialForm => self.keyword_form,
            Class::Builtin => self.builtin,
            Class::Constant => self.constant,
            Class::SpecialVar | Class::ConstName => self.special_var,
            Class::Definition => self.definition,
            Class::Prefix => self.prefix,
            Class::Bracket => {
                if rainbow {
                    self.rainbow[depth as usize % self.rainbow.len()]
                } else {
                    self.plain
                }
            }
            Class::Error => self.error,
            Class::Type => self.type_name,
            Class::Preproc => self.preproc,
            Class::Heading => self.heading,
            Class::Strong => self.strong,
            Class::Emphasis => self.emphasis,
            Class::Code => self.code,
            Class::Link => self.link,
            Class::Quote => self.quote,
        }
    }

    /// Apply the palette to egui's global style.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut v = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = self.panel_bg;
        v.window_fill = self.popup_bg;
        v.extreme_bg_color = self.input_bg;
        v.text_edit_bg_color = Some(self.input_bg);
        v.faint_bg_color = self.hover;
        v.window_stroke = egui::Stroke::NONE;
        // flat design: popups, menus, tooltips and windows carry no shadow
        v.window_shadow = egui::Shadow::NONE;
        v.popup_shadow = egui::Shadow::NONE;
        v.weak_text_color = Some(self.text_dim);
        v.selection.bg_fill = self.selection;
        v.selection.stroke = egui::Stroke::new(1.0, self.accent);
        v.hyperlink_color = self.accent;
        v.window_corner_radius = egui::CornerRadius::same(12);
        v.menu_corner_radius = egui::CornerRadius::same(8);
        let r = egui::CornerRadius::same(6);
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = r;
        }
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.fg_stroke = egui::Stroke::new(1.0, self.text);
        }
        v.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
        v.widgets.noninteractive.bg_fill = self.panel_bg;
        v.widgets.inactive.bg_fill = self.hover;
        v.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        v.widgets.hovered.bg_fill = self.hover;
        v.widgets.hovered.weak_bg_fill = self.hover;
        v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        v.widgets.active.bg_fill = self.selected;
        v.widgets.active.weak_bg_fill = self.selected;
        v.widgets.open.bg_fill = self.hover;
        v.widgets.open.weak_bg_fill = self.hover;
        ctx.set_theme(if self.dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        ctx.set_visuals_of(if self.dark { egui::Theme::Dark } else { egui::Theme::Light }, v);
        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 5.0);
            style.spacing.interact_size = egui::vec2(36.0, 30.0);
            style.spacing.menu_margin = egui::Margin::same(6);
            style.spacing.window_margin = egui::Margin::same(12);
            style.spacing.scroll = egui::style::ScrollStyle::floating();
            style.spacing.scroll.floating_width = 6.0;
            style.spacing.scroll.bar_width = 10.0;
            style.interaction.selectable_labels = false;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_are_distinct_and_readable() {
        let (d, l) = (Palette::dark(), Palette::light());
        assert!(d.dark && !l.dark);
        assert_ne!(d.editor_bg, l.editor_bg);
        // text must contrast against the editor background
        fn lum(c: Color32) -> f32 {
            0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32
        }
        for p in [d, l] {
            assert!((lum(p.plain) - lum(p.editor_bg)).abs() > 100.0);
            assert!((lum(p.text) - lum(p.panel_bg)).abs() > 100.0);
            assert!((lum(p.comment) - lum(p.editor_bg)).abs() > 40.0);
        }
    }

    #[test]
    fn rainbow_cycles() {
        let p = Palette::dark();
        assert_eq!(p.syntax(Class::Bracket, 0, true), p.syntax(Class::Bracket, 6, true));
        assert_ne!(p.syntax(Class::Bracket, 0, true), p.syntax(Class::Bracket, 1, true));
        assert_eq!(p.syntax(Class::Bracket, 3, false), p.plain);
    }
}
