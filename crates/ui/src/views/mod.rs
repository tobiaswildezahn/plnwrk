//! The views. They read the workspace and push [`Intent`]s; they never change data themselves.

mod board;
mod date;
pub(crate) mod detail;
mod dialogs;
mod overview;
mod people;
mod project;
mod ribbon;
pub(crate) mod schedule;

use egui::{Key, RichText};
use plnwrk_core::{Classification, TaskStatus};
use plnwrk_engine::query;

use crate::design::Icon;
use crate::design::widgets::Kind;
use crate::doc::OpenDoc;
use crate::state::{Dialog, FileAction, ProjectForm, State, TemplateForm, View};
use crate::{Intent, design, text};

/// The keyboard shortcuts (Cmd on the Mac, Ctrl elsewhere).
pub mod keys {
    use egui::{Key, KeyboardShortcut, Modifiers};

    const COMMAND_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);
    pub const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
    pub const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::S);
    pub const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
    pub const REDO: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::Z);
    pub const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
    pub const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
    pub const PALETTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);
    /// Collapse or expand the ribbon (as in Office).
    pub const RIBBON: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F1);
    /// The same as Office for the Mac has it (F1 is a brightness key on Mac keyboards).
    pub const RIBBON_MAC: KeyboardShortcut =
        KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::R);
}

