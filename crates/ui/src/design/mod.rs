//! The design system of plnwrk (Phase 9a): tokens, font, theme, symbols and building blocks.
//! The only source of colours, spacing, radii, type and symbols; views use what is here.
//! docs/plans/2026-10-17-phase-9-gestaltung.md §5.

// The building blocks and symbols are taken up view by view in phases 9b–9f.
#![allow(
    dead_code,
    reason = "the design system is adopted view by view in phases 9b–9f"
)]

pub mod icons;
pub mod ribbon;
pub mod tokens;
pub mod widgets;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use egui::{
    CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle,
    Theme, vec2,
};

pub use icons::Icon;
pub use tokens::palette;
use tokens::{radius, size, space};

/// Fira Sans (SIL Open Font License 1.1), subset to Latin, punctuation, arrows and symbols,
/// with tabular figures as the default digits: `assets/fonts/README.md`.
const REGULAR: &[u8] = include_bytes!("../../assets/fonts/FiraSans-Regular.otf");
const SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/FiraSans-SemiBold.otf");

/// The family for headings and emphasis (Fira Sans SemiBold).
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// The text style of section titles.
pub fn subheading() -> TextStyle {
    TextStyle::Name("subheading".into())
}

/// Puts the font and both themes in place before the first frame (the desktop and web apps,
/// at start): egui loads the fonts at the beginning of the first pass, so the very first
/// frame already shows Fira Sans and the theme.
pub fn install_at_start(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.data_mut(|d| d.insert_temp(fonts_id(), 0_u64));
    themes(ctx);
}

/// Puts the font and both themes in place from inside a frame (tests, or an app that missed
/// the start), once per context. egui takes new fonts only from the next pass on, and a style
/// naming the semibold family before then would stop the app: so the fonts first, the themes
/// one pass later.
pub fn install(ctx: &egui::Context) {
    if ready(ctx) {
        return;
    }
    let pass = ctx.cumulative_pass_nr();
    match ctx.data(|d| d.get_temp::<u64>(fonts_id())) {
        None => {
            ctx.set_fonts(fonts());
            ctx.data_mut(|d| d.insert_temp(fonts_id(), pass + 1));
            ctx.request_repaint();
        }
        Some(from) if pass >= from => themes(ctx),
        Some(_) => {}
    }
}

fn fonts_id() -> egui::Id {
    egui::Id::new("plnwrk-design-fonts-from-pass")
}

fn themes_id() -> egui::Id {
    egui::Id::new("plnwrk-design-themes")
}

/// Whether the fonts and themes are in place (so the semibold family can be used).
fn ready(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(themes_id()))
        .unwrap_or(false)
}

fn themes(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(themes_id(), true));
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |style| apply(style, theme == Theme::Dark));
    }
}

/// Semibold in this size, or the regular font while the fonts are not in place yet.
pub fn strong_font(ctx: &egui::Context, size: f32) -> FontId {
    if ready(ctx) {
        FontId::new(size, semibold())
    } else {
        FontId::proportional(size)
    }
}

/// Fira Sans first; egui's own fonts stay behind it for what Fira lacks (♦, emoji, symbols).
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("fira".into(), Arc::new(FontData::from_static(REGULAR)));
    fonts.font_data.insert(
        "fira-semibold".into(),
        Arc::new(FontData::from_static(SEMIBOLD)),
    );
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut proportional = vec!["fira".to_owned()];
    proportional.extend(fallback.iter().cloned());
    let mut strong = vec!["fira-semibold".to_owned()];
    strong.extend(fallback);
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    fonts.families.insert(semibold(), strong);
    fonts
}

/// The style of one theme, from the tokens.
fn apply(style: &mut egui::Style, dark: bool) {
    let p = tokens::of(dark);
    style.text_styles = [
        (TextStyle::Body, FontId::proportional(size::BODY)),
        (TextStyle::Button, FontId::proportional(size::BODY)),
        (TextStyle::Small, FontId::proportional(size::SMALL)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
        (TextStyle::Heading, FontId::new(size::HEADING, semibold())),
        (subheading(), FontId::new(size::SUBHEADING, semibold())),
    ]
    .into();

    let spacing = &mut style.spacing;
    // Controls 28 high and 4 apart: rows of 32 (lists, forms); buttons with text are 32 high.
    spacing.item_spacing = vec2(space::S, space::XS);
    spacing.button_padding = vec2(space::M, 7.0);
    spacing.interact_size = vec2(size::TARGET, size::ROW - space::XS);
    spacing.window_margin = Margin::same(16);
    spacing.menu_margin = Margin::same(4);
    spacing.indent = space::L + 4.0;
    spacing.icon_width = 16.0;
    spacing.icon_width_inner = 10.0;
    spacing.icon_spacing = space::S - 2.0;
    spacing.combo_height = 320.0;
    spacing.scroll = egui::style::ScrollStyle::solid();
    spacing.scroll.bar_width = 8.0;

    let v = &mut style.visuals;
    *v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.background;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::same(radius::LARGE);
    v.menu_corner_radius = CornerRadius::same(radius::MEDIUM);
    let shadow = if dark { 96 } else { 28 };
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: egui::Color32::from_black_alpha(shadow),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: egui::Color32::from_black_alpha(shadow / 2),
    };
    v.faint_bg_color = p.raised;
    v.extreme_bg_color = p.surface;
    v.text_edit_bg_color = Some(p.surface);
    v.code_bg_color = p.raised;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    v.weak_text_color = Some(p.muted);
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.striped = false;
    v.slider_trailing_fill = true;
    v.indent_has_left_vline = false;
    v.collapsing_header_frame = false;

    // egui's own controls (also check boxes, which must stay square): the small radius. The
    // buttons of `widgets` draw their own, medium one.
    let small = CornerRadius::same(radius::SMALL);
    let medium = small;
    let w = &mut v.widgets;
    // Labels, separators, frames.
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = small;
    // Buttons and fields at rest: a surface with a visible border (3:1).
    w.inactive.bg_fill = p.surface;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_stroke = Stroke::new(1.0, p.control);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = medium;
    w.inactive.expansion = 0.0;
    // Under the pointer.
    w.hovered.bg_fill = p.raised;
    w.hovered.weak_bg_fill = p.raised;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = medium;
    w.hovered.expansion = 0.0;
    // Pressed, or a field being edited.
    w.active.bg_fill = p.accent_soft;
    w.active.weak_bg_fill = p.accent_soft;
    w.active.bg_stroke = Stroke::new(2.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.5, p.text);
    w.active.corner_radius = medium;
    w.active.expansion = 0.0;
    // An open menu or combo box.
    w.open.bg_fill = p.surface;
    w.open.weak_bg_fill = p.raised;
    w.open.bg_stroke = Stroke::new(1.0, p.accent);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = medium;
}
