//! The views. They read the workspace and push [`Intent`]s; they never change data themselves.

mod board;
mod date;
mod detail;
mod dialogs;
mod overview;
mod people;
mod project;

use egui::{Key, KeyboardShortcut, Modifiers, RichText};
use plnwrk_core::{Classification, TaskStatus};
use plnwrk_engine::query;

use crate::doc::OpenDoc;
use crate::state::{Dialog, FileAction, ProjectForm, State, TemplateForm, View};
use crate::{Intent, text};

/// Keyboard shortcuts. Undo/redo only when no text field is being edited (text fields have
/// their own undo).
pub fn shortcuts(ctx: &egui::Context, open: bool, out: &mut Vec<Intent>) {
    let typing = ctx.egui_wants_keyboard_input();
    let save = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
    let save_as = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
    let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
    let redo = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    let open_file = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
    let quit = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
    ctx.input_mut(|input| {
        if input.consume_shortcut(&quit) {
            out.push(Intent::File(FileAction::Quit));
            return;
        }
        if input.consume_shortcut(&open_file) {
            out.push(Intent::File(FileAction::Open));
        }
        if !open {
            return;
        }
        // Longer shortcuts first: Cmd+Shift+S also matches Cmd+S otherwise.
        if input.consume_shortcut(&save_as) {
            out.push(Intent::File(FileAction::SaveAs));
        } else if input.consume_shortcut(&save) {
            out.push(Intent::File(FileAction::Save));
        }
        if typing {
            return;
        }
        if input.consume_shortcut(&redo) {
            out.push(Intent::run("redo", serde_json::json!({})));
        } else if input.consume_shortcut(&undo) {
            out.push(Intent::run("undo", serde_json::json!({})));
        }
    });
}

/// The whole window.
pub fn show(ui: &mut egui::Ui, doc: Option<&OpenDoc>, state: &mut State, out: &mut Vec<Intent>) {
    menu_bar(ui, doc, out);
    status_bar(ui, doc, state);
    match doc {
        None => {
            egui::CentralPanel::default().show(ui, |ui| start(ui, state, out));
        }
        Some(doc) => {
            sidebar(ui, doc, state, out);
            if let Some(task) = state.selected
                && matches!(state.view, View::Project(_))
            {
                detail::panel(ui, doc, task, state, out);
            }
            egui::CentralPanel::default().show(ui, |ui| match state.view {
                View::Overview => overview::show(ui, doc, out),
                View::Project(id) => project::show(ui, doc, id, state, out),
                View::Templates => overview::templates(ui, doc, out),
                View::People => people::show(ui, doc, state, out),
            });
        }
    }
    dialogs::show(ui.ctx(), doc, state, out);
}

fn menu_bar(ui: &mut egui::Ui, doc: Option<&OpenDoc>, out: &mut Vec<Intent>) {
    egui::Panel::top("menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(text::FILE_MENU, |ui| {
                if ui.button(text::NEW_FILE).clicked() {
                    out.push(Intent::File(FileAction::New));
                }
                if ui.button(text::OPEN_FILE).clicked() {
                    out.push(Intent::File(FileAction::Open));
                }
                if ui.button(text::OPEN_SAMPLE).clicked() {
                    out.push(Intent::File(FileAction::Sample));
                }
                ui.separator();
                ui.add_enabled_ui(doc.is_some(), |ui| {
                    if ui.button(text::SAVE).clicked() {
                        out.push(Intent::File(FileAction::Save));
                    }
                    if ui.button(text::SAVE_AS).clicked() {
                        out.push(Intent::File(FileAction::SaveAs));
                    }
                    ui.separator();
                    let encrypted = doc.is_some_and(OpenDoc::encrypted);
                    if !encrypted && ui.button(text::PROTECT).clicked() {
                        out.push(Intent::Open(Dialog::Protect {
                            first: String::new(),
                            second: String::new(),
                        }));
                    }
                    if encrypted && ui.button(text::UNPROTECT).clicked() {
                        out.push(Intent::Open(Dialog::Unprotect));
                    }
                    ui.separator();
                    if ui.button(text::CLOSE_FILE).clicked() {
                        out.push(Intent::File(FileAction::Close));
                    }
                });
            });
            ui.add_enabled_ui(doc.is_some(), |ui| {
                ui.menu_button(text::EDIT_MENU, |ui| {
                    undo_redo(ui, doc, out);
                });
            });
            if doc.is_some() {
                ui.separator();
                undo_redo(ui, doc, out);
            }
        });
    });
}

