//! Building blocks drawn from the tokens: buttons in four kinds, symbol buttons, chips, cards,
//! progress, segmented choice, field errors and empty states. Each tells the screen reader
//! (and the kittest tests) what it is through `widget_info`.

use egui::{
    Align2, Color32, CornerRadius, Frame, InnerResponse, Margin, Response, Sense, Stroke,
    StrokeKind, TextStyle, Ui, WidgetInfo, WidgetText, WidgetType, vec2,
};

use super::icons::{Icon, square};
use super::tokens::{Palette, palette, radius, size, space};

/// What a button does, which decides how it looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The one main action of a dialog or area: filled with the accent.
    Primary,
    /// Further actions: framed.
    Secondary,
    /// Small side actions: text only.
    Quiet,
    /// Deleting, discarding: red. Never right next to the primary button.
    Destructive,
}

/// Fill, frame and text colour of a button in one state.
fn colours(
    p: &Palette,
    kind: Kind,
    response: &Response,
    enabled: bool,
) -> (Color32, Stroke, Color32) {
    if !enabled {
        return (p.raised, Stroke::NONE, p.muted);
    }
    let hot = response.hovered() || response.has_focus();
    let down = response.is_pointer_button_down_on();
    match kind {
        Kind::Primary => {
            let fill = if hot || down {
                p.accent_hover
            } else {
                p.accent_fill
            };
            (fill, Stroke::NONE, p.on_accent)
        }
        Kind::Destructive => {
            let fill = if hot || down {
                p.danger_hover
            } else {
                p.danger_fill
            };
            (fill, Stroke::NONE, p.on_accent)
        }
        Kind::Secondary => {
            let fill = if down {
                p.accent_soft
            } else if hot {
                p.raised
            } else {
                p.surface
            };
            let border = if hot { p.accent } else { p.control };
            (fill, Stroke::new(1.0, border), p.text)
        }
        Kind::Quiet => {
            let fill = if down {
                p.accent_soft
            } else if hot {
                p.raised
            } else {
                Color32::TRANSPARENT
            };
            (fill, Stroke::NONE, p.accent)
        }
    }
}

/// A button with text: 32 high, rounded, coloured by its kind.
pub fn button(ui: &mut Ui, kind: Kind, text: &str) -> Response {
    button_with(ui, kind, None, text)
}

/// A button at least `width` wide (a column of equal buttons).
pub fn wide_button(ui: &mut Ui, kind: Kind, text: &str, width: f32) -> Response {
    draw_button(ui, kind, None, text, width)
}

/// A button with a symbol before its text.
pub fn button_with(ui: &mut Ui, kind: Kind, icon: Option<Icon>, text: &str) -> Response {
    draw_button(ui, kind, icon, text, size::CONTROL)
}

fn draw_button(
    ui: &mut Ui,
    kind: Kind,
    icon: Option<Icon>,
    text: &str,
    min_width: f32,
) -> Response {
    let enabled = ui.is_enabled();
    let galley = WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    );
    let icon_width = if icon.is_some() {
        size::ICON + space::XS + 2.0
    } else {
        0.0
    };
    let width = galley.size().x + icon_width + 2.0 * space::M;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width.max(min_width), size::CONTROL), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, text));
    hand(ui, &response);
    if ui.is_rect_visible(rect) {
        let p = palette(ui);
        let (fill, stroke, fore) = colours(p, kind, &response, enabled);
        let corner = CornerRadius::same(radius::MEDIUM);
        ui.painter()
            .rect(rect, corner, fill, stroke, StrokeKind::Inside);
        focus_ring(ui, &response, corner);
        let content = icon_width + galley.size().x;
        let mut x = rect.center().x - content / 2.0;
        if let Some(icon) = icon {
            icon.paint(
                ui.painter(),
                square(
                    egui::pos2(x + size::ICON / 2.0, rect.center().y),
                    size::ICON,
                ),
                fore,
            );
            x += icon_width;
        }
        let at = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(at, galley, fore);
    }
    response
}