/// Keyboard shortcuts. Undo/redo only when no text field is being edited (text fields have
/// their own undo).
pub fn shortcuts(ctx: &egui::Context, open: bool, out: &mut Vec<Intent>) {
    let typing = ctx.egui_wants_keyboard_input();
    let (save, save_as, undo, redo) = (keys::SAVE, keys::SAVE_AS, keys::UNDO, keys::REDO);
    ctx.input_mut(|input| {
        if input.consume_shortcut(&keys::QUIT) {
            out.push(Intent::File(FileAction::Quit));
            return;
        }
        if input.consume_shortcut(&keys::PALETTE) {
            out.push(Intent::Open(Dialog::Palette {
                query: String::new(),
                selected: 0,
            }));
            return;
        }
        if input.consume_shortcut(&keys::OPEN) {
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
    ribbon::show(ui, doc, state, out);
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

/// The themes with their names.
pub fn themes() -> [(egui::ThemePreference, &'static str); 3] {
    let t = text::t();
    [
        (egui::ThemePreference::System, t.theme_system),
        (egui::ThemePreference::Light, t.theme_light),
        (egui::ThemePreference::Dark, t.theme_dark),
    ]
}

fn status_bar(ui: &mut egui::Ui, doc: Option<&OpenDoc>, state: &State) {
    egui::Panel::bottom("status").show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(doc) = doc {
                let place = doc.path().map_or_else(
                    || text::t().untitled.to_owned(),
                    |p| p.display().to_string(),
                );
                ui.weak(place);
                if doc.encrypted() {
                    ui.weak("🔒");
                }
                if doc.is_dirty() {
                    ui.label(text::t().not_saved);
                }
                if doc.session.workspace().highest_classification() >= Classification::VsNfd {
                    ui.label(
                        RichText::new(text::t().vs_nfd)
                            .strong()
                            .color(ui.visuals().error_fg_color),
                    );
                }
            }
            if state.browser {
                ui.weak(text::t().browser_not_vs)
                    .on_hover_text(text::t().browser_not_vs_hint);
            }
            if doc.is_some() {
                match &state.recovery {
                    Some(crate::autosave::Written::At(time)) => {
                        ui.weak(format!("{} {time}", text::t().recovery_saved))
                            .on_hover_text(text::t().recovery_saved_hint);
                    }
                    Some(crate::autosave::Written::Failed(reason)) => {
                        ui.label(
                            RichText::new(text::t().recovery_failed.trim_end_matches(':'))
                                .color(ui.visuals().error_fg_color),
                        )
                        .on_hover_text(reason);
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
        ui.heading(RichText::new(crate::APP_NAME).size(design::tokens::size::DISPLAY));
        ui.label(text::t().welcome);
        ui.weak(format!(
            "{} {}",
            text::t().version,
            env!("CARGO_PKG_VERSION")
        ));
        if state.browser {
            ui.add_space(8.0);
            ui.label(RichText::new(text::t().browser_not_vs).strong())
                .on_hover_text(text::t().browser_not_vs_hint);
            ui.weak(text::t().browser_not_vs_hint);
        }
        ui.add_space(16.0);
        for (kind, label, action) in [
            (Kind::Primary, text::t().new_file, FileAction::New),
            (Kind::Secondary, text::t().open_file, FileAction::Open),
            (Kind::Secondary, text::t().open_sample, FileAction::Sample),
        ] {
            if design::widgets::wide_button(ui, kind, label, 220.0).clicked() {
                out.push(Intent::File(action));
            }
        }
        if !state.stored_copies.is_empty() {
            ui.add_space(16.0);
            ui.strong(text::t().recovered);
            for copy in &state.stored_copies {
                let mut label = copy.file.name.clone();
                if let Some(time) = copy.time {
                    label = format!("{label} – {}", crate::dates::format_moment(time));
                }
                if copy.downloaded {
                    label = format!("{label} ({})", text::t().copy_downloaded);
                }
                let (open, delete) = copy_row(ui, &label, text::t().browser_copy_hint);
                if open {
                    out.push(Intent::OpenStoredCopy(copy.key.clone()));
                }
                if delete {
                    out.push(Intent::Open(Dialog::DeleteStoredCopy {
                        key: copy.key.clone(),
                        name: label,
                    }));
                }
            }
            // On a shared computer: nothing of one's work stays behind.
            if state.stored_copies.len() > 1
                && ui.small_button(text::t().delete_all_copies).clicked()
            {
                out.push(Intent::Open(Dialog::DeleteAllStoredCopies));
            }
        }
        if !state.recovered.is_empty() {
            ui.add_space(16.0);
            ui.strong(text::t().recovered);
            for path in &state.recovered {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                let place = path.display().to_string();
                let (open, delete) = copy_row(ui, &name, &place);
                if open {
                    out.push(Intent::File(FileAction::OpenPath(path.clone())));
                }
                if delete {
                    out.push(Intent::Open(Dialog::DeleteCopy(path.clone())));
                }
            }
        }
        if !state.recent.is_empty() {
            ui.add_space(16.0);
            ui.strong(text::t().recent);
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

/// A recovery copy on the start page: its name to open it and a button to delete it, centred
/// like the rest of the page. Returns (open, delete).
fn copy_row(ui: &mut egui::Ui, name: &str, hover: &str) -> (bool, bool) {
    // Sized to its content, so the centred page centres the line too.
    let font = egui::TextStyle::Body.resolve(ui.style());
    let width = ui
        .painter()
        .layout_no_wrap(name.to_owned(), font, egui::Color32::PLACEHOLDER)
        .size()
        .x
        + 2.0 * ui.spacing().item_spacing.x
        + 3.0 * ui.spacing().interact_size.y;
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        let open = ui.link(name).on_hover_text(hover).clicked();
        let delete = icon_button(ui, Icon::Delete, text::t().delete_copy).clicked();
        (open, delete)
    })
    .inner
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
                    .selectable_label(state.view == View::Overview, text::t().overview)
                    .clicked()
                {
                    out.push(Intent::Go(View::Overview));
                }
                ui.horizontal(|ui| {
                    if ui.button(text::t().new_project).clicked() {
                        out.push(Intent::Open(Dialog::NewProject(ProjectForm::default())));
                    }
                    if ui.button(text::t().from_template).clicked() {
                        out.push(Intent::Open(Dialog::FromTemplate(TemplateForm {
                            start: crate::dates::format(today),
                            ..TemplateForm::default()
                        })));
                    }
                });
                ui.separator();
                ui.strong(text::t().projects);
                for card in query::overview(ws, now) {
                    let label = match &card.reference {
                        Some(reference) => format!("{} · {reference}", card.title),
                        None => card.title.clone(),
                    };
                    let shown = state.view == View::Project(card.id);
                    let mut response = ui.selectable_label(shown, label);
                    if card.overdue_tasks > 0 || card.days_left.is_some_and(|d| d < 0) {
                        response = response.on_hover_text(text::t().overdue);
                    }
                    if response.clicked() {
                        out.push(Intent::Go(View::Project(card.id)));
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(state.view == View::Templates, text::t().templates)
                    .clicked()
                {
                    out.push(Intent::Go(View::Templates));
                }
                if ui
                    .selectable_label(state.view == View::People, text::t().people)
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

/// A button showing only a symbol, with a spoken name for screen readers (and as tooltip).
pub use crate::design::widgets::icon_button;

/// A text field that commits on Enter or when it loses focus; returns whether to commit.
fn commit_field(response: &egui::Response) -> bool {
    response.lost_focus()
}

/// A single-line input that commits on Enter only (and keeps the focus for the next entry).
/// `focus`: it takes the cursor now.
fn enter_field(ui: &mut egui::Ui, value: &mut String, hint: &str, width: f32, focus: bool) -> bool {
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(width),
    );
    if focus {
        response.request_focus();
        response.scroll_to_me(None);
    }
    let entered = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if entered {
        response.request_focus();
    }
    entered && !value.trim().is_empty()
}
