//! Font setup: prefer good system fonts (JetBrains Mono / Cascadia / Consolas ...), keep egui's
//! bundled fonts as fallback so every glyph (Cyrillic, symbols, emoji) still renders.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

fn candidates_mono() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(windows) {
        let win = std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        if let Some(l) = &local {
            v.push(l.join(r"Microsoft\Windows\Fonts\JetBrainsMono-Regular.ttf"));
        }
        for f in ["JetBrainsMono-Regular.ttf", "CascadiaMono.ttf", "CascadiaCode.ttf", "consola.ttf"] {
            v.push(win.join("Fonts").join(f));
        }
    } else if cfg!(target_os = "macos") {
        for f in ["/Library/Fonts/JetBrainsMono-Regular.ttf", "/System/Library/Fonts/Menlo.ttc", "/System/Library/Fonts/Monaco.ttf", "/System/Library/Fonts/SFNSMono.ttf"] {
            v.push(PathBuf::from(f));
        }
        if let Some(h) = &home {
            v.push(h.join("Library/Fonts/JetBrainsMono-Regular.ttf"));
        }
    } else {
        for f in [
            "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        ] {
            v.push(PathBuf::from(f));
        }
        if let Some(h) = &home {
            v.push(h.join(".local/share/fonts/JetBrainsMono-Regular.ttf"));
        }
    }
    v
}

fn candidates_ui() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        let win = std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        for f in ["Inter-Regular.ttf", "segoeui.ttf", "arial.ttf"] {
            v.push(win.join("Fonts").join(f));
        }
    } else if cfg!(target_os = "macos") {
        for f in ["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Helvetica.ttc", "/Library/Fonts/Arial.ttf"] {
            v.push(PathBuf::from(f));
        }
    } else {
        for f in [
            "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ] {
            v.push(PathBuf::from(f));
        }
    }
    v
}

/// First readable font file among `paths`. `.ttc` collections are supported (index 0).
fn first_font(paths: &[PathBuf]) -> Option<(String, Vec<u8>)> {
    for p in paths {
        if let Ok(bytes) = std::fs::read(p) {
            // sanity check: TrueType / OpenType / collection magic
            let ok = matches!(bytes.get(0..4), Some([0, 1, 0, 0]) | Some(b"OTTO") | Some(b"true") | Some(b"ttcf"));
            if ok {
                let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "font".into());
                return Some((name, bytes));
            }
        }
    }
    None
}

pub fn install(ctx: &egui::Context) -> (Option<String>, Option<String>) {
    let mut fonts = FontDefinitions::default();
    let mono = first_font(&candidates_mono());
    let ui = first_font(&candidates_ui());
    let mut names = (None, None);
    if let Some((name, bytes)) = mono {
        let key = format!("lc-mono-{name}");
        fonts.font_data.insert(key.clone(), Arc::new(FontData::from_owned(bytes)));
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, key);
        names.0 = Some(name);
    }
    if let Some((name, bytes)) = ui {
        let key = format!("lc-ui-{name}");
        fonts.font_data.insert(key.clone(), Arc::new(FontData::from_owned(bytes)));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, key);
        names.1 = Some(name);
    }
    ctx.set_fonts(fonts);
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_font_files() {
        let dir = std::env::temp_dir().join(format!("lc-font-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("fake.ttf");
        std::fs::write(&bad, b"this is not a font").unwrap();
        assert!(first_font(&[bad.clone(), dir.join("missing.ttf")]).is_none());
        let good = dir.join("good.ttf");
        std::fs::write(&good, [0u8, 1, 0, 0, 9, 9, 9, 9]).unwrap();
        assert_eq!(first_font(&[bad, good]).unwrap().0, "good");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn candidate_lists_are_not_empty() {
        assert!(!candidates_mono().is_empty());
        assert!(!candidates_ui().is_empty());
    }
}