/// A square button with only a symbol, at least 24 × 24 (WCAG 2.2): its name is spoken and
/// shown when the pointer rests on it.
pub fn icon_button(ui: &mut Ui, icon: Icon, name: &str) -> Response {
    let enabled = ui.is_enabled();
    let (rect, response) = ui.allocate_exact_size(vec2(size::TARGET, size::TARGET), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name));
    hand(ui, &response);
    if ui.is_rect_visible(rect) {
        let p = palette(ui);
        let corner = CornerRadius::same(radius::SMALL);
        if enabled && response.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, corner, p.accent_soft);
        } else if enabled && (response.hovered() || response.has_focus()) {
            ui.painter().rect_filled(rect, corner, p.raised);
        }
        focus_ring(ui, &response, corner);
        let color = if enabled { p.text } else { p.muted };
        icon.paint(ui.painter(), square(rect.center(), size::ICON), color);
    }
    response.on_hover_text(name)
}

/// The hand over something that can be clicked, as egui's own buttons show it.
fn hand(ui: &Ui, response: &Response) {
    if response.hovered()
        && ui.is_enabled()
        && let Some(cursor) = ui.visuals().interact_cursor
    {
        ui.ctx().set_cursor_icon(cursor);
    }
}

/// The ring around what has the keyboard focus (WCAG 2.4.7), in the accent.
fn focus_ring(ui: &Ui, response: &Response, corner: CornerRadius) {
    if response.has_focus() {
        let p = palette(ui);
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            corner + 2,
            Stroke::new(2.0, p.accent),
            StrokeKind::Outside,
        );
    }
}

/// The tone of a chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

/// A small label with a soft background: deadline, status, person. Its colour never carries
/// the meaning alone – the text says it.
pub fn chip(ui: &mut Ui, tone: Tone, text: &str) -> Response {
    let p = palette(ui);
    let (back, fore) = match tone {
        Tone::Neutral => (p.raised, p.text),
        Tone::Info => (p.info_soft, p.accent),
        Tone::Success => (p.success_soft, p.success),
        Tone::Warning => (p.warning_soft, p.warning),
        Tone::Danger => (p.danger_soft, p.danger),
    };
    let galley = WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Small,
    );
    let padding = vec2(space::S, 2.0);
    let (rect, response) = ui.allocate_exact_size(galley.size() + 2.0 * padding, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius::SMALL), back);
        ui.painter().galley(rect.min + padding, galley, fore);
    }
    response
}

/// A card: white (dark: raised) surface, thin frame, large corners.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let p = palette(ui);
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(CornerRadius::same(radius::LARGE))
        .inner_margin(Margin::same(12))
        .show(ui, add)
}

/// The frame of a dialog: surface, large corners, room inside, a soft shadow.
pub fn dialog_frame(ctx: &egui::Context) -> Frame {
    let style = ctx.global_style();
    let p = super::tokens::of(style.visuals.dark_mode);
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(CornerRadius::same(radius::LARGE))
        .inner_margin(Margin::same(24))
        .shadow(style.visuals.window_shadow)
}

/// A progress bar with its text next to it (never on top, where it cannot be read); one
/// element for the screen reader.
pub fn progress(ui: &mut Ui, fraction: f32, text: &str) -> Response {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let p = palette(ui);
    let galley = WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Small,
    );
    let bar = vec2(96.0, 6.0);
    let size = vec2(
        bar.x + space::S + galley.size().x,
        galley.size().y.max(bar.y),
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::ProgressIndicator, true, text);
        info.value = Some(f64::from(fraction));
        info
    });
    if ui.is_rect_visible(rect) {
        let track =
            egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - bar.y / 2.0), bar);
        let corner = CornerRadius::same(3);
        ui.painter().rect_filled(track, corner, p.line);
        if fraction > 0.0 {
            let mut done = track;
            done.set_width(track.width() * fraction);
            let fill = if fraction >= 1.0 { p.success } else { p.accent };
            ui.painter().rect_filled(done, corner, fill);
        }
        let at = egui::pos2(
            track.right() + space::S,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(at, galley, p.muted);
    }
    response
}

