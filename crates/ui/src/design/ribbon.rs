//! The building blocks of the ribbon (Phase 9b): tabs, groups, large and small buttons, columns
//! and the arrows for a ribbon wider than the window.
//!
//! Adapted from RustGIS (opengeos/RustGIS, Apache-2.0, commit 72b5829,
//! `crates/ui-egui/src/widgets.rs` `big`/`small`/`group`/`col`/`bg` and
//! `crates/ui-egui/src/ribbon.rs` `tab_strip`/`scroll_buttons`): same mechanics, plnwrk's
//! tokens and smallest sizes (text 12.5, targets 24), a focus ring and the hand pointer.

use egui::{
    Align2, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, TextStyle, Ui, pos2, vec2,
};

use super::icons::{Icon, square};
use super::tokens::{palette, radius, size};

/// Height of the controls of a group (three small buttons, or a large one).
pub const CONTENT_H: f32 = 3.0 * size::TARGET + 4.0;
/// Height of the group names under the controls.
pub const LABEL_H: f32 = 20.0;
/// Height of the tab strip.
pub const TAB_H: f32 = 32.0;
/// Width of the arrows at the edges of a ribbon wider than the window.
pub const ARROW_W: f32 = 20.0;

/// Width of a text in a text style.
fn text_width(ui: &Ui, text: &str, style: &TextStyle) -> f32 {
    let font = style.resolve(ui.style());
    ui.fonts_mut(|f| {
        f.layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
            .size()
            .x
    })
}

/// Splits a long label of a large button into two lines, at the space nearest its middle.
fn two_lines(ui: &Ui, label: &str) -> Vec<String> {
    let style = TextStyle::Small;
    if text_width(ui, label, &style) <= 72.0 || !label.contains(' ') {
        return vec![label.to_owned()];
    }
    let middle = label.len() / 2;
    let split = label
        .match_indices(' ')
        .map(|(i, _)| i)
        .min_by_key(|i| i.abs_diff(middle))
        .unwrap_or(middle);
    let (first, rest) = label.split_at(split);
    vec![first.to_owned(), rest.trim_start().to_owned()]
}

/// The background of a button: chosen, under the pointer, pressed; and the focus ring.
fn background(ui: &Ui, rect: Rect, response: &Response, enabled: bool, chosen: bool) {
    let p = palette(ui);
    let corner = CornerRadius::same(radius::MEDIUM);
    let fill = if (enabled && response.is_pointer_button_down_on()) || chosen {
        Some(p.accent_soft)
    } else if enabled && response.hovered() {
        Some(p.raised)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, corner, fill);
    }
    if chosen {
        ui.painter()
            .rect_stroke(rect, corner, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    }
    focus_ring(ui, response, rect, corner);
    if enabled
        && response.hovered()
        && let Some(cursor) = ui.visuals().interact_cursor
    {
        ui.ctx().set_cursor_icon(cursor);
    }
}

/// Clickable and reachable with Tab when enabled; a greyed button is skipped by the keyboard.
fn sense(enabled: bool) -> Sense {
    if enabled {
        Sense::click()
    } else {
        Sense::hover()
    }
}

/// The ring around what has the keyboard focus (WCAG 2.4.7).
fn focus_ring(ui: &Ui, response: &Response, rect: Rect, corner: CornerRadius) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            corner + 1,
            Stroke::new(2.0, palette(ui).accent),
            StrokeKind::Outside,
        );
    }
}

/// What the screen reader hears: the label, and whether it is the current choice.
fn speak(response: &Response, enabled: bool, chosen: bool, label: &str) {
    response.widget_info(|| {
        if chosen {
            egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, true, label)
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label)
        }
    });
}

/// The tooltip: the label and the shortcut.
fn hint(response: Response, label: &str, shortcut: Option<&str>) -> Response {
    match shortcut {
        Some(keys) => response.on_hover_text(format!("{label} ({keys})")),
        None => response.on_hover_text(label),
    }
}

