//! The ribbon (Phase 9b): tabs over groups of buttons, in place of the menu bar. What it offers
//! comes from [`crate::actions`]; this file only lays it out. It collapses to its tabs (double
//! click on a tab, the arrow at the right, Ctrl/Cmd+F1, on the Mac also Cmd+Option+R) – by
//! default only in very low windows, decided once at start – and a click on a tab then opens it
//! for a moment over the content ("peek"), until a click elsewhere, an action or Escape.

use egui::{Frame, Margin, Panel, vec2};

use crate::actions::{self, Action, Group, RibbonTab, Size};
use crate::design::ribbon::{self, CONTENT_H, LABEL_H, TAB_H};
use crate::design::{Icon, palette};
use crate::doc::OpenDoc;
use crate::state::State;
use crate::views::keys;
use crate::{Intent, text};

/// Below this window height the ribbon starts collapsed (unless the person chose otherwise).
/// Laptops (800 px and more, a browser on them about 700) start with it open: a newcomer must
/// see the commands.
const LOW_WINDOW: f32 = 640.0;

/// Whether the ribbon shows only its tabs now: as the person chose, otherwise as decided at
/// start (so it never jumps while the window is resized).
pub fn collapsed(ctx: &egui::Context, state: &mut State) -> bool {
    if let Some(collapsed) = state.ribbon_collapsed {
        return collapsed;
    }
    *state
        .ribbon_default
        .get_or_insert_with(|| ctx.content_rect().height() < LOW_WINDOW)
}

/// Collapses or expands for good (the person's choice, remembered).
fn toggle(state: &mut State, collapsed: &mut bool) {
    *collapsed = !*collapsed;
    state.ribbon_collapsed = Some(*collapsed);
    state.ribbon_peek = false;
}

pub fn show(ui: &mut egui::Ui, doc: Option<&OpenDoc>, state: &mut State, out: &mut Vec<Intent>) {
    let tabs = actions::tabs(doc, state);
    if !tabs.contains(&state.ribbon_tab) {
        state.ribbon_tab = RibbonTab::Start;
    }
    let mut collapsed = collapsed(ui.ctx(), state);
    // Cmd+Option+R only in the Mac app: Safari reloads the page with it, and elsewhere
    // Ctrl+Alt is AltGr (AltGr+R types a character).
    let mac = cfg!(target_os = "macos") && !state.browser;
    if state.dialog.is_none()
        && ui.input_mut(|i| {
            i.consume_shortcut(&keys::RIBBON) || (mac && i.consume_shortcut(&keys::RIBBON_MAC))
        })
    {
        toggle(state, &mut collapsed);
    }
    // An expanded ribbon or a dialog ends a peek.
    if !collapsed || state.dialog.is_some() {
        state.ribbon_peek = false;
    }
    // Escape ends a peek, and the focus goes back to the tab it was opened from. (A field
    // outside the peek cannot have the focus meanwhile: that ends the peek, see below.)
    let mut focus_tab = false;
    if state.ribbon_peek
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        state.ribbon_peek = false;
        focus_tab = true;
    }
    // The keyboard focus went elsewhere (Tab out of the peek): the peek must not hide it
    // (WCAG 2.4.11). Checked against where the ribbon was last frame.
    let last = egui::Id::new("ribbon-peek-rect");
    if state.ribbon_peek
        && let Some(shown) = ui.data(|d| d.get_temp::<egui::Rect>(last))
        && let Some(focused) = ui.memory(|m| m.focused())
        && let Some(widget) = ui.ctx().read_response(focused)
        && !shown.contains_rect(widget.rect)
    {
        state.ribbon_peek = false;
    }
    let mut all = actions::all(doc, state);
    let mut chosen = None;
    let p = palette(ui);

    let strip = Panel::top("ribbon-tabs")
        .exact_size(TAB_H + 2.0)
        .show_separator_line(false)
        .frame(Frame::NONE.fill(p.background).inner_margin(Margin {
            left: 8,
            right: 8,
            top: 2,
            bottom: 0,
        }))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                for tab in &tabs {
                    let response =
                        ribbon::tab(ui, tab.name(), *tab == state.ribbon_tab, tab.contextual());
                    if focus_tab && *tab == state.ribbon_tab {
                        response.request_focus();
                    }
                    // A double click also sends a click first: it is checked first.
                    if response.double_clicked() {
                        toggle(state, &mut collapsed);
                        state.ribbon_tab = *tab;
                    } else if response.clicked() {
                        if collapsed {
                            state.ribbon_peek = !(state.ribbon_peek && state.ribbon_tab == *tab);
                        }
                        state.ribbon_tab = *tab;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = &text::t().ribbon;
                    let (icon, name) = if collapsed {
                        (Icon::ChevronDown, r.expand)
                    } else {
                        (Icon::ChevronUp, r.collapse)
                    };
                    let name = format!("{name} ({})", ui.ctx().format_shortcut(&keys::RIBBON));
                    if crate::design::widgets::icon_button(ui, icon, &name).clicked() {
                        toggle(state, &mut collapsed);
                    }
                    ui.add_space(8.0);
                    // Quick access: undo and redo stay at hand with the ribbon collapsed too.
                    for index in [
                        actions::index_of(&all, Group::Undo, 1),
                        actions::index_of(&all, Group::Undo, 0),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let Some(action) = all.get(index) {
                            // With its shortcut, as Office names them (and so apart from the
                            // same action in the tab Start).
                            let name = match action.shortcut {
                                Some(keys) => {
                                    format!(
                                        "{} ({})",
                                        action.label,
                                        ui.ctx().format_shortcut(&keys)
                                    )
                                }
                                None => action.label.clone(),
                            };
                            let response = ui.add_enabled_ui(action.enabled, |ui| {
                                crate::design::widgets::icon_button(ui, action.icon, &name)
                            });
                            if response.inner.clicked() {
                                chosen = Some(index);
                            }
                        }
                    }
                });
            });
        });

    if collapsed && !state.ribbon_peek {
        take(&mut all, chosen, state, out);
        return;
    }
    // A ribbon opened for a moment lies over the content (it does not push it down) and closes
    // on the next click outside it and its tabs.
    let peek_click = state.ribbon_peek && ui.input(|i| i.pointer.any_click());
    let tab = state.ribbon_tab;
    let height = CONTENT_H + LABEL_H + 10.0;
    let margin = Margin {
        left: 8,
        right: 8,
        top: 4,
        bottom: 4,
    };
    let body = if state.ribbon_peek {
        let below = strip.response.rect;
        egui::Area::new(egui::Id::new("ribbon-peek"))
            .order(egui::Order::Foreground)
            .fixed_pos(below.left_bottom())
            .show(ui.ctx(), |ui| {
                Frame::NONE
                    .fill(p.surface)
                    .inner_margin(margin)
                    .shadow(ui.visuals().popup_shadow)
                    .show(ui, |ui| {
                        ui.set_width(below.width() - 16.0);
                        ui.set_height(height - 8.0);
                        groups(ui, tab, &all, &mut chosen);
                    });
            })
            .response
            .rect
    } else {
        Panel::top("ribbon")
            .exact_size(height)
            .show_separator_line(false)
            .frame(Frame::NONE.fill(p.surface).inner_margin(margin))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.left() - 8.0..=rect.right() + 8.0,
                    rect.bottom() + 4.5,
                    egui::Stroke::new(1.0, p.line),
                );
                groups(ui, tab, &all, &mut chosen);
            })
            .response
            .rect
    };
    // Tabs and peek together, for the focus check of the next frame.
    let shown = body.union(strip.response.rect);
    ui.data_mut(|d| d.insert_temp(last, shown));
    if peek_click {
        let at = ui.input(|i| i.pointer.interact_pos());
        let inside = at.is_some_and(|at| body.contains(at) || strip.response.rect.contains(at));
        if !inside {
            state.ribbon_peek = false;
        }
    }
    take(&mut all, chosen, state, out);
}