/// A choice of a few options side by side (like Vorgang | Projekt). Returns whether the
/// person changed it.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    let enabled = ui.is_enabled();
    let p = palette(ui);
    Frame::new()
        .stroke(Stroke::new(1.0, p.control))
        .corner_radius(CornerRadius::same(radius::MEDIUM))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                for (option, label) in options {
                    let selected = *value == *option;
                    let galley = WidgetText::from(*label).into_galley(
                        ui,
                        Some(egui::TextWrapMode::Extend),
                        f32::INFINITY,
                        TextStyle::Button,
                    );
                    let size = vec2(galley.size().x + 2.0 * space::M, size::ROW - 6.0);
                    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                    response.widget_info(|| {
                        WidgetInfo::selected(WidgetType::RadioButton, enabled, selected, *label)
                    });
                    if !selected {
                        hand(ui, &response);
                    }
                    if response.clicked() && !selected {
                        *value = *option;
                        changed = true;
                    }
                    let corner = CornerRadius::same(radius::SMALL);
                    let (fill, fore) = if selected {
                        (p.accent_soft, p.accent)
                    } else if response.hovered() {
                        (p.raised, p.text)
                    } else {
                        (Color32::TRANSPARENT, p.text)
                    };
                    ui.painter().rect_filled(rect, corner, fill);
                    focus_ring(ui, &response, corner);
                    ui.painter()
                        .galley(rect.center() - galley.size() / 2.0, galley, fore);
                }
            });
        });
    changed
}

/// What is wrong with a field, right under it: a symbol and a sentence in red.
pub fn field_error(ui: &mut Ui, message: &str) -> Response {
    let p = palette(ui);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(size::ICON, size::ICON), Sense::hover());
        Icon::Warning.paint(ui.painter(), rect, p.danger);
        ui.label(egui::RichText::new(message).color(p.danger))
    })
    .inner
}

/// Nothing here yet: a symbol, one sentence and (optionally) the one action that helps.
/// Returns whether the action was chosen.
pub fn empty_state(ui: &mut Ui, icon: Icon, sentence: &str, action: Option<&str>) -> bool {
    let p = palette(ui);
    let mut chosen = false;
    ui.vertical_centered(|ui| {
        ui.add_space(space::XL);
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        icon.paint(ui.painter(), rect, p.muted);
        ui.add_space(space::S);
        ui.label(egui::RichText::new(sentence).color(p.muted));
        if let Some(action) = action {
            ui.add_space(space::S);
            chosen = button(ui, Kind::Primary, action).clicked();
        }
        ui.add_space(space::XL);
    });
    chosen
}

/// A heading of a section (18 pt, semibold).
pub fn subheading(ui: &mut Ui, text: &str) -> Response {
    // Until the fonts are set up (the very first frame) the style does not exist yet.
    let style = super::subheading();
    if ui.style().text_styles.contains_key(&style) {
        ui.label(egui::RichText::new(text).text_style(style))
    } else {
        ui.label(egui::RichText::new(text).strong())
    }
}

