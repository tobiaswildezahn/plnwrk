//! The design tokens: colours by role (light and dark), spacing, corner radii and type sizes.
//! The only place with colour values (a guard test in `design/tests.rs` checks that); views ask
//! for a role through [`palette`].
//!
//! Phase 9a (docs/plans/2026-10-17-phase-9-gestaltung.md §5): warm off-white instead of grey,
//! one accent (the dark Hamburg blue of the app icon), status colours only for status. Every
//! pair of text and background reaches WCAG AA (4.5:1), every control border 3:1.

use egui::Color32;

/// Colours by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Behind everything (side bar, panels).
    pub background: Color32,
    /// Cards, dialogs, fields.
    pub surface: Color32,
    /// Raised a little: hover, alternate rows, quiet areas.
    pub raised: Color32,
    /// Set apart without a frame: days off in the schedule.
    pub shade: Color32,
    /// Separators and the frames of cards (decorative).
    pub line: Color32,
    /// The border of a control (field, secondary button): 3:1 against the surface.
    pub control: Color32,
    /// Text.
    pub text: Color32,
    /// Secondary text (dates, hints).
    pub muted: Color32,
    /// The accent as text or line (links, focus, selection).
    pub accent: Color32,
    /// The accent as a fill (primary buttons); text on it is [`Palette::on_accent`].
    pub accent_fill: Color32,
    /// The accent fill under the pointer.
    pub accent_hover: Color32,
    /// The accent, softly: selected rows, active tab.
    pub accent_soft: Color32,
    /// Text on the accent fill and on the danger fill.
    pub on_accent: Color32,
    /// Overdue, critical, errors: as text.
    pub danger: Color32,
    /// Destructive buttons.
    pub danger_fill: Color32,
    /// Destructive buttons under the pointer.
    pub danger_hover: Color32,
    /// Behind danger text.
    pub danger_soft: Color32,
    /// At risk: as text.
    pub warning: Color32,
    /// At risk: marks and symbols (3:1).
    pub warning_mark: Color32,
    /// Behind warning text.
    pub warning_soft: Color32,
    /// Done: as text.
    pub success: Color32,
    /// Behind success text.
    pub success_soft: Color32,
    /// Information, behind accent text.
    pub info_soft: Color32,
    /// Contextual ribbon tabs (a chosen task, the schedule): as text and mark.
    pub context: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    // The masks keep each value in 0..=255, so the casts never cut anything off.
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// Light mode: warm off-white, white surfaces, dark blue accent.
pub const LIGHT: Palette = Palette {
    background: hex(0xF7F7F5),
    surface: hex(0xFFFFFF),
    raised: hex(0xF0F1F3),
    shade: hex(0xECEDEF),
    line: hex(0xE3E5E8),
    control: hex(0x7D8591),
    text: hex(0x1F2630),
    muted: hex(0x5B6472),
    accent: hex(0x1D4F91),
    accent_fill: hex(0x1D4F91),
    accent_hover: hex(0x163E73),
    accent_soft: hex(0xDCE7F5),
    on_accent: hex(0xFFFFFF),
    danger: hex(0xB42318),
    danger_fill: hex(0xB42318),
    danger_hover: hex(0x8F1C13),
    danger_soft: hex(0xFCE9E7),
    warning: hex(0x8A5300),
    warning_mark: hex(0xB7791F),
    warning_soft: hex(0xFFF1D6),
    success: hex(0x1C6E46),
    success_soft: hex(0xE3F2EA),
    info_soft: hex(0xE6EEF8),
    context: hex(0x0F766E),
};

/// Dark mode: near black blue-grey, the accent lighter so it reads on dark.
pub const DARK: Palette = Palette {
    background: hex(0x16181C),
    surface: hex(0x1F2228),
    raised: hex(0x2A2E36),
    shade: hex(0x24272D),
    line: hex(0x343941),
    control: hex(0x7C8492),
    text: hex(0xE8EAED),
    muted: hex(0xA3AAB5),
    accent: hex(0x8DB3E8),
    accent_fill: hex(0x3F74BF),
    accent_hover: hex(0x3A6EB8),
    accent_soft: hex(0x243650),
    on_accent: hex(0xFFFFFF),
    danger: hex(0xF28B82),
    danger_fill: hex(0xC8443A),
    danger_hover: hex(0xB03A30),
    danger_soft: hex(0x3D2424),
    warning: hex(0xF2C46D),
    warning_mark: hex(0xE0A43C),
    warning_soft: hex(0x3A3020),
    success: hex(0x7ACFA3),
    success_soft: hex(0x1F3529),
    info_soft: hex(0x22324A),
    context: hex(0x5CC8B2),
};

/// The palette of the mode this ui is drawn in.
pub fn palette(ui: &egui::Ui) -> &'static Palette {
    of(ui.visuals().dark_mode)
}

/// The palette of light (`false`) or dark (`true`) mode.
pub const fn of(dark: bool) -> &'static Palette {
    if dark { &DARK } else { &LIGHT }
}

/// Spacing on a 4 px grid.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Corner radii: never rounder than this (KERN).
pub mod radius {
    /// Fields, chips.
    pub const SMALL: u8 = 4;
    /// Buttons, bars.
    pub const MEDIUM: u8 = 6;
    /// Cards, dialogs.
    pub const LARGE: u8 = 8;
}

/// Type sizes in points.
pub mod size {
    /// Running text, buttons, fields.
    pub const BODY: f32 = 14.0;
    /// Secondary text; nothing is smaller.
    pub const SMALL: f32 = 12.5;
    /// Section titles.
    pub const SUBHEADING: f32 = 18.0;
    /// View titles, dialog titles.
    pub const HEADING: f32 = 22.0;
    /// The name of the app on the start page.
    pub const DISPLAY: f32 = 32.0;
    /// Height of buttons with text.
    pub const CONTROL: f32 = 32.0;
    /// Height of a row in lists, forms and the schedule.
    pub const ROW: f32 = 32.0;
    /// The smallest target for the pointer (WCAG 2.2, 2.5.8), also for small symbol buttons.
    pub const TARGET: f32 = 24.0;
    /// A symbol inside a button.
    pub const ICON: f32 = 16.0;
}