/// A large button: a 32 px symbol over one or two lines of text. `name`: spoken and shown as
/// tooltip (the label with what the group adds, e.g. "Neuer Vorgang").
pub fn big(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    name: &str,
    shortcut: Option<&str>,
    enabled: bool,
    chosen: bool,
) -> Response {
    let lines = two_lines(ui, label);
    let style = TextStyle::Small;
    let widest = lines
        .iter()
        .map(|l| text_width(ui, l, &style))
        .fold(0.0, f32::max);
    let (rect, response) =
        ui.allocate_exact_size(vec2((widest + 16.0).max(56.0), CONTENT_H), sense(enabled));
    speak(&response, enabled, chosen, name);
    if ui.is_rect_visible(rect) {
        background(ui, rect, &response, enabled, chosen);
        let p = palette(ui);
        let fore = if enabled { p.text } else { p.muted };
        let symbol = if enabled && !chosen { p.accent } else { fore };
        icon.paint(
            ui.painter(),
            square(pos2(rect.center().x, rect.top() + 24.0), 32.0),
            symbol,
        );
        let font = style.resolve(ui.style());
        let mut y = rect.top() + 44.0;
        for line in &lines {
            ui.painter().text(
                pos2(rect.center().x, y),
                Align2::CENTER_TOP,
                line,
                font.clone(),
                fore,
            );
            y += 15.0;
        }
    }
    hint(response, name, shortcut)
}

/// A small button: a 16 px symbol and the text in a row, 24 high.
pub fn small(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    name: &str,
    shortcut: Option<&str>,
    enabled: bool,
    chosen: bool,
) -> Response {
    let style = TextStyle::Small;
    let width = 28.0 + text_width(ui, label, &style) + 8.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::TARGET), sense(enabled));
    speak(&response, enabled, chosen, name);
    if ui.is_rect_visible(rect) {
        background(ui, rect, &response, enabled, chosen);
        let p = palette(ui);
        let fore = if enabled { p.text } else { p.muted };
        let symbol = if enabled && !chosen { p.accent } else { fore };
        icon.paint(
            ui.painter(),
            square(pos2(rect.left() + 14.0, rect.center().y), size::ICON),
            symbol,
        );
        ui.painter().text(
            pos2(rect.left() + 28.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            style.resolve(ui.style()),
            fore,
        );
    }
    hint(response, name, shortcut)
}

/// A group: its controls left to right over its name, then a separator.
pub fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let p = palette(ui);
    let start = ui.cursor().min;
    let width = ui
        .vertical(|ui| {
            let controls = ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.set_min_height(CONTENT_H);
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                add(ui);
            });
            let width = controls
                .response
                .rect
                .width()
                .max(text_width(ui, title, &TextStyle::Small) + 12.0);
            ui.allocate_exact_size(vec2(width, LABEL_H), Sense::hover());
            width
        })
        .inner;
    ui.painter().text(
        pos2(start.x + width / 2.0, start.y + CONTENT_H + LABEL_H / 2.0),
        Align2::CENTER_CENTER,
        title,
        TextStyle::Small.resolve(ui.style()),
        p.muted,
    );
    ui.add_space(6.0);
    let x = ui.cursor().min.x;
    ui.painter().line_segment(
        [
            pos2(x, start.y + 6.0),
            pos2(x, start.y + CONTENT_H + LABEL_H - 4.0),
        ],
        Stroke::new(1.0, p.line),
    );
    ui.add_space(7.0);
}

/// A column of up to three small buttons.
pub fn column(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        add(ui);
    });
}

