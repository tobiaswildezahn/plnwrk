//! Checks of the design system: contrast (WCAG 2.2 AA), smallest sizes, and that colour values
//! live only in `design/`.

use egui::Color32;

use super::tokens::{DARK, LIGHT, Palette, size};

/// Relative luminance (WCAG 2.x).
fn luminance(color: Color32) -> f64 {
    let channel = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

fn contrast(a: Color32, b: Color32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    let (light, dark) = if a > b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

/// Text and what it stands on (4.5:1), and controls against their surroundings (3:1).
fn pairs(p: &Palette) -> Vec<(String, Color32, Color32, f64)> {
    let mut all = Vec::new();
    for (name, back) in [
        ("background", p.background),
        ("surface", p.surface),
        ("raised", p.raised),
    ] {
        for (text, fore) in [
            ("text", p.text),
            ("muted", p.muted),
            ("accent", p.accent),
            ("danger", p.danger),
            ("warning", p.warning),
            ("success", p.success),
        ] {
            all.push((format!("{text} on {name}"), fore, back, 4.5));
        }
        all.push((format!("control border on {name}"), p.control, back, 3.0));
        all.push((format!("warning mark on {name}"), p.warning_mark, back, 3.0));
    }
    let more = [
        ("text on accent_soft", p.text, p.accent_soft, 4.5),
        ("accent on accent_soft", p.accent, p.accent_soft, 4.5),
        ("text on info_soft", p.text, p.info_soft, 4.5),
        ("accent on info_soft", p.accent, p.info_soft, 4.5),
        ("on_accent on accent_fill", p.on_accent, p.accent_fill, 4.5),
        (
            "on_accent on accent_hover",
            p.on_accent,
            p.accent_hover,
            4.5,
        ),
        ("on_accent on danger_fill", p.on_accent, p.danger_fill, 4.5),
        (
            "on_accent on danger_hover",
            p.on_accent,
            p.danger_hover,
            4.5,
        ),
        ("danger on danger_soft", p.danger, p.danger_soft, 4.5),
        ("warning on warning_soft", p.warning, p.warning_soft, 4.5),
        ("success on success_soft", p.success, p.success_soft, 4.5),
        ("accent_fill on surface", p.accent_fill, p.surface, 3.0),
        ("danger_fill on surface", p.danger_fill, p.surface, 3.0),
    ];
    all.extend(more.map(|(name, fore, back, least)| (name.to_owned(), fore, back, least)));
    all
}

#[test]
fn every_colour_pair_meets_wcag_aa() {
    let mut failed = Vec::new();
    for (mode, palette) in [("light", &LIGHT), ("dark", &DARK)] {
        for (name, fore, back, least) in pairs(palette) {
            let ratio = contrast(fore, back);
            if ratio < least {
                failed.push(format!("{mode}: {name} = {ratio:.2} < {least}"));
            }
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn the_contrast_formula_is_right() {
    // Known values: black on white is 21:1, a colour on itself 1:1.
    assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
    assert!((contrast(LIGHT.accent, LIGHT.accent) - 1.0).abs() < 1e-9);
    // #767676 on white is the classic smallest grey for 4.5:1 (4.54).
    let grey = Color32::from_rgb(0x76, 0x76, 0x76);
    assert!((contrast(grey, Color32::WHITE) - 4.54).abs() < 0.01);
}

#[test]
fn nothing_is_smaller_than_readable_and_clickable() {
    const {
        assert!(size::SMALL >= 12.5);
        assert!(size::BODY >= 14.0);
        assert!(size::TARGET >= 24.0, "WCAG 2.2, 2.5.8");
        assert!(size::CONTROL >= size::TARGET);
    }
    let ctx = egui::Context::default();
    super::install(&ctx);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let style = ctx.style_of(theme);
        for (name, font) in &style.text_styles {
            assert!(font.size >= size::SMALL, "{name:?}: {}", font.size);
        }
        assert!(style.spacing.interact_size.y >= size::TARGET);
    }
}

/// Every source file of the interface outside `design/`, with its path.
fn sources() -> Vec<(std::path::PathBuf, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "design") {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                found.push((path, text));
            }
        }
    }
    found
}

#[test]
fn colours_come_only_from_the_design_tokens() {
    let forbidden = [
        "Color32::from_rgb",
        "Color32::from_rgba",
        "Color32::from_gray",
        "Color32::from_black_alpha",
        "Color32::from_white_alpha",
        "Color32::from_hex",
        "Color32::RED",
        "Color32::GREEN",
        "Color32::BLUE",
        "Color32::YELLOW",
        "Color32::GRAY",
        "Color32::LIGHT",
        "Color32::DARK",
        "Color32::BLACK",
        "Color32::WHITE",
        "Color32::GOLD",
        "Color32::KHAKI",
        "Color32::BROWN",
        "Color32::ORANGE",
    ];
    let mut found = Vec::new();
    for (path, text) in sources() {
        if path.ends_with("tests.rs") || path.ends_with("model_tests.rs") {
            continue;
        }
        for (n, line) in text.lines().enumerate() {
            if forbidden.iter().any(|f| line.contains(f)) {
                found.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "colours belong in design/tokens.rs: {found:#?}"
    );
}

#[test]
fn the_guard_sees_a_colour_outside_the_design_system() {
    // The guard reads the real files: a view with a colour value would be found.
    let views = sources();
    assert!(
        views.iter().any(|(p, _)| p.ends_with("views/schedule.rs")),
        "the guard reads the views"
    );
    assert!(!views.iter().any(|(p, _)| p.ends_with("design/tokens.rs")));
}
