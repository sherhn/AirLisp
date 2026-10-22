//! UI icons: Lucide (ISC license) rendered from a bundled icon font, plus the Lisp file badge.

use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Rect, Sense, Vec2};

use crate::fonts;
use crate::lucide as l;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Folder,
    FolderOpen,
    File,
    /// A Lisp source file: a small badge with a lambda.
    Lisp,
    Play,
    Stop,
    Restart,
    Search,
    Warning,
    Error,
    Check,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Close,
    Plus,
    Gear,
    List,
    Terminal,
    Trash,
    Project,
    Branch,
    Dot,
    Clear,
    Info,
    ArrowUp,
    ArrowDown,
    Commit,
    Refresh,
    History,
    Diff,
    Menu,
    More,
    /// The REPL tool window.
    Repl,
    Download,
    Palette,
    Keyboard,
    Sliders,
    Code,
    FolderPlus,
    FilePlus,
    Pencil,
    Copy,
    Save,
    Sun,
    Moon,
    Fork,
    External,
    Zap,
    Layers,
    Package,
    /// The Modules tab.
    Puzzle,
    Hammer,
    Globe,
    Cloud,
    Eye,
    ChevronLeft,
    Undo,
    FileDiff,
    Script,
    /// The Linux (WSL) environment.
    Linux,
    /// Install / download into this machine.
    Install,
    Ok,
    Spinner,
    Key,
    User,
    Lock,
    /// A repository.
    Repo,
}

pub fn glyph(icon: Icon) -> Option<char> {
    Some(match icon {
        Icon::Folder => l::FOLDER,
        Icon::FolderOpen => l::FOLDER_OPEN,
        Icon::File => l::FILE_TEXT,
        Icon::Lisp | Icon::Dot => return None,
        Icon::Play => l::PLAY,
        Icon::Stop => l::SQUARE,
        Icon::Restart => l::ROTATE_CW,
        Icon::Search => l::SEARCH,
        Icon::Warning => l::TRIANGLE_ALERT,
        Icon::Error => l::CIRCLE_X,
        Icon::Check => l::CHECK,
        Icon::ChevronRight => l::CHEVRON_RIGHT,
        Icon::ChevronDown => l::CHEVRON_DOWN,
        Icon::ChevronUp => l::CHEVRON_UP,
        Icon::Close => l::X,
        Icon::Plus => l::PLUS,
        Icon::Gear => l::SETTINGS,
        Icon::List => l::LIST_TREE,
        Icon::Terminal => l::TERMINAL,
        Icon::Trash => l::TRASH_2,
        Icon::Project => l::FOLDER_TREE,
        Icon::Branch => l::GIT_BRANCH,
        Icon::Clear => l::ERASER,
        Icon::Info => l::INFO,
        Icon::ArrowUp => l::ARROW_UP,
        Icon::ArrowDown => l::ARROW_DOWN,
        Icon::Commit => l::GIT_COMMIT_HORIZONTAL,
        Icon::Refresh => l::REFRESH_CW,
        Icon::History => l::HISTORY,
        Icon::Diff => l::DIFF,
        Icon::Menu => l::MENU,
        Icon::More => l::ELLIPSIS,
        Icon::Repl => l::SQUARE_TERMINAL,
        Icon::Download => l::CLOUD_DOWNLOAD,
        Icon::Palette => l::PALETTE,
        Icon::Keyboard => l::KEYBOARD,
        Icon::Sliders => l::SLIDERS_HORIZONTAL,
        Icon::Code => l::CODE_XML,
        Icon::FolderPlus => l::FOLDER_PLUS,
        Icon::FilePlus => l::FILE_PLUS,
        Icon::Pencil => l::PENCIL,
        Icon::Copy => l::COPY,
        Icon::Save => l::SAVE,
        Icon::Sun => l::SUN,
        Icon::Moon => l::MOON,
        Icon::Fork => l::GIT_FORK,
        Icon::External => l::EXTERNAL_LINK,
        Icon::Zap => l::ZAP,
        Icon::Layers => l::LAYERS,
        Icon::Package => l::PACKAGE,
        Icon::Puzzle => l::PUZZLE,
        Icon::Hammer => l::HAMMER,
        Icon::Globe => l::GLOBE,
        Icon::Cloud => l::CLOUD_DOWNLOAD,
        Icon::Eye => l::SQUARE_TERMINAL,
        Icon::ChevronLeft => l::CHEVRON_LEFT,
        Icon::Undo => l::UNDO_2,
        Icon::FileDiff => l::FILE_DIFF,
        Icon::Script => l::SCROLL_TEXT,
        Icon::Linux => l::CONTAINER,
        Icon::Install => l::HARD_DRIVE_DOWNLOAD,
        Icon::Ok => l::CIRCLE_CHECK_BIG,
        Icon::Spinner => l::LOADER_CIRCLE,
        Icon::Key => l::KEY_ROUND,
        Icon::User => l::USER,
        Icon::Lock => l::LOCK,
        Icon::Repo => l::BOOK_MARKED,
    })
}

/// Paint `icon` centred in `rect` (the glyph is scaled to the shorter side).
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    match icon {
        Icon::Dot => {
            painter.circle_filled(rect.center(), size * 0.22, color);
        }
        Icon::Lisp => {
            let r = Rect::from_center_size(rect.center(), Vec2::splat(size));
            painter.rect_filled(r, CornerRadius::same((size * 0.28) as u8), color.gamma_multiply(0.22));
            painter.text(r.center() + Vec2::new(0.0, size * 0.02), Align2::CENTER_CENTER, "\u{3bb}", egui::FontId::monospace(size * 0.82), color);
        }
        _ => {
            if let Some(g) = glyph(icon) {
                painter.text(rect.center(), Align2::CENTER_CENTER, g.to_string(), fonts::icons(size), color);
            }
        }
    }
}

/// Allocate a square, paint the icon, and return the response.
pub fn button(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32, hover_bg: Color32, tooltip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), hover_bg);
    }
    paint(ui.painter(), rect.shrink(size * 0.24), icon, color);
    if tooltip.is_empty() { resp } else { resp.on_hover_text(tooltip) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_a_glyph_or_is_custom() {
        use Icon::*;
        for i in [Folder, FolderOpen, File, Play, Stop, Restart, Search, Warning, Error, Check, ChevronRight, ChevronDown, ChevronUp, Close, Plus, Gear, List, Terminal, Trash, Project, Branch, Clear, Info, ArrowUp, ArrowDown, Commit, Refresh, History, Diff, Menu, More, Repl, Download, Palette, Keyboard, Sliders, Code, FolderPlus, FilePlus, Pencil, Copy, Save, Sun, Moon, Fork, External, Zap, Layers, Package, Puzzle, Hammer, Globe, Cloud, Eye, ChevronLeft, Undo, FileDiff, Script, Linux, Install, Ok, Spinner, Key, User, Lock, Repo] {
            assert!(glyph(i).is_some(), "{i:?}");
        }
        assert!(glyph(Lisp).is_none() && glyph(Dot).is_none());
    }

    #[test]
    fn glyphs_are_in_the_private_use_area_of_the_font() {
        let g = glyph(Icon::Play).unwrap() as u32;
        assert!((0xe000..=0xf8ff).contains(&g));
    }
}