fn undo_redo(ui: &mut egui::Ui, doc: Option<&OpenDoc>, out: &mut Vec<Intent>) {
    let session = doc.map(|d| &d.session);
    let can_undo = session.is_some_and(plnwrk_engine::Session::can_undo);
    let can_redo = session.is_some_and(plnwrk_engine::Session::can_redo);
    if ui
        .add_enabled(can_undo, egui::Button::new(format!("⟲ {}", text::UNDO)))
        .clicked()
    {
        out.push(Intent::run("undo", serde_json::json!({})));
    }
    if ui
        .add_enabled(can_redo, egui::Button::new(format!("⟳ {}", text::REDO)))
        .clicked()
    {
        out.push(Intent::run("redo", serde_json::json!({})));
    }
}

fn status_bar(ui: &mut egui::Ui, doc: Option<&OpenDoc>, state: &State) {
    egui::Panel::bottom("status").show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(doc) = doc {
                let place = doc
                    .path()
                    .map_or_else(|| text::UNTITLED.to_owned(), |p| p.display().to_string());
                ui.weak(place);
                if doc.encrypted() {
                    ui.weak("🔒");
                }
                if doc.is_dirty() {
                    ui.label(text::NOT_SAVED);
                }
                if doc.session.workspace().highest_classification() >= Classification::VsNfd {
                    ui.label(
                        RichText::new(text::VS_NFD)
                            .strong()
                            .color(ui.visuals().error_fg_color),
                    );
                }
            }
            if doc.is_some() {
                match &state.recovery {
                    Some(crate::autosave::Written::At(time)) => {
                        ui.weak(format!("{} {time}", text::RECOVERY_SAVED))
                            .on_hover_text(text::RECOVERED);
                    }
                    Some(crate::autosave::Written::Failed(_)) => {
                        ui.label(
                            RichText::new(text::RECOVERY_FAILED.trim_end_matches(':'))
                                .color(ui.visuals().error_fg_color),
                        );
                    }
                    None => {}
                }
            }
            if let Some(message) = &state.message {
                ui.separator();
                let color = if message.error {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().weak_text_color()
                };
                ui.label(RichText::new(&message.text).color(color));
            }
        });
    });
}

fn start(ui: &mut egui::Ui, state: &State, out: &mut Vec<Intent>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.2);
        ui.heading(RichText::new(crate::APP_NAME).size(32.0));
        ui.label(text::WELCOME);
        ui.weak(format!("Version {}", env!("CARGO_PKG_VERSION")));
        ui.add_space(16.0);
        let big = |label: &str| {
            egui::Button::new(RichText::new(label).size(16.0)).min_size(egui::vec2(220.0, 34.0))
        };
        if ui.add(big(text::NEW_FILE)).clicked() {
            out.push(Intent::File(FileAction::New));
        }
        if ui.add(big(text::OPEN_FILE)).clicked() {
            out.push(Intent::File(FileAction::Open));
        }
        if ui.add(big(text::OPEN_SAMPLE)).clicked() {
            out.push(Intent::File(FileAction::Sample));
        }
        if !state.recovered.is_empty() {
            ui.add_space(16.0);
            ui.strong(text::RECOVERED);
            for path in &state.recovered {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                if ui
                    .link(name)
                    .on_hover_text(path.display().to_string())
                    .clicked()
                {
                    out.push(Intent::File(FileAction::OpenPath(path.clone())));
                }
            }
        }
        if !state.recent.is_empty() {
            ui.add_space(16.0);
            ui.strong(text::RECENT);
            for path in &state.recent {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                if ui
                    .link(name)
                    .on_hover_text(path.display().to_string())
                    .clicked()
                {
                    out.push(Intent::File(FileAction::OpenPath(path.clone())));
                }
            }
        }
    });
}