/// The groups of a tab, scrolled sideways with arrows when wider than the window.
fn groups(ui: &mut egui::Ui, tab: RibbonTab, all: &[Action], chosen: &mut Option<usize>) {
    let scroll = egui::ScrollArea::horizontal()
        .id_salt("ribbon-body")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                for group in Group::ALL.iter().filter(|g| g.tab() == tab) {
                    let members: Vec<usize> = all
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| a.group == *group)
                        .map(|(i, _)| i)
                        .collect();
                    if members.is_empty() {
                        continue;
                    }
                    ribbon::group(ui, group.name(), |ui| {
                        lay_out(ui, all, &members, chosen);
                    });
                }
            });
        });
    let r = &text::t().ribbon;
    ribbon::scroll_arrows(ui, &scroll, [r.scroll_left, r.scroll_right]);
}

/// Large buttons one by one, small ones in columns of three.
fn lay_out(ui: &mut egui::Ui, all: &[Action], members: &[usize], chosen: &mut Option<usize>) {
    let mut column: Vec<usize> = Vec::new();
    let flush = |ui: &mut egui::Ui, column: &mut Vec<usize>, chosen: &mut Option<usize>| {
        if column.is_empty() {
            return;
        }
        ribbon::column(ui, |ui| {
            for index in column.iter() {
                if let Some(action) = all.get(*index)
                    && button(ui, action, Size::Small)
                {
                    *chosen = Some(*index);
                }
            }
        });
        column.clear();
    };
    for index in members {
        let Some(action) = all.get(*index) else {
            continue;
        };
        match action.size {
            Size::Large => {
                flush(ui, &mut column, chosen);
                if button(ui, action, Size::Large) {
                    *chosen = Some(*index);
                }
            }
            Size::Small => {
                column.push(*index);
                if column.len() == 3 {
                    flush(ui, &mut column, chosen);
                }
            }
        }
    }
    flush(ui, &mut column, chosen);
}

/// Draws one action; whether it was chosen.
fn button(ui: &mut egui::Ui, action: &Action, size: Size) -> bool {
    let shortcut = action.shortcut.map(|s| ui.ctx().format_shortcut(&s));
    let response = match size {
        Size::Large => ribbon::big(
            ui,
            action.icon,
            &action.label,
            action.full_name(),
            shortcut.as_deref(),
            action.enabled,
            action.chosen,
        ),
        Size::Small => ribbon::small(
            ui,
            action.icon,
            &action.label,
            action.full_name(),
            shortcut.as_deref(),
            action.enabled,
            action.chosen,
        ),
    };
    response.clicked() && action.enabled
}

/// Carries out the chosen action; a ribbon opened for a moment closes with it.
fn take(all: &mut Vec<Action>, chosen: Option<usize>, state: &mut State, out: &mut Vec<Intent>) {
    if let Some(index) = chosen
        && index < all.len()
    {
        let action = all.swap_remove(index);
        state.ribbon_peek = false;
        out.extend(action.intents);
    }
}