/// A tab of the tab strip. Contextual tabs carry their colour (text and a mark on top).
pub fn tab(ui: &mut Ui, label: &str, active: bool, contextual: bool) -> Response {
    let p = palette(ui);
    let style = TextStyle::Body;
    let width = text_width(ui, label, &style) + 28.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, TAB_H), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label)
    });
    let body = Rect::from_min_max(pos2(rect.left(), rect.top() + 3.0), rect.max);
    let top = CornerRadius {
        nw: radius::MEDIUM,
        ne: radius::MEDIUM,
        sw: 0,
        se: 0,
    };
    if active {
        // The active tab merges into the ribbon below it.
        ui.painter().rect_filled(body, top, p.surface);
        ui.painter()
            .rect_stroke(body, top, Stroke::new(1.0, p.line), StrokeKind::Inside);
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(rect.left() + 1.0, rect.bottom() - 1.5),
                pos2(rect.right() - 1.0, rect.bottom() + 1.0),
            ),
            0.0,
            p.surface,
        );
        let mark = if contextual { p.context } else { p.accent };
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(rect.left() + 8.0, rect.bottom() - 3.0),
                pos2(rect.right() - 8.0, rect.bottom() - 1.0),
            ),
            CornerRadius::same(1),
            mark,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(
            body.shrink2(vec2(1.0, 2.0)),
            CornerRadius::same(radius::SMALL),
            p.raised,
        );
    }
    focus_ring(ui, &response, body, top);
    if contextual {
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(rect.left() + 4.0, rect.top() + 3.0),
                pos2(rect.right() - 4.0, rect.top() + 5.0),
            ),
            CornerRadius::same(1),
            p.context,
        );
    }
    let color = if contextual {
        p.context
    } else if active {
        p.text
    } else {
        p.muted
    };
    let font = if active {
        super::strong_font(ui.ctx(), size::BODY)
    } else {
        style.resolve(ui.style())
    };
    ui.painter().text(
        pos2(rect.center().x, rect.center().y + 1.5),
        Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
    if response.hovered()
        && let Some(cursor) = ui.visuals().interact_cursor
    {
        ui.ctx().set_cursor_icon(cursor);
    }
    response
}

/// Arrows at the edges of a horizontal scroll area wider than its room (the scroll bar would
/// cover the group names, and the wheel alone is hard to find). `names`: left, right.
pub fn scroll_arrows<R>(
    ui: &mut Ui,
    scroll: &egui::scroll_area::ScrollAreaOutput<R>,
    names: [&str; 2],
) {
    let view = scroll.inner_rect;
    let most = (scroll.content_size.x - view.width()).max(0.0);
    if most < 1.0 {
        return;
    }
    let p = palette(ui);
    let mut state = scroll.state;
    let offset = state.offset.x;
    let step = (view.width() * 0.6).max(120.0);
    let mut moved = false;
    for (left, show) in [(true, offset > 0.5), (false, offset < most - 0.5)] {
        if !show {
            continue;
        }
        let x = if left {
            view.left()
        } else {
            view.right() - ARROW_W
        };
        let rect = Rect::from_min_size(pos2(x, view.top()), vec2(ARROW_W, view.height()));
        let response = ui.interact(rect, scroll.id.with(left), Sense::click());
        let name = if left { names[0] } else { names[1] };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
        let fill = if response.hovered() {
            p.raised
        } else {
            p.surface
        };
        let corner = CornerRadius::same(radius::SMALL);
        ui.painter().rect_filled(rect, corner, fill);
        ui.painter()
            .rect_stroke(rect, corner, Stroke::new(1.0, p.line), StrokeKind::Inside);
        let icon = if left {
            Icon::ChevronLeft
        } else {
            Icon::ChevronRight
        };
        icon.paint(ui.painter(), square(rect.center(), size::ICON), p.muted);
        focus_ring(ui, &response, rect, corner);
        if response.on_hover_text(name).clicked() {
            state.offset.x = if left {
                (offset - step).max(0.0)
            } else {
                (offset + step).min(most)
            };
            moved = true;
        }
    }
    if moved {
        state.store(ui.ctx(), scroll.id);
        ui.ctx().request_repaint();
    }
}