/// Writes `text` centred at `pos` in the given style and colour (for painted views).
pub fn paint_text(ui: &Ui, pos: egui::Pos2, text: &str, style: &TextStyle, color: Color32) {
    let font = style.resolve(ui.style());
    ui.painter()
        .text(pos, Align2::CENTER_CENTER, text, font, color);
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    #[derive(Default)]
    struct Probe {
        clicks: Vec<&'static str>,
        choice: u8,
        changed: bool,
    }

    fn harness() -> Harness<'static, Probe> {
        Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui_state(
                |ui, probe: &mut Probe| {
                    super::super::install(ui.ctx());
                    for (kind, name) in [
                        (Kind::Primary, "Anlegen"),
                        (Kind::Secondary, "Abbrechen"),
                        (Kind::Quiet, "Mehr"),
                        (Kind::Destructive, "Löschen"),
                    ] {
                        if button(ui, kind, name).clicked() {
                            probe.clicks.push(name);
                        }
                    }
                    if icon_button(ui, Icon::Today, "Heute").clicked() {
                        probe.clicks.push("Heute");
                    }
                    chip(ui, Tone::Danger, "überfällig");
                    progress(ui, 0.4, "2 von 5 erledigt");
                    probe.changed |=
                        segmented(ui, &mut probe.choice, &[(0, "Vorgang"), (1, "Projekt")]);
                    field_error(ui, "Titel fehlt");
                    if empty_state(
                        ui,
                        Icon::Task,
                        "Noch keine Aufgaben.",
                        Some("Aufgabe anlegen"),
                    ) {
                        probe.clicks.push("Aufgabe anlegen");
                    }
                },
                Probe::default(),
            )
    }

    #[test]
    fn every_block_is_named_and_buttons_answer_clicks() {
        let mut harness = harness();
        harness.run();
        for name in [
            "Anlegen",
            "Abbrechen",
            "Mehr",
            "Löschen",
            "Heute",
            "Aufgabe anlegen",
        ] {
            harness.get_by_label(name).click();
            harness.run();
        }
        assert_eq!(
            harness.state().clicks,
            [
                "Anlegen",
                "Abbrechen",
                "Mehr",
                "Löschen",
                "Heute",
                "Aufgabe anlegen"
            ]
        );
        for name in [
            "überfällig",
            "2 von 5 erledigt",
            "Titel fehlt",
            "Noch keine Aufgaben.",
        ] {
            assert!(harness.query_by_label(name).is_some(), "{name}");
        }
    }

    #[test]
    fn the_segmented_choice_changes_once() {
        let mut harness = harness();
        harness.run();
        harness.get_by_label("Projekt").click();
        harness.run();
        assert_eq!(harness.state().choice, 1);
        assert!(harness.state().changed);
        harness.state_mut().changed = false;
        harness.get_by_label("Projekt").click();
        harness.run();
        assert!(
            !harness.state().changed,
            "choosing the chosen one changes nothing"
        );
    }

    #[test]
    fn targets_are_large_enough() {
        let mut harness = harness();
        harness.run();
        for name in ["Anlegen", "Heute", "Vorgang", "Projekt"] {
            let rect = harness.get_by_label(name).rect();
            assert!(
                rect.width() >= size::TARGET && rect.height() >= size::TARGET,
                "{name}: {rect:?}"
            );
        }
        assert!(harness.get_by_label("Anlegen").rect().height() >= size::CONTROL);
    }

    #[test]
    fn the_pointer_becomes_a_hand_over_every_button() {
        let mut harness = harness();
        harness.run();
        for name in ["Anlegen", "Heute", "Projekt"] {
            harness.hover_at(harness.get_by_label(name).rect().center());
            harness.run();
            assert_eq!(
                harness.output().platform_output.cursor_icon,
                egui::CursorIcon::PointingHand,
                "{name}"
            );
        }
    }

    #[test]
    fn a_disabled_button_does_not_answer() {
        let mut harness = Harness::builder().build_ui_state(
            |ui, clicked: &mut bool| {
                ui.add_enabled_ui(false, |ui| {
                    *clicked |= button(ui, Kind::Primary, "Anlegen").clicked();
                    *clicked |= icon_button(ui, Icon::Delete, "Löschen").clicked();
                });
            },
            false,
        );
        harness.run();
        harness.get_by_label("Anlegen").click();
        harness.get_by_label("Löschen").click();
        harness.run();
        assert!(!harness.state());
    }

    #[test]
    fn progress_survives_odd_values() {
        for fraction in [f32::NAN, -1.0, 0.0, 1.0, 7.0, f32::INFINITY] {
            let mut harness = Harness::new_ui(|ui| {
                progress(ui, fraction, "x");
            });
            harness.run();
        }
    }
}