fn sidebar(ui: &mut egui::Ui, doc: &OpenDoc, state: &State, out: &mut Vec<Intent>) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let today = now.date();
    egui::Panel::left("sidebar")
        .default_size(230.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                if ui
                    .selectable_label(state.view == View::Overview, text::OVERVIEW)
                    .clicked()
                {
                    out.push(Intent::Go(View::Overview));
                }
                ui.horizontal(|ui| {
                    if ui.button(text::NEW_PROJECT).clicked() {
                        out.push(Intent::Open(Dialog::NewProject(ProjectForm::default())));
                    }
                    if ui.button(text::FROM_TEMPLATE).clicked() {
                        out.push(Intent::Open(Dialog::FromTemplate(TemplateForm {
                            start: crate::dates::format(today),
                            ..TemplateForm::default()
                        })));
                    }
                });
                ui.separator();
                ui.strong(text::PROJECTS);
                for card in query::overview(ws, now) {
                    let label = match &card.reference {
                        Some(reference) => format!("{} · {reference}", card.title),
                        None => card.title.clone(),
                    };
                    let shown = state.view == View::Project(card.id);
                    let mut response = ui.selectable_label(shown, label);
                    if card.overdue_tasks > 0 || card.days_left.is_some_and(|d| d < 0) {
                        response = response.on_hover_text(text::OVERDUE);
                    }
                    if response.clicked() {
                        out.push(Intent::Go(View::Project(card.id)));
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(state.view == View::Templates, text::TEMPLATES)
                    .clicked()
                {
                    out.push(Intent::Go(View::Templates));
                }
                if ui
                    .selectable_label(state.view == View::People, text::PEOPLE)
                    .clicked()
                {
                    out.push(Intent::Go(View::People));
                }
            });
        });
}

/// A deadline, red when it has passed.
fn due_label(
    ui: &mut egui::Ui,
    due: Option<jiff::civil::Date>,
    time: Option<jiff::civil::Time>,
    overdue: bool,
) {
    if let Some(due) = due {
        let shown = match time {
            Some(time) => format!(
                "{} {}",
                crate::dates::format(due),
                crate::dates::format_time(time)
            ),
            None => crate::dates::format(due),
        };
        let text = RichText::new(shown).small();
        if overdue {
            ui.label(text.color(ui.visuals().error_fg_color));
        } else {
            ui.label(text.weak());
        }
    }
}

/// A short status tag for tasks that are neither open nor done.
fn status_tag(ui: &mut egui::Ui, status: TaskStatus) {
    if !matches!(status, TaskStatus::Todo | TaskStatus::Done) {
        ui.label(RichText::new(text::task_status(status)).small().weak());
    }
}

/// A small button showing only a symbol, with a spoken name for screen readers (and as
/// tooltip): "📅" is read as "Im Kalender wählen", not as "calendar emoji".
pub fn icon_button(ui: &mut egui::Ui, icon: &str, name: &str) -> egui::Response {
    let response = ui.small_button(icon).on_hover_text(name);
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    response
}

/// A text field that commits on Enter or when it loses focus; returns whether to commit.
fn commit_field(response: &egui::Response) -> bool {
    response.lost_focus()
}

/// A single-line input that commits on Enter only (and keeps the focus for the next entry).
fn enter_field(ui: &mut egui::Ui, value: &mut String, hint: &str, width: f32) -> bool {
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(width),
    );
    let entered = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if entered {
        response.request_focus();
    }
    entered && !value.trim().is_empty()
}
