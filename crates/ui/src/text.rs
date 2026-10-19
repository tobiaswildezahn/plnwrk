//! Every text of the user interface, in one place, in German and English.
//!
//! Views read fixed texts through [`t()`] and texts with values through the functions below.
//! The language is kept per thread and set by the app at the start of every frame, so tests
//! running side by side do not disturb each other.

use std::cell::Cell;

use plnwrk_core::{Priority, ProjectStatus, Rule, TaskStatus};
use plnwrk_engine::CommandError;
use plnwrk_engine::query::MilestoneState;
use plnwrk_store::StoreError;

/// A language of the interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    German,
    English,
}

impl Language {
    /// Every language, in the order offered.
    pub const ALL: [Self; 2] = [Self::German, Self::English];

    /// The stored code: `de`, `en`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::German => "de",
            Self::English => "en",
        }
    }

    /// The language with this stored code.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }

    /// Its own name, the same in every language (so it can always be found).
    pub const fn name(self) -> &'static str {
        match self {
            Self::German => "Deutsch",
            Self::English => "English",
        }
    }
}

thread_local! {
    static LANGUAGE: Cell<Language> = const { Cell::new(Language::German) };
}

/// Uses this language for the texts from now on (on this thread).
pub fn set_language(language: Language) {
    LANGUAGE.with(|l| l.set(language));
}

/// The language in use.
pub fn language() -> Language {
    LANGUAGE.with(Cell::get)
}

/// The fixed texts in the language in use.
pub fn t() -> &'static Texts {
    match language() {
        Language::German => &DE,
        Language::English => &EN,
    }
}

/// German or English, by the language in use.
fn pick<T>(german: T, english: T) -> T {
    match language() {
        Language::German => german,
        Language::English => english,
    }
}

/// Every fixed text of the interface, once per language. A missing translation does not build.
#[derive(Debug)]
pub struct Texts {
    pub untitled: &'static str,
    pub shared_file_replaced: &'static str,
    pub who_am_i_title: &'static str,
    pub who_am_i_question: &'static str,
    pub only_me: &'static str,
    pub shared_badge: &'static str,
    pub change_me: &'static str,
    pub held_elsewhere: &'static str,
    pub me_marker: &'static str,
    pub edit_person: &'static str,
    pub welcome: &'static str,
    pub new_file: &'static str,
    pub open_file: &'static str,
    pub open_sample: &'static str,
    pub save: &'static str,
    pub save_as: &'static str,
    pub close_file: &'static str,
    pub recent: &'static str,
    pub recovered: &'static str,
    pub file_menu: &'static str,
    pub undo: &'static str,
    pub redo: &'static str,
    pub overview: &'static str,
    pub projects: &'static str,
    pub templates: &'static str,
    pub people: &'static str,
    pub new_project: &'static str,
    pub from_template: &'static str,
    pub nothing_yet: &'static str,
    pub overdue: &'static str,
    pub list: &'static str,
    pub board: &'static str,
    pub milestones: &'static str,
    pub edit: &'static str,
    pub archive: &'static str,
    pub apply: &'static str,
    pub cancel: &'static str,
    pub create: &'static str,
    pub delete: &'static str,
    pub title: &'static str,
    pub kind: &'static str,
    pub reference: &'static str,
    pub start: &'static str,
    pub due: &'static str,
    pub owner: &'static str,
    pub status: &'static str,
    pub priority: &'static str,
    pub assignee: &'static str,
    pub progress: &'static str,
    pub tags: &'static str,
    pub description: &'static str,
    pub template: &'static str,
    pub nobody: &'static str,
    pub new_task_hint: &'static str,
    pub new_subtask_hint: &'static str,
    pub add_subtask: &'static str,
    pub close_details: &'static str,
    pub rename_hint: &'static str,
    pub no_tasks: &'static str,
    pub new_milestone: &'static str,
    pub subtasks: &'static str,
    pub comments: &'static str,
    pub new_comment_hint: &'static str,
    pub add_comment: &'static str,
    pub history: &'static str,
    /// Marks a change in the history made by an AI (over MCP).
    pub by_ai: &'static str,
    pub delete_task: &'static str,
    pub delete_task_question: &'static str,
    pub confirm_milestone_title: &'static str,
    pub confirm_milestone_hint: &'static str,
    pub change_button: &'static str,
    pub milestone_date: &'static str,
    pub milestone_deleted: &'static str,
    pub milestone_unmarked: &'static str,
    pub milestone_marked: &'static str,
    pub after_project_due_then: &'static str,
    pub first_case: &'static str,
    pub person_pending: &'static str,
    pub first_milestone: &'static str,
    pub milestones_empty: &'static str,
    pub people_empty: &'static str,
    pub dismiss_message: &'static str,
    pub group_assignment: &'static str,
    pub group_dates: &'static str,
    pub group_notes: &'static str,
    pub not_itself: &'static str,
    pub waits_already: &'static str,
    pub would_circle: &'static str,
    pub cases_heading: &'static str,
    pub projects_heading: &'static str,
    pub today_and_overdue: &'static str,
    pub overtaken_milestones: &'static str,
    pub nothing_due: &'static str,
    pub new_file_explained: &'static str,
    pub sample_explained: &'static str,
    pub case_explained: &'static str,
    pub project_explained: &'static str,
    pub add_person: &'static str,
    pub title_needed: &'static str,
    pub board_filter_everyone: &'static str,
    pub board_filter_overdue: &'static str,
    pub board_search: &'static str,
    pub board_search_label: &'static str,
    pub board_filter_reset: &'static str,
    pub light_reached: &'static str,
    pub light_overdue: &'static str,
    pub light_at_risk: &'static str,
    pub light_on_plan: &'static str,
    pub reach_button: &'static str,
    pub reopen_button: &'static str,
    pub change_date: &'static str,
    pub project_due_mark: &'static str,
    pub predecessor_late: &'static str,
    pub no_assignee: &'static str,
    pub timeline_hint: &'static str,
    pub close_subtasks_title: &'static str,
    pub whole_project: &'static str,
    pub float_hint: &'static str,
    pub overdue_hint: &'static str,
    pub due_line_hint: &'static str,
    pub drag_hint: &'static str,
    pub schedule_empty: &'static str,
    pub give_dates: &'static str,
    pub first_task: &'static str,
    pub link_handle: &'static str,
    pub add_predecessor_menu: &'static str,
    pub add_successor_menu: &'static str,
    pub show: &'static str,
    pub critical_path: &'static str,
    pub overdue_short: &'static str,
    pub critical_short: &'static str,
    pub fold_subtasks: &'static str,
    pub unfold_subtasks: &'static str,
    pub title_width: &'static str,
    pub zoom_levels: [&'static str; 4],
    /// "ja" in tables (critical, milestone).
    pub yes: &'static str,
    pub export_schedule: &'static str,
    pub exported: &'static str,
    pub export_failed: &'static str,
    pub export_over_open_file: &'static str,
    pub close_all_subtasks: &'static str,
    pub only_this_task: &'static str,
    pub name: &'static str,
    pub role: &'static str,
    pub initials: &'static str,
    pub email: &'static str,
    pub new_person: &'static str,
    pub i_am: &'static str,
    pub passphrase: &'static str,
    pub passphrase_again: &'static str,
    pub protect: &'static str,
    pub protect_button: &'static str,
    pub protect_hint: &'static str,
    pub unprotect: &'static str,
    pub unprotect_button: &'static str,
    pub unprotect_question: &'static str,
    pub protected: &'static str,
    pub unprotected: &'static str,
    pub protection_unchanged: &'static str,
    pub passphrases_differ: &'static str,
    pub passphrase_too_short: &'static str,
    pub passphrase_question: &'static str,
    pub unsaved_title: &'static str,
    pub unsaved_question: &'static str,
    pub discard: &'static str,
    pub saved: &'static str,
    pub already_open: &'static str,
    pub recovery_failed: &'static str,
    pub recovery_saved: &'static str,
    pub recovery_saved_hint: &'static str,
    pub recovery_writer_stopped: &'static str,
    pub delete_copy: &'static str,
    pub delete_copy_question: &'static str,
    pub previous_month: &'static str,
    pub next_month: &'static str,
    pub recovered_opened: &'static str,
    pub not_saved: &'static str,
    pub vs_nfd: &'static str,
    pub pick_date: &'static str,
    pub today: &'static str,
    pub clear: &'static str,
    pub time_hint: &'static str,
    pub add_time: &'static str,
    pub milestone: &'static str,
    pub milestone_hint: &'static str,
    pub upgraded: &'static str,
    pub part_of: &'static str,
    pub new_project_title: &'static str,
    pub from_template_title: &'static str,
    pub from_template_hint: &'static str,
    pub please_choose: &'static str,
    pub no_template: &'static str,
    pub open: &'static str,
    pub version: &'static str,
    pub i_am_hint: &'static str,
    pub templates_hint: &'static str,
    pub no_templates: &'static str,
    pub not_taken: &'static str,
    pub kind_hint: &'static str,
    pub use_as_template: &'static str,
    pub date_needed: &'static str,
    pub choose_template: &'static str,
    pub start_needed: &'static str,
    pub invalid_dates: &'static str,
    pub time_needs_date: &'static str,
    pub created: &'static str,
    pub removed: &'static str,
    pub changed: &'static str,
    #[cfg_attr(
        not(target_arch = "wasm32"),
        expect(dead_code, reason = "only the browser has no files yet")
    )]
    pub browser_files_later: &'static str,
    pub o_clock: &'static str,
    pub view_menu: &'static str,
    pub language_menu: &'static str,
    pub appearance: &'static str,
    pub theme_system: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub date_hint: &'static str,
    pub palette: &'static str,
    pub palette_hint: &'static str,
    pub palette_empty: &'static str,
    pub go_to: &'static str,
    pub quit: &'static str,
    pub new_task: &'static str,
    pub edit_project: &'static str,
    pub archive_project: &'static str,
    pub upgrade_failed: &'static str,
    pub no_file_chosen: &'static str,
    pub not_a_file_name: &'static str,
    pub more_results: &'static str,
    pub downloaded: &'static str,
    pub download_failed: &'static str,
    pub vs_blocked: &'static str,
    pub browser_not_vs: &'static str,
    pub browser_not_vs_hint: &'static str,
    pub no_copy_encrypted: &'static str,
    pub browser_copy_hint: &'static str,
    pub copy_downloaded: &'static str,
    pub upload_failed: &'static str,
    pub delete_all_copies: &'static str,
    pub delete_all_copies_question: &'static str,
    pub waits_for: &'static str,
    pub followed_by: &'static str,
    pub add_predecessor: &'static str,
    pub remove_dependency: &'static str,
    pub schedule_tab: &'static str,
    pub unscheduled: &'static str,
    pub critical_hint: &'static str,
    pub project_due: &'static str,
    pub critical: &'static str,
    /// Suggestions for the kind of a project.
    pub kinds: [&'static str; 2],
    /// Names of the months, January first.
    pub months: [&'static str; 12],
    /// Short names of the weekdays, Monday first.
    pub weekdays: [&'static str; 7],
    /// Tabs, groups and buttons of the ribbon.
    pub ribbon: RibbonTexts,
}

/// The texts of the ribbon (Phase 9b).
#[derive(Debug)]
pub struct RibbonTexts {
    pub start: &'static str,
    pub task: &'static str,
    pub new: &'static str,
    pub protection: &'static str,
    pub close: &'static str,
    pub undo: &'static str,
    pub find: &'static str,
    pub views: &'static str,
    pub areas: &'static str,
    pub zoom: &'static str,
    pub share: &'static str,
    pub new_case: &'static str,
    /// The kind a new case gets (the form shows it and it can be changed).
    pub case_kind: &'static str,
    pub new_project: &'static str,
    pub from_template: &'static str,
    pub find_action: &'static str,
    pub collapse: &'static str,
    pub expand: &'static str,
    pub scroll_left: &'static str,
    pub scroll_right: &'static str,
    pub new_case_long: &'static str,
    pub schedule_tools: &'static str,
    pub navigate: &'static str,
    pub edit: &'static str,
    pub new_project_long: &'static str,
}

/// German.
pub static DE: Texts = Texts {
    untitled: "Unbenannt",
    shared_file_replaced: "Die gemeinsame Datei wurde durch eine andere ersetzt. Bitte mit „Speichern unter“ an einem anderen Ort sichern.",
    who_am_i_title: "Wer bist du?",
    who_am_i_question: "In dieser Datei arbeiten mehrere Personen. Als wer arbeitest du hier? plnwrk merkt es sich auf diesem Gerät, speichert dann jede Änderung von selbst und zeigt die der anderen.",
    only_me: "Nur ich nutze diese Datei",
    shared_badge: "Gemeinsam",
    change_me: "Person wechseln",
    held_elsewhere: "Die Datei ist in einer älteren plnwrk-Version geöffnet. Speichern geht erst, wenn sie dort geschlossen ist.",
    me_marker: "(ich)",
    edit_person: "Person bearbeiten",
    welcome: "Projekte und Vorgänge – in einer Datei, die dir gehört.",
    new_file: "Neue Datei",
    open_file: "Datei öffnen…",
    open_sample: "Beispiel öffnen",
    save: "Speichern",
    save_as: "Speichern unter…",
    close_file: "Datei schließen",
    recent: "Zuletzt geöffnet",
    recovered: "Wiederherstellen (automatisch gesichert, nicht gespeichert)",
    file_menu: "Datei",
    undo: "Rückgängig",
    redo: "Wiederholen",
    overview: "Übersicht",
    projects: "Projekte und Vorgänge",
    templates: "Vorlagen",
    people: "Personen",
    new_project: "+ Neu",
    from_template: "+ Aus Vorlage",
    nothing_yet: "Noch kein Vorgang und kein Projekt – leg den ersten an.",
    overdue: "Überfällig",
    list: "Liste",
    board: "Board",
    milestones: "Meilensteine",
    edit: "Bearbeiten",
    archive: "Archivieren",
    apply: "Übernehmen",
    cancel: "Abbrechen",
    create: "Anlegen",
    delete: "Löschen",
    title: "Titel",
    kind: "Art",
    reference: "Aktenzeichen",
    start: "Beginn",
    due: "Frist",
    owner: "Verantwortlich",
    status: "Status",
    priority: "Priorität",
    assignee: "Zuständig",
    progress: "Fortschritt",
    tags: "Schlagworte",
    description: "Beschreibung",
    template: "Vorlage",
    nobody: "–",
    new_task_hint: "Neue Aufgabe … (Enter)",
    new_subtask_hint: "Neue Unteraufgabe … (Enter)",
    add_subtask: "Unteraufgabe hinzufügen",
    close_details: "Details schließen",
    rename_hint: "Doppelklick: umbenennen",
    no_tasks: "Noch keine Aufgaben hier.",
    new_milestone: "+ Meilenstein",
    subtasks: "Unteraufgaben",
    comments: "Kommentare",
    new_comment_hint: "Kommentar schreiben …",
    add_comment: "Kommentieren",
    history: "Verlauf",
    by_ai: "durch KI",
    delete_task: "Aufgabe löschen",
    delete_task_question: "Die Aufgabe wird mit allen Unteraufgaben und Kommentaren gelöscht (rückgängig machbar).",
    confirm_milestone_title: "Meilenstein ändern?",
    confirm_milestone_hint: "Meilensteine sind feste Termine. Jede Änderung wird bestätigt.",
    change_button: "Ändern",
    milestone_date: "Termin",
    milestone_deleted: "wird gelöscht",
    milestone_unmarked: "ist danach kein Meilenstein mehr",
    milestone_marked: "wird ein Meilenstein",
    after_project_due_then: "Er liegt dann nach der Frist des Projekts.",
    first_case: "Vorgang anlegen",
    person_pending: "Erst die neue Person anlegen (oder ihr Feld leeren).",
    first_milestone: "Ersten Meilenstein anlegen",
    milestones_empty: "Noch keine Meilensteine – feste Termine wie eine Abgabe oder eine Abnahme.",
    people_empty: "Noch niemand angelegt. Personen sind zuständig für Aufgaben und Projekte.",
    dismiss_message: "Meldung schließen",
    group_assignment: "Zuordnung",
    group_dates: "Termine",
    group_notes: "Notizen & Verlauf",
    not_itself: "Eine Aufgabe wartet nicht auf sich selbst.",
    waits_already: "Wartet schon darauf.",
    would_circle: "Ergäbe einen Kreis: Diese Aufgabe wartet (über andere) schon auf die hier.",
    cases_heading: "Vorgänge",
    projects_heading: "Projekte",
    today_and_overdue: "Heute und überfällig",
    overtaken_milestones: "Überholte Meilensteine",
    nothing_due: "Heute ist nichts fällig, und nichts ist überfällig.",
    new_file_explained: "Eine leere Datei für eigene Vorgänge und Projekte. Sie gehört dir und bleibt auf diesem Gerät.",
    sample_explained: "Ein Projekt und ein Vorgang mit Aufgaben, Fristen und Meilensteinen – zum Ausprobieren, nichts geht kaputt.",
    case_explained: "Ein Auftrag mit Frist, oft mit Aktenzeichen – z. B. eine Schriftliche Kleine Anfrage.",
    project_explained: "Ein Vorhaben mit vielen Schritten und Meilensteinen – z. B. die Umrüstung der Wachen auf Digitalfunk.",
    add_person: "Person anlegen…",
    title_needed: "Bitte einen Titel eingeben.",
    board_filter_everyone: "Alle",
    board_filter_overdue: "Nur Überfälliges",
    board_search: "Im Titel suchen",
    board_search_label: "Suche",
    board_filter_reset: "Filter aufheben",
    light_reached: "erreicht",
    light_overdue: "überfällig",
    light_at_risk: "gefährdet",
    light_on_plan: "im Plan",
    reach_button: "Erreicht",
    reopen_button: "Wieder öffnen",
    change_date: "Termin ändern",
    project_due_mark: "Projektfrist",
    predecessor_late: "Ein Vorgänger ist überfällig.",
    no_assignee: "niemand",
    timeline_hint: "Raute anklicken wählt aus, ziehen ändert den Termin (mit Rückfrage).",
    close_subtasks_title: "Unteraufgaben auch erledigen?",
    whole_project: "Ganzes Projekt",
    float_hint: "So viele Tage kann sich die Aufgabe verschieben, ohne dass sich das Ende verschiebt.",
    overdue_hint: "Rot umrandet mit ⚠: überfällig.",
    due_line_hint: "Rote Linie: Frist des Projekts.",
    drag_hint: "Balken ziehen verschiebt, an den Enden ziehen ändert Beginn oder Frist; was wartet, rutscht mit. Der Punkt am Ende verbindet.",
    schedule_empty: "Noch keine Aufgabe mit Termin. Der Zeitplan zeigt Aufgaben, sobald sie Beginn oder Frist haben.",
    give_dates: "Termine vergeben",
    first_task: "Erste Aufgabe anlegen",
    link_handle: "Verbinden",
    add_predecessor_menu: "Vorgänger hinzufügen…",
    add_successor_menu: "Nachfolger hinzufügen…",
    show: "Anzeigen",
    critical_path: "Kritischer Pfad",
    overdue_short: "überfällig",
    critical_short: "auf dem kritischen Pfad",
    fold_subtasks: "Unteraufgaben zuklappen",
    unfold_subtasks: "Unteraufgaben aufklappen",
    title_width: "Breite der Titelspalte",
    zoom_levels: ["Quartal", "Monat", "Woche", "Tag"],
    yes: "ja",
    export_schedule: "Als Tabelle exportieren (Excel)",
    exported: "Exportiert:",
    export_failed: "Export fehlgeschlagen:",
    export_over_open_file: "Nicht exportiert: Das ist die geöffnete plnwrk-Datei. Bitte einen anderen Namen wählen.",
    close_all_subtasks: "Alle erledigen",
    only_this_task: "Nur die Hauptaufgabe",
    name: "Name",
    role: "Funktion",
    initials: "Kürzel",
    email: "E-Mail",
    new_person: "+ Person",
    i_am: "Ich arbeite als",
    passphrase: "Passphrase",
    passphrase_again: "Passphrase wiederholen",
    protect: "Mit Passphrase schützen…",
    protect_button: "Schützen",
    protect_hint: "Mindestens 12 Zeichen. Ohne Passphrase ist die Datei nicht mehr zu öffnen – sie lässt sich nicht zurücksetzen.",
    unprotect: "Schutz entfernen…",
    unprotect_button: "Schutz entfernen",
    unprotect_question: "Die Datei wird danach unverschlüsselt gespeichert.",
    protected: "Die Datei ist jetzt mit Passphrase geschützt.",
    unprotected: "Die Datei ist jetzt unverschlüsselt.",
    protection_unchanged: "Schutz nicht geändert.",
    passphrases_differ: "Die beiden Eingaben stimmen nicht überein.",
    passphrase_too_short: "Die Passphrase braucht mindestens 12 Zeichen.",
    passphrase_question: "Die Datei ist verschlüsselt. Passphrase:",
    unsaved_title: "Ungespeicherte Änderungen",
    unsaved_question: "Die Änderungen sind noch nicht gespeichert.",
    discard: "Verwerfen",
    saved: "Gespeichert.",
    already_open: "Diese Datei ist bereits geöffnet.",
    recovery_failed: "Wiederherstellungskopie nicht möglich:",
    recovery_saved: "Wiederherstellungskopie",
    recovery_saved_hint: "Automatisch gesichert. Nach einem Absturz steht sie beim nächsten Start auf der Startseite.",
    recovery_writer_stopped: "Das Schreiben wurde unterbrochen.",
    delete_copy: "Kopie löschen",
    delete_copy_question: "Die Wiederherstellungskopie wird endgültig gelöscht. Ihr Inhalt ist dann verloren.",
    previous_month: "Vormonat",
    next_month: "Folgemonat",
    recovered_opened: "Wiederhergestellt. Bitte mit „Speichern unter…“ an ihren Platz speichern.",
    not_saved: "• ungespeichert",
    vs_nfd: "VS – NUR FÜR DEN DIENSTGEBRAUCH",
    pick_date: "Im Kalender wählen",
    today: "Heute",
    clear: "Leeren",
    time_hint: "HH:MM",
    add_time: "Uhrzeit hinzufügen",
    milestone: "Meilenstein",
    milestone_hint: "Ein Datum, das zählt: erreicht, wenn erledigt. Nur oberste Ebene, ohne Unteraufgaben.",
    upgraded: "Datei auf das neue Format umgestellt: Meilensteine sind jetzt Aufgaben. Bitte speichern.",
    part_of: "Teil von:",
    new_project_title: "Neues Projekt oder neuer Vorgang",
    from_template_title: "Aus Vorlage anlegen",
    from_template_hint: "Aufgaben und Meilensteine der Vorlage werden übernommen und auf den Beginn verschoben.",
    please_choose: "Bitte wählen",
    no_template: "Es gibt noch keine Vorlage.",
    open: "Öffnen",
    version: "Version",
    i_am_hint: "Änderungen und Kommentare werden mit dieser Person vermerkt.",
    templates_hint: "Eine Vorlage ist ein Projekt mit festem Ablauf. Daraus entstehen neue Vorgänge.",
    no_templates: "Noch keine Vorlagen. Lege ein Projekt an und markiere es als Vorlage.",
    not_taken: "Nicht übernommen:",
    kind_hint: "Projekt, SKA, Beschaffung …",
    use_as_template: "Als Vorlage verwenden",
    date_needed: "Bitte ein Datum angeben.",
    choose_template: "Bitte eine Vorlage wählen.",
    start_needed: "Bitte den Beginn angeben.",
    invalid_dates: "Kein gültiges Datum oder keine gültige Uhrzeit, nicht übernommen:",
    time_needs_date: "Eine Uhrzeit braucht ein Datum.",
    created: "angelegt",
    removed: "gelöscht",
    changed: "geändert:",
    browser_files_later: "Dateien öffnen und speichern folgt im Browser in Phase 4",
    o_clock: " Uhr",
    view_menu: "Ansicht",
    language_menu: "Sprache",
    appearance: "Darstellung",
    theme_system: "Wie das System",
    theme_light: "Hell",
    theme_dark: "Dunkel",
    date_hint: "TT.MM.JJJJ",
    palette: "Befehlspalette…",
    palette_hint: "Befehl oder Name suchen …",
    palette_empty: "Nichts gefunden.",
    go_to: "Gehe zu",
    quit: "Beenden",
    new_task: "Neue Aufgabe",
    edit_project: "Projekt bearbeiten",
    archive_project: "Projekt archivieren",
    upgrade_failed: "Umstellung auf das neue Format fehlgeschlagen:",
    no_file_chosen: "Noch keine Datei gewählt.",
    not_a_file_name: "Das ist kein Dateiname:",
    more_results: "… weitere Treffer: Suche genauer eingeben.",
    downloaded: "Heruntergeladen – die Datei liegt im Download-Ordner:",
    download_failed: "Herunterladen nicht möglich:",
    vs_blocked: "VS-NfD ist im Browser gesperrt. Bitte die Desktop-App verwenden.",
    browser_not_vs: "Browser: nicht für VS-NfD freigegeben",
    browser_not_vs_hint: "Der Browser ist keine für Verschlusssachen geeignete Umgebung. VS-NfD-Dateien öffnen nur in der Desktop-App.",
    no_copy_encrypted: "Kein Zwischenstand im Browser: die Datei ist verschlüsselt.",
    browser_copy_hint: "Zwischenstand im Browser-Speicher. Nach einem Neuladen steht er auf der Startseite.",
    copy_downloaded: "heruntergeladen",
    upload_failed: "Die Datei ließ sich nicht lesen:",
    delete_all_copies: "Alle Zwischenstände löschen",
    delete_all_copies_question: "Alle hier angebotenen Zwischenstände werden endgültig aus diesem Browser gelöscht. Ihr Inhalt ist dann verloren.",
    waits_for: "Wartet auf",
    followed_by: "Danach",
    add_predecessor: "+ Vorgänger …",
    remove_dependency: "Abhängigkeit entfernen",
    schedule_tab: "Zeitplan",
    unscheduled: "Ohne Termin:",
    critical_hint: "Rot: kritischer Pfad – rutscht eine dieser Aufgaben, rutscht das Ende.",
    project_due: "Projektfrist",
    critical: "kritisch",
    kinds: ["Projekt", "Vorgang"],
    months: [
        "Januar",
        "Februar",
        "März",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
    ],
    weekdays: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
    ribbon: RibbonTexts {
        start: "Start",
        task: "Aufgabe",
        new: "Neu",
        protection: "Schutz",
        close: "Schließen",
        undo: "Rückgängig",
        find: "Suchen",
        views: "Ansichten",
        areas: "Bereiche",
        zoom: "Zoom",
        share: "Weitergeben",
        new_case: "Vorgang",
        case_kind: "Vorgang",
        new_project: "Projekt",
        from_template: "Aus Vorlage",
        find_action: "Befehl suchen",
        collapse: "Menüband einklappen",
        expand: "Menüband ausklappen",
        scroll_left: "Menüband nach links",
        scroll_right: "Menüband nach rechts",
        new_case_long: "Neuer Vorgang",
        schedule_tools: "Zeitplan-Werkzeuge",
        navigate: "Gehe zu",
        edit: "Ändern",
        new_project_long: "Neues Projekt",
    },
};

/// English.
pub static EN: Texts = Texts {
    untitled: "Untitled",
    shared_file_replaced: "The shared file was replaced by another one. Please keep your work with “Save as” in another place.",
    who_am_i_title: "Who are you?",
    who_am_i_question: "Several people work in this file. Who are you here? plnwrk remembers it on this device, then saves every change by itself and shows the others' changes.",
    only_me: "Only I use this file",
    shared_badge: "Shared",
    change_me: "Change person",
    held_elsewhere: "The file is open in an older plnwrk. Saving works once it is closed there.",
    me_marker: "(me)",
    edit_person: "Edit person",
    welcome: "Projects and cases – in one file that belongs to you.",
    new_file: "New file",
    open_file: "Open file…",
    open_sample: "Open sample",
    save: "Save",
    save_as: "Save as…",
    close_file: "Close file",
    recent: "Recently opened",
    recovered: "Recover (saved automatically, not saved by you)",
    file_menu: "File",
    undo: "Undo",
    redo: "Redo",
    overview: "Overview",
    projects: "Projects and cases",
    templates: "Templates",
    people: "People",
    new_project: "+ New",
    from_template: "+ From template",
    nothing_yet: "No case and no project yet – create the first.",
    overdue: "Overdue",
    list: "List",
    board: "Board",
    milestones: "Milestones",
    edit: "Edit",
    archive: "Archive",
    apply: "Apply",
    cancel: "Cancel",
    create: "Create",
    delete: "Delete",
    title: "Title",
    kind: "Type",
    reference: "Reference",
    start: "Start",
    due: "Deadline",
    owner: "Responsible",
    status: "Status",
    priority: "Priority",
    assignee: "Assignee",
    progress: "Progress",
    tags: "Tags",
    description: "Description",
    template: "Template",
    nobody: "–",
    new_task_hint: "New task … (Enter)",
    new_subtask_hint: "New subtask … (Enter)",
    add_subtask: "Add subtask",
    close_details: "Close details",
    rename_hint: "Double-click: rename",
    no_tasks: "No tasks yet.",
    new_milestone: "+ Milestone",
    subtasks: "Subtasks",
    comments: "Comments",
    new_comment_hint: "Write a comment …",
    add_comment: "Comment",
    history: "History",
    by_ai: "by AI",
    delete_task: "Delete task",
    delete_task_question: "The task is deleted with all its subtasks and comments (can be undone).",
    confirm_milestone_title: "Change a milestone?",
    confirm_milestone_hint: "Milestones are fixed dates. Every change is confirmed.",
    change_button: "Change",
    milestone_date: "Date",
    milestone_deleted: "is deleted",
    milestone_unmarked: "is no longer a milestone afterwards",
    milestone_marked: "becomes a milestone",
    after_project_due_then: "It then lies after the project's deadline.",
    first_case: "Create a case",
    person_pending: "Add the new person first (or empty its field).",
    first_milestone: "Create the first milestone",
    milestones_empty: "No milestones yet – fixed dates such as a submission or an acceptance.",
    people_empty: "Nobody yet. People are in charge of tasks and projects.",
    dismiss_message: "Close the message",
    group_assignment: "Assignment",
    group_dates: "Dates",
    group_notes: "Notes & history",
    not_itself: "A task does not wait for itself.",
    waits_already: "Waits for it already.",
    would_circle: "Would make a circle: it already waits (through others) for this one.",
    cases_heading: "Cases",
    projects_heading: "Projects",
    today_and_overdue: "Today and overdue",
    overtaken_milestones: "Overtaken milestones",
    nothing_due: "Nothing is due today, and nothing is overdue.",
    new_file_explained: "An empty file for your own cases and projects. It is yours and stays on this device.",
    sample_explained: "A project and a case with tasks, deadlines and milestones – to try things out, nothing breaks.",
    case_explained: "A request with a deadline, often with a reference – e.g. a written question from parliament.",
    project_explained: "An undertaking with many steps and milestones – e.g. moving the stations to digital radio.",
    add_person: "Add a person…",
    title_needed: "Please enter a title.",
    board_filter_everyone: "Everyone",
    board_filter_overdue: "Only overdue",
    board_search: "Search titles",
    board_search_label: "Search",
    board_filter_reset: "Clear filter",
    light_reached: "reached",
    light_overdue: "overdue",
    light_at_risk: "at risk",
    light_on_plan: "on track",
    reach_button: "Reached",
    reopen_button: "Reopen",
    change_date: "Change date",
    project_due_mark: "Project deadline",
    predecessor_late: "A task it waits for is overdue.",
    no_assignee: "nobody",
    timeline_hint: "Click a diamond to choose it, drag it to change the date (asked first).",
    close_subtasks_title: "Close the subtasks too?",
    whole_project: "Whole project",
    float_hint: "By this many days the task can slip without the end slipping.",
    overdue_hint: "Framed red with ⚠: overdue.",
    due_line_hint: "Red line: the project's deadline.",
    drag_hint: "Drag a bar to move it, its ends to change start or deadline; what waits moves along. The dot at the end links.",
    schedule_empty: "No task with dates yet. The schedule shows tasks once they have a start or deadline.",
    give_dates: "Give dates",
    first_task: "Create the first task",
    link_handle: "Link",
    add_predecessor_menu: "Add predecessor…",
    add_successor_menu: "Add successor…",
    show: "Show",
    critical_path: "Critical path",
    overdue_short: "overdue",
    critical_short: "on the critical path",
    fold_subtasks: "Fold subtasks",
    unfold_subtasks: "Unfold subtasks",
    title_width: "Width of the title column",
    zoom_levels: ["Quarter", "Month", "Week", "Day"],
    yes: "yes",
    export_schedule: "Export as table (Excel)",
    exported: "Exported:",
    export_failed: "Export failed:",
    export_over_open_file: "Not exported: that is the open plnwrk file. Please choose another name.",
    close_all_subtasks: "Close them all",
    only_this_task: "Only this task",
    name: "Name",
    role: "Role",
    initials: "Initials",
    email: "Email",
    new_person: "+ Person",
    i_am: "Working as",
    passphrase: "Passphrase",
    passphrase_again: "Repeat passphrase",
    protect: "Protect with passphrase…",
    protect_button: "Protect",
    protect_hint: "At least 12 characters. Without the passphrase the file can no longer be opened – it cannot be reset.",
    unprotect: "Remove protection…",
    unprotect_button: "Remove protection",
    unprotect_question: "The file will then be saved unencrypted.",
    protected: "The file is now protected with a passphrase.",
    unprotected: "The file is now unencrypted.",
    protection_unchanged: "Protection not changed.",
    passphrases_differ: "The two entries do not match.",
    passphrase_too_short: "The passphrase needs at least 12 characters.",
    passphrase_question: "The file is encrypted. Passphrase:",
    unsaved_title: "Unsaved changes",
    unsaved_question: "The changes have not been saved yet.",
    discard: "Discard",
    saved: "Saved.",
    already_open: "This file is already open.",
    recovery_failed: "Recovery copy not possible:",
    recovery_saved: "Recovery copy",
    recovery_saved_hint: "Saved automatically. After a crash it is offered on the start page next time.",
    recovery_writer_stopped: "Writing was interrupted.",
    delete_copy: "Delete copy",
    delete_copy_question: "The recovery copy is deleted for good. Its content is then lost.",
    previous_month: "Previous month",
    next_month: "Next month",
    recovered_opened: "Recovered. Please save it in its place with “Save as…”.",
    not_saved: "• unsaved",
    vs_nfd: "VS – NUR FÜR DEN DIENSTGEBRAUCH",
    pick_date: "Pick in calendar",
    today: "Today",
    clear: "Clear",
    time_hint: "HH:MM",
    add_time: "Add time",
    milestone: "Milestone",
    milestone_hint: "A date that matters: reached when done. Top level only, without subtasks.",
    upgraded: "File converted to the new format: milestones are now tasks. Please save.",
    part_of: "Part of:",
    new_project_title: "New project or case",
    from_template_title: "Create from template",
    from_template_hint: "The tasks and milestones of the template are copied and moved to the start.",
    please_choose: "Please choose",
    no_template: "There is no template yet.",
    open: "Open",
    version: "Version",
    i_am_hint: "Changes and comments are recorded with this person.",
    templates_hint: "A template is a project with a fixed sequence. New cases are created from it.",
    no_templates: "No templates yet. Create a project and mark it as a template.",
    not_taken: "Not taken:",
    kind_hint: "Project, inquiry, procurement …",
    use_as_template: "Use as template",
    date_needed: "Please enter a date.",
    choose_template: "Please choose a template.",
    start_needed: "Please enter the start.",
    invalid_dates: "Not a valid date or time, not taken:",
    time_needs_date: "A time needs a date.",
    created: "created",
    removed: "deleted",
    changed: "changed:",
    browser_files_later: "Opening and saving files in the browser follows in Phase 4",
    o_clock: "",
    view_menu: "View",
    language_menu: "Language",
    appearance: "Appearance",
    theme_system: "Like the system",
    theme_light: "Light",
    theme_dark: "Dark",
    date_hint: "YYYY-MM-DD",
    palette: "Command palette…",
    palette_hint: "Search a command or name …",
    palette_empty: "Nothing found.",
    go_to: "Go to",
    quit: "Quit",
    new_task: "New task",
    edit_project: "Edit project",
    archive_project: "Archive project",
    upgrade_failed: "Converting to the new format failed:",
    no_file_chosen: "No file chosen yet.",
    not_a_file_name: "This is not a file name:",
    more_results: "… more results: narrow the search.",
    downloaded: "Downloaded – the file is in the downloads folder:",
    download_failed: "Download not possible:",
    vs_blocked: "VS-NfD is blocked in the browser. Please use the desktop app.",
    browser_not_vs: "Browser: not cleared for VS-NfD",
    browser_not_vs_hint: "The browser is not an environment suited to classified material. VS-NfD files open only in the desktop app.",
    no_copy_encrypted: "No interim copy in the browser: the file is encrypted.",
    browser_copy_hint: "Interim copy in the browser's storage. After a reload it is offered on the start page.",
    copy_downloaded: "downloaded",
    upload_failed: "The file could not be read:",
    delete_all_copies: "Delete all interim copies",
    delete_all_copies_question: "All interim copies offered here are deleted from this browser for good. Their content is then lost.",
    waits_for: "Waits for",
    followed_by: "Followed by",
    add_predecessor: "+ Predecessor …",
    remove_dependency: "Remove dependency",
    schedule_tab: "Schedule",
    unscheduled: "Without dates:",
    critical_hint: "Red: critical path – if one of these slips, the end slips.",
    project_due: "Project deadline",
    critical: "critical",
    kinds: ["Project", "Case"],
    months: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    weekdays: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    ribbon: RibbonTexts {
        start: "Home",
        task: "Task",
        new: "New",
        protection: "Protection",
        close: "Close",
        undo: "Undo",
        find: "Find",
        views: "Views",
        areas: "Areas",
        zoom: "Zoom",
        share: "Share",
        new_case: "Case",
        case_kind: "Case",
        new_project: "Project",
        from_template: "From template",
        find_action: "Find a command",
        collapse: "Collapse the ribbon",
        expand: "Expand the ribbon",
        scroll_left: "Scroll the ribbon left",
        scroll_right: "Scroll the ribbon right",
        new_case_long: "New case",
        schedule_tools: "Schedule tools",
        navigate: "Go to",
        edit: "Change",
        new_project_long: "New project",
    },
};

/// Name of a task status (board column).
pub fn task_status(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => pick("Offen", "To do"),
        TaskStatus::InProgress => pick("In Arbeit", "In progress"),
        TaskStatus::Blocked => pick("Blockiert", "Blocked"),
        TaskStatus::Review => pick("Prüfung", "Review"),
        TaskStatus::Done => pick("Erledigt", "Done"),
    }
}

/// Name of a priority.
pub fn priority(priority: Priority) -> &'static str {
    match priority {
        Priority::Low => pick("Niedrig", "Low"),
        Priority::Normal => pick("Normal", "Normal"),
        Priority::High => pick("Hoch", "High"),
        Priority::Critical => pick("Kritisch", "Critical"),
    }
}

/// Name of a project status.
pub fn project_status(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::Draft => pick("Entwurf", "Draft"),
        ProjectStatus::Active => pick("Aktiv", "Active"),
        ProjectStatus::OnHold => pick("Pausiert", "On hold"),
        ProjectStatus::Done => pick("Abgeschlossen", "Done"),
        ProjectStatus::Archived => pick("Archiviert", "Archived"),
    }
}

/// Name of the state of a milestone.
pub fn milestone_state(state: MilestoneState) -> &'static str {
    match state {
        MilestoneState::Open => pick("offen", "open"),
        MilestoneState::Reached => pick("erreicht", "reached"),
        MilestoneState::Missed => pick("verpasst", "missed"),
    }
}

/// "in 3 Tagen", "heute", "seit 2 Tagen überfällig".
pub fn days_left(days: i64) -> String {
    match (language(), days) {
        (Language::German, 0) => "heute fällig".to_owned(),
        (Language::German, 1) => "morgen fällig".to_owned(),
        (Language::German, -1) => "seit 1 Tag überfällig".to_owned(),
        (Language::German, d) if d > 1 => format!("in {d} Tagen"),
        (Language::German, d) => format!("seit {} Tagen überfällig", d.unsigned_abs()),
        (Language::English, 0) => "due today".to_owned(),
        (Language::English, 1) => "due tomorrow".to_owned(),
        (Language::English, -1) => "1 day overdue".to_owned(),
        (Language::English, d) if d > 1 => format!("in {d} days"),
        (Language::English, d) => format!("{} days overdue", d.unsigned_abs()),
    }
}

/// How a deadline stands at `now`, for people, and whether it has passed: "in 3 Tagen",
/// "heute bis 14:00 Uhr", "seit 14:00 Uhr überfällig", "seit 2 Tagen überfällig".
pub fn deadline_state(
    due: jiff::civil::Date,
    time: Option<jiff::civil::Time>,
    now: jiff::civil::DateTime,
) -> (String, bool) {
    let past = plnwrk_engine::query::is_past(due, time, now);
    let days = plnwrk_engine::query::days_between(now.date(), due);
    let text = match (time, days, past) {
        (Some(time), 0, true) => {
            let time = crate::dates::format_time(time);
            pick(
                format!("seit {time} Uhr überfällig"),
                format!("overdue since {time}"),
            )
        }
        (Some(time), 0, false) => {
            let time = crate::dates::format_time(time);
            pick(
                format!("heute bis {time} Uhr"),
                format!("due today at {time}"),
            )
        }
        _ => days_left(days),
    };
    (text, past)
}

/// "„32.1.2026“ ist kein Datum (TT.MM.JJJJ)".
pub fn not_a_date(typed: &str) -> String {
    pick(
        format!("„{typed}“ ist kein Datum (TT.MM.JJJJ)"),
        format!("“{typed}” is not a date (YYYY-MM-DD)"),
    )
}

/// "„25:00“ ist keine Uhrzeit (HH:MM)".
pub fn not_a_time(typed: &str) -> String {
    pick(
        format!("„{typed}“ ist keine Uhrzeit (HH:MM)"),
        format!("“{typed}” is not a time (HH:MM)"),
    )
}

/// "40 % · 2 von 5 erledigt".
pub fn progress(percent: u8, done: usize, tasks: usize) -> String {
    pick(
        format!("{percent} % · {done} von {tasks} erledigt"),
        format!("{percent} % · {done} of {tasks} done"),
    )
}

/// "3 offen".
pub fn open_tasks(count: usize) -> String {
    pick(format!("{count} offen"), format!("{count} open"))
}

/// "2 überfällig".
pub fn overdue_tasks(count: usize) -> String {
    pick(format!("{count} überfällig"), format!("{count} overdue"))
}

/// The columns of the schedule table (CSV export).
pub fn schedule_csv_header() -> [&'static str; 14] {
    pick(
        [
            "Aufgabe",
            "Ebene",
            "Oberaufgabe",
            "Status",
            "Zuständig",
            "Beginn",
            "Frist",
            "Uhrzeit",
            "Dauer (Tage)",
            "Puffer (Tage)",
            "Kritisch",
            "Meilenstein",
            "Wartet auf",
            "Fortschritt (%)",
        ],
        [
            "Task",
            "Level",
            "Parent",
            "Status",
            "Assigned to",
            "Start",
            "Due",
            "Time",
            "Length (days)",
            "Float (days)",
            "Critical",
            "Milestone",
            "Waits for",
            "Progress (%)",
        ],
    )
}

/// A month on the time axis: "Okt 2026", "Oct 2026".
pub fn month(date: jiff::civil::Date) -> String {
    const DE: [&str; 12] = [
        "Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
    ];
    const EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let names = pick(DE, EN);
    let name = usize::try_from(date.month() - 1)
        .ok()
        .and_then(|i| names.get(i))
        .copied()
        .unwrap_or_default();
    format!("{name} {}", date.year())
}

/// "2 Unteraufgaben sind noch offen."
pub fn subtasks_still_open(count: usize) -> String {
    match (language(), count) {
        (Language::German, 1) => "1 Unteraufgabe ist noch offen.".to_owned(),
        (Language::German, n) => format!("{n} Unteraufgaben sind noch offen."),
        (Language::English, 1) => "1 subtask is still open.".to_owned(),
        (Language::English, n) => format!("{n} subtasks are still open."),
    }
}

/// A push larger than one step can take, refused.
pub fn too_many_moves(count: usize) -> String {
    pick(
        format!(
            "Nicht verschoben: {count} Nachfolger müssten mit, mehr als in einem Schritt geht. Abhängigkeiten lösen oder in Teilen verschieben."
        ),
        format!(
            "Not moved: {count} successors would have to move along, more than one step can take. Remove dependencies or move in parts."
        ),
    )
}

/// "2 Nachfolger mitverschoben".
pub fn moved_along(count: usize) -> String {
    match (language(), count) {
        (Language::German, 1) => "1 Nachfolger mitverschoben.".to_owned(),
        (Language::German, n) => format!("{n} Nachfolger mitverschoben."),
        (Language::English, 1) => "1 successor moved along.".to_owned(),
        (Language::English, n) => format!("{n} successors moved along."),
    }
}

/// "KW 42" (calendar week).
pub fn week(number: i8) -> String {
    pick(format!("KW {number}"), format!("CW {number}"))
}

/// "Puffer: 3 Tage".
/// A span with weekdays, for the preview while dragging: "Fr 16.10. – Di 20.10.".
pub fn span_with_weekdays(from: jiff::civil::Date, to: jiff::civil::Date) -> String {
    let day = |d: jiff::civil::Date| {
        let weekday = t()
            .weekdays
            .get(usize::try_from(d.weekday().to_monday_zero_offset()).unwrap_or(0))
            .copied()
            .unwrap_or("");
        format!("{weekday} {}", crate::dates::format_short(d))
    };
    if from == to {
        day(to)
    } else {
        format!("{} – {}", day(from), day(to))
    }
}

/// Tasks a milestone waits for that end after its new date.
pub fn late_predecessors(count: usize) -> String {
    match count {
        1 => pick(
            "1 Aufgabe, auf die er wartet, endet danach.",
            "1 task it waits for ends after it.",
        )
        .to_owned(),
        n => pick(
            format!("{n} Aufgaben, auf die er wartet, enden danach."),
            format!("{n} tasks it waits for end after it."),
        ),
    }
}

/// How many tasks that wait move along with a change: "+2 Aufgaben verschieben sich".
pub fn reached_on(day: Option<&str>) -> String {
    match day {
        Some(day) => pick(format!("erreicht am {day}"), format!("reached on {day}")),
        None => pick("erreicht", "reached").to_owned(),
    }
}

pub fn open_predecessors(count: usize) -> String {
    match count {
        0 => pick("Wartet auf nichts Offenes", "Waits for nothing open").to_owned(),
        1 => pick("Wartet auf 1 offene Aufgabe", "Waits for 1 open task").to_owned(),
        n => pick(
            format!("Wartet auf {n} offene Aufgaben"),
            format!("Waits for {n} open tasks"),
        ),
    }
}

/// Why a milestone is at risk: little slack before it.
pub fn little_slack(days: i32) -> String {
    match (language(), days) {
        (Language::German, 1) => "Nur 1 Tag Spielraum bis hierhin.".to_owned(),
        (Language::German, n) if n <= 0 => "Kein Spielraum bis hierhin.".to_owned(),
        (Language::German, n) => format!("Nur {n} Tage Spielraum bis hierhin."),
        (Language::English, 1) => "Only 1 day of slack up to here.".to_owned(),
        (Language::English, n) if n <= 0 => "No slack up to here.".to_owned(),
        (Language::English, n) => format!("Only {n} days of slack up to here."),
    }
}

/// Milestones a move puts before what they wait for (they never move along).
pub fn overtakes(titles: &[String]) -> String {
    let quoted = |open: &str, close: &str| {
        titles
            .iter()
            .map(|t| format!("{open}{t}{close}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match (language(), titles.len()) {
        (Language::German, 1) => format!(
            "Danach liegt der Meilenstein {} vor einer Aufgabe, auf die er wartet.",
            quoted("„", "“")
        ),
        (Language::German, _) => format!(
            "Danach liegen die Meilensteine {} vor Aufgaben, auf die sie warten.",
            quoted("„", "“")
        ),
        (Language::English, 1) => format!(
            "The milestone {} then lies before a task it waits for.",
            quoted("“", "”")
        ),
        (Language::English, _) => format!(
            "The milestones {} then lie before tasks they wait for.",
            quoted("“", "”")
        ),
    }
}

/// The kind of a project as shown: the two base words ("Vorgang"/"Case", "Projekt"/"Project")
/// in the language of the interface, any other kind ("SKA") as typed.
pub fn kind_shown(kind: &str) -> String {
    let base = kind.trim().to_lowercase();
    let [project, case] = t().kinds;
    match base.as_str() {
        "projekt" | "project" => project.to_owned(),
        "vorgang" | "case" => case.to_owned(),
        _ => kind.to_owned(),
    }
}

/// The tick of a task in "Heute und überfällig", as read out.
pub fn mark_done(title: &str) -> String {
    pick(
        format!("„{title}“ erledigen"),
        format!("Mark “{title}” done"),
    )
}

/// A title longer than the engine takes, said at the field.
pub fn title_too_long(max: usize) -> String {
    pick(
        format!("Höchstens {max} Zeichen."),
        format!("At most {max} characters."),
    )
}

/// A card made with "+" that the board's filter hides.
pub fn created_but_filtered(title: &str) -> String {
    pick(
        format!("„{title}“ angelegt – der Filter blendet die Karte aus."),
        format!("“{title}” created – the filter hides the card."),
    )
}

/// The "+" of a board column: a new task with this status.
pub fn new_in_column(status: TaskStatus) -> String {
    pick(
        format!("Neue Aufgabe in „{}“", task_status(status)),
        format!("New task in “{}”", task_status(status)),
    )
}

/// A column heading: its name and how many cards (with a filter: shown of all).
pub fn column_heading(status: TaskStatus, shown: usize, all: usize) -> String {
    if shown == all {
        format!("{} ({all})", task_status(status))
    } else {
        pick(
            format!("{} ({shown} von {all})", task_status(status)),
            format!("{} ({shown} of {all})", task_status(status)),
        )
    }
}

/// While a card is dragged to "Erledigt": it still waits for open work (a hint, not a stop).
pub fn still_waits_for(open: usize) -> String {
    match open {
        1 => pick(
            "Wartet noch auf 1 offene Aufgabe",
            "Still waits for 1 open task",
        )
        .to_owned(),
        n => pick(
            format!("Wartet noch auf {n} offene Aufgaben"),
            format!("Still waits for {n} open tasks"),
        ),
    }
}

pub fn moves_along(count: usize) -> String {
    match count {
        1 => pick("+1 Aufgabe verschiebt sich", "+1 task moves along").to_owned(),
        n => pick(
            format!("+{n} Aufgaben verschieben sich"),
            format!("+{n} tasks move along"),
        ),
    }
}

pub fn float(days: i32) -> String {
    match (language(), days) {
        (Language::German, 1) => "Spielraum: 1 Tag".to_owned(),
        (Language::German, n) => format!("Spielraum: {n} Tage"),
        (Language::English, 1) => "Slack: 1 day".to_owned(),
        (Language::English, n) => format!("Slack: {n} days"),
    }
}

/// A dependency that does not hold.
pub fn waits_too_short(from: &str, to: &str) -> String {
    pick(
        format!(
            "{} beginnt, bevor {} so weit ist.",
            quoted(to),
            quoted(from)
        ),
        format!("{} starts before {} allows.", quoted(to), quoted(from)),
    )
}

/// The chip of an overtaken milestone in the overview: the earliest day it could be.
pub fn earliest_on(day: &str) -> String {
    pick(
        format!("frühestens {day}"),
        format!("{day} at the earliest"),
    )
}

/// The rest of its line: its own date and what holds it back.
pub fn overtaken_by(date: &str, by: &[String]) -> String {
    let names = by.iter().map(|t| quoted(t)).collect::<Vec<_>>().join(", ");
    pick(
        format!("am {date} – wartet auf {names}"),
        format!("on {date} – waits for {names}"),
    )
}

/// A milestone that cannot hold.
pub fn milestone_unreachable(task: &str, earliest: &str) -> String {
    pick(
        format!(
            "Meilenstein {} nicht haltbar – frühestens {earliest}.",
            quoted(task)
        ),
        format!(
            "Milestone {} cannot hold – {earliest} at the earliest.",
            quoted(task)
        ),
    )
}

/// A task that ends after the project's deadline.
pub fn after_project_due(task: &str) -> String {
    pick(
        format!("{} endet nach der Projektfrist.", quoted(task)),
        format!("{} ends after the project deadline.", quoted(task)),
    )
}

/// Something typed, in quotation marks of the language.
pub fn quoted(text: &str) -> String {
    pick(format!("„{text}“"), format!("“{text}”"))
}

/// A broken rule of the data, for people.
pub fn rule(rule: Rule) -> &'static str {
    match rule {
        Rule::Text => pick(
            "Ein Text ist leer, zu lang oder enthält Steuerzeichen.",
            "A text is empty, too long or contains control characters.",
        ),
        Rule::MissingReference => pick(
            "Etwas verweist auf einen Eintrag, den es nicht gibt.",
            "Something refers to an entry that does not exist.",
        ),
        Rule::CrossProject => pick(
            "Unteraufgaben und Abhängigkeiten bleiben im selben Projekt.",
            "Subtasks and dependencies stay within one project.",
        ),
        Rule::Cycle => pick(
            "Das ergäbe einen Kreis: Eine Aufgabe läge unter sich selbst oder wartete auf sich selbst.",
            "That would make a circle: a task below itself or waiting for itself.",
        ),
        Rule::DateOrder => pick(
            "Die Frist liegt vor dem Beginn.",
            "The deadline is before the start.",
        ),
        Rule::Range => pick(
            "Ein Wert liegt außerhalb des erlaubten Bereichs.",
            "A value is out of range.",
        ),
        Rule::Classification => pick(
            "Eine Aufgabe darf nicht niedriger eingestuft sein als ihr Projekt.",
            "A task may not be classified lower than its project.",
        ),
        Rule::Duplicate => pick(
            "Diese Abhängigkeit gibt es schon.",
            "This dependency exists already.",
        ),
        Rule::SelfReference => pick(
            "Eine Aufgabe kann nicht von sich selbst abhängen.",
            "A task cannot depend on itself.",
        ),
        Rule::Corrupt => pick(
            "Die Daten passen nicht zusammen.",
            "The data does not fit together.",
        ),
        Rule::Milestone => pick(
            "Ein Meilenstein steht auf oberster Ebene und hat keine Unteraufgaben.",
            "A milestone is on the top level and has no subtasks.",
        ),
    }
}

/// Name of a changed field in the history; `None` for bookkeeping fields not worth showing.
pub fn field(name: &str) -> Option<String> {
    let t = t();
    let known = match name {
        "updated" | "created" | "id" => return None,
        "title" => t.title,
        "description" => t.description,
        "status" => t.status,
        "priority" => t.priority,
        "assignee" => t.assignee,
        "start" => t.start,
        "due" => t.due,
        "due_time" => pick("Uhrzeit", "time"),
        "milestone" => t.milestone,
        "progress" => t.progress,
        "tags" => t.tags,
        "parent" => pick("übergeordnete Aufgabe", "parent task"),
        "project" => pick("Projekt", "project"),
        "rank" => pick("Reihenfolge", "order"),
        "classification" => pick("Einstufung", "classification"),
        "estimate_minutes" => pick("Schätzung", "estimate"),
        other => return Some(other.to_owned()),
    };
    Some(known.to_owned())
}

/// A refused command, for people.
pub fn command_error(error: &CommandError) -> String {
    match error {
        CommandError::BadParam { param, reason } => pick(
            format!("Eingabe nicht gültig ({param}): {reason}"),
            format!("Input not valid ({param}): {reason}"),
        ),
        CommandError::Disabled { .. } => pick(
            "Das ist gerade nicht möglich.",
            "That is not possible right now.",
        )
        .to_owned(),
        CommandError::NotFound { .. } => {
            pick("Das gibt es nicht mehr.", "That does not exist any more.").to_owned()
        }
        CommandError::Invalid { violations } => match violations.first() {
            Some(v) => format!(
                "{} {}",
                pick("Nicht erlaubt:", "Not allowed:"),
                rule(v.rule)
            ),
            None => pick("Nicht erlaubt.", "Not allowed.").to_owned(),
        },
        CommandError::Conflict { reason } => format!("{} {reason}", pick("Konflikt:", "Conflict:")),
        CommandError::UnknownCommand { command } => {
            format!(
                "{} {command}",
                pick("Unbekannter Befehl", "Unknown command")
            )
        }
    }
}

/// A file that could not be read or written, for people.
pub fn store_error(error: &StoreError) -> String {
    match error {
        StoreError::NotPlnwrk => {
            pick("Das ist keine plnwrk-Datei.", "This is not a plnwrk file.").to_owned()
        }
        StoreError::NewerFormat { .. } => pick(
            "Die Datei stammt aus einer neueren plnwrk-Version. Bitte plnwrk aktualisieren.",
            "The file comes from a newer version of plnwrk. Please update plnwrk.",
        )
        .to_owned(),
        StoreError::UnsupportedFormat { found } => pick(
            format!("Dateiformat {found} wird nicht unterstützt."),
            format!("File format {found} is not supported."),
        ),
        StoreError::Corrupt { reason } => pick(
            format!("Die Datei ist beschädigt: {reason}"),
            format!("The file is damaged: {reason}"),
        ),
        StoreError::TooLarge { .. } => {
            pick("Die Datei ist zu groß.", "The file is too large.").to_owned()
        }
        StoreError::PassphraseRequired => {
            pick("Die Datei ist verschlüsselt.", "The file is encrypted.").to_owned()
        }
        StoreError::WrongPassphrase => {
            pick("Die Passphrase ist falsch.", "The passphrase is wrong.").to_owned()
        }
        StoreError::ClassifiedUnencrypted => pick(
            "VS-NfD-Inhalte dürfen nur verschlüsselt gespeichert werden.",
            "VS-NfD content may only be saved encrypted.",
        )
        .to_owned(),
        StoreError::ChainBroken { seq } => pick(
            format!("Das Änderungsprotokoll wurde bei Änderung {seq} verändert."),
            format!("The change log was altered at change {seq}."),
        ),
        StoreError::SnapshotMismatch => pick(
            "Datenstand und Änderungsprotokoll passen nicht zusammen.",
            "The data and the change log do not match.",
        )
        .to_owned(),
        StoreError::Invalid { violations } => pick(
            format!("Die Daten verletzen {} Regel(n).", violations.len()),
            format!("The data breaks {} rule(s).", violations.len()),
        ),
        StoreError::Encryption { reason } => pick(
            format!("Verschlüsselung fehlgeschlagen: {reason}"),
            format!("Encryption failed: {reason}"),
        ),
        StoreError::InUse { path } => pick(
            format!(
                "{path} wird gerade von jemand anderem gespeichert oder ist in einer älteren \
                 plnwrk-Version geöffnet. Bitte gleich noch einmal versuchen."
            ),
            format!(
                "{path} is being saved by someone else right now or is open in an older plnwrk. \
                 Please try again in a moment."
            ),
        ),
        StoreError::ChangedMeanwhile { path } => pick(
            format!(
                "{path} hat sich beim Speichern geändert (das Laufwerk hat die Sperre nicht \
                 beachtet). Nichts wurde überschrieben – beim nächsten Speichern wird \
                 zusammengeführt."
            ),
            format!(
                "{path} changed while saving (the drive ignored the lock). Nothing was \
                 overwritten – the next save merges."
            ),
        ),
        StoreError::Io { reason } => pick(
            format!("Dateizugriff fehlgeschlagen: {reason}"),
            format!("File access failed: {reason}"),
        ),
    }
}

/// A value of a field as people read it: a date as a date, nothing as a dash, lists joined.
pub fn value_shown(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "–".to_owned(),
        serde_json::Value::String(text) => match text.parse::<jiff::civil::Date>() {
            Ok(day) => crate::dates::format(day),
            Err(_) => text.clone(),
        },
        serde_json::Value::Array(items) => {
            items.iter().map(value_shown).collect::<Vec<_>>().join(", ")
        }
        serde_json::Value::Bool(true) => pick("ja", "yes").to_owned(),
        serde_json::Value::Bool(false) => pick("nein", "no").to_owned(),
        other => other.to_string(),
    }
}

/// Fields and their values, e.g. "Frist 20.11.2026, Uhrzeit –".
pub fn fields_shown(values: &serde_json::Map<String, serde_json::Value>) -> String {
    values
        .iter()
        .filter_map(|(name, value)| {
            field(name).map(|name| format!("{name} {}", value_shown(value)))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Another person has this name or these initials already (adding the same person twice by
/// mistake in a shared file, Phase 10a).
pub fn same_person(name: &str) -> String {
    pick(
        format!("Es gibt schon „{name}“ mit diesem Namen oder Kürzel."),
        format!("“{name}” has this name or initials already."),
    )
}

/// The comment a conflict leaves on a task: both values, who had which.
pub fn conflict_comment(theirs: &str, by: &str, ours: &str, me: &str) -> String {
    pick(
        format!("Gleichzeitig geändert: {by} hatte {theirs}; gespeichert wurde {ours} ({me})."),
        format!("Changed at the same time: {by} had {theirs}; saved was {ours} ({me})."),
    )
}

/// Saved and merged with what others saved meanwhile.
pub fn merged_saved(taken: usize, from: &str, conflicts: usize, lost: &[String]) -> String {
    let mut said = pick(
        format!("Gespeichert und zusammengeführt: {taken} Änderung(en) von {from} übernommen."),
        format!("Saved and merged: {taken} change(s) by {from} taken."),
    );
    if conflicts > 0 {
        said.push(' ');
        said.push_str(&pick(
            format!("Bei {conflicts} Angabe(n) galt deine – der andere Wert steht als Kommentar an der Aufgabe."),
            format!("For {conflicts} value(s) yours was kept – the other one is a comment on the task."),
        ));
    }
    if !lost.is_empty() {
        said.push(' ');
        said.push_str(&pick(
            format!(
                "Nicht übernommen, weil inzwischen anders: {}.",
                lost.join("; ")
            ),
            format!("Not taken, as it changed meanwhile: {}.", lost.join("; ")),
        ));
    }
    said
}

/// Others' changes fetched from the shared file.
pub fn fetched(taken: usize, from: &str) -> String {
    pick(
        format!("Aktualisiert: {taken} Änderung(en) von {from}."),
        format!("Updated: {taken} change(s) by {from}."),
    )
}

/// Someone unknown (a change without a person: the command line, an AI).
pub fn someone() -> &'static str {
    pick("jemand anderem", "someone else")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// String literals of a Rust source, outside comments and before its test module: plain,
    /// raw (`r#"…"#`) and continued over lines (`\` at the end of a line).
    fn literals(source: &str) -> Vec<String> {
        // Windows checkouts may have CRLF line ends.
        let source = source.replace("\r\n", "\n");
        let code = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap_or_default();
        let chars: Vec<char> = code.chars().collect();
        let at = |i: usize| chars.get(i).copied();
        let mut found = Vec::new();
        let mut i = 0;
        while let Some(c) = at(i) {
            match c {
                '/' if at(i + 1) == Some('/') => {
                    while at(i).is_some_and(|c| c != '\n') {
                        i += 1;
                    }
                }
                '/' if at(i + 1) == Some('*') => {
                    let mut depth = 0usize;
                    loop {
                        match (at(i), at(i + 1)) {
                            (Some('/'), Some('*')) => {
                                depth += 1;
                                i += 2;
                            }
                            (Some('*'), Some('/')) => {
                                depth -= 1;
                                i += 2;
                                if depth == 0 {
                                    break;
                                }
                            }
                            (Some(_), _) => i += 1,
                            (None, _) => break,
                        }
                    }
                    continue;
                }
                'r' if !at(i.wrapping_sub(1)).is_some_and(|p| p.is_alphanumeric() || p == '_')
                    && matches!(at(i + 1), Some('"' | '#')) =>
                {
                    let mut hashes = 0;
                    let mut j = i + 1;
                    while at(j) == Some('#') {
                        hashes += 1;
                        j += 1;
                    }
                    if at(j) != Some('"') {
                        i += 1;
                        continue;
                    }
                    j += 1;
                    let mut text = String::new();
                    while let Some(c) = at(j) {
                        if c == '"' && (1..=hashes).all(|k| at(j + k) == Some('#')) {
                            break;
                        }
                        text.push(c);
                        j += 1;
                    }
                    found.push(text);
                    i = j + 1 + hashes;
                    continue;
                }
                '"' => {
                    let mut text = String::new();
                    i += 1;
                    while let Some(c) = at(i) {
                        match c {
                            '\\' if at(i + 1) == Some('\n') => {
                                // Continued on the next line: its leading spaces do not count.
                                i += 2;
                                while at(i).is_some_and(char::is_whitespace) {
                                    i += 1;
                                }
                                continue;
                            }
                            '\\' => {
                                if let Some(next) = at(i + 1) {
                                    text.push(next);
                                }
                                i += 2;
                                continue;
                            }
                            '"' => break,
                            c => text.push(c),
                        }
                        i += 1;
                    }
                    found.push(text);
                }
                '\'' => {
                    // A char literal ('"', '\'') must not start a string; a lifetime has no
                    // closing quote.
                    if at(i + 1) == Some('\\') {
                        i += 2;
                        while at(i).is_some_and(|c| c != '\'') {
                            i += 1;
                        }
                    } else if at(i + 2) == Some('\'') {
                        i += 2;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        found
    }

    /// Texts on disk that are not interface texts: the recovery folder and copy names stay
    /// the same in every language, so copies are found again after switching.
    const NOT_INTERFACE: &[&str] = &[
        " – Wiederherstellung ",
        "{} – Wiederherstellung {}.plnwrk",
        "Notkopien",
    ];

    /// Whether a text looks German: German letters or quotes, or a word only German uses. The
    /// words are taken from the German texts themselves, so new texts widen the net.
    fn looks_german(text: &str) -> bool {
        if text.chars().any(|c| "äöüÄÖÜß„“".contains(c)) {
            return true;
        }
        let english = format!("{EN:?}");
        let english: std::collections::HashSet<&str> =
            english.split(|c: char| !c.is_alphanumeric()).collect();
        let german = format!("{DE:?}");
        let german: std::collections::HashSet<&str> = german
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 3 && !english.contains(w))
            .collect();
        text.split(|c: char| !c.is_alphanumeric())
            .any(|w| german.contains(w))
    }

    #[test]
    fn interface_texts_live_only_here() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![src.clone()];
        let mut german = Vec::new();
        let mut checked = 0;
        while let Some(path) = files.pop() {
            if path.is_dir() {
                files.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            // Tests (also those in `src/tests/`) hold German data, not interface texts.
            let in_tests = path.parent().is_some_and(|dir| dir.ends_with("tests"));
            if in_tests || ["text.rs", "tests.rs", "model_tests.rs"].contains(&name.as_str()) {
                continue;
            }
            checked += 1;
            for literal in literals(&std::fs::read_to_string(&path).unwrap()) {
                if looks_german(&literal) && !NOT_INTERFACE.contains(&literal.as_str()) {
                    german.push(format!("{name}: {literal}"));
                }
            }
        }
        assert!(checked >= 10, "the sources were found");
        assert!(
            german.is_empty(),
            "German texts outside text.rs: {german:#?}"
        );
    }

    #[test]
    fn the_literal_scanner_sees_continued_and_raw_strings() {
        let source = "let a = \"Erste Zeile \\\n        nicht übernommen\";\n\
                      /* \"im Kommentar\" */ let b = r#\"Bitte \"x\" jetzt\"#;\n\
                      fn f<'a>(x: &'a str) -> &'a str { \"Frist\" }";
        assert_eq!(
            literals(source),
            vec!["Erste Zeile nicht übernommen", "Bitte \"x\" jetzt", "Frist"]
        );
        let windows =
            "fn f() {}\r\n#[cfg(test)]\r\nmod tests { const X: &str = \"Neue Aufgabe\"; }";
        assert!(
            literals(windows).is_empty(),
            "the test module is skipped with CRLF too"
        );
        let continued = "let a = \"Erste \\\r\n    Zeile\";";
        assert_eq!(literals(continued), vec!["Erste Zeile"]);
        assert!(looks_german("Speichern"));
        assert!(looks_german("Abbrechen"));
        assert!(!looks_german("Save"));
        assert!(!looks_german("task.update"));
    }

    #[test]
    fn the_literal_scanner_sees_strings_and_skips_comments() {
        let source =
            "let a = \"Neue Aufgabe\"; // \"Kommentar\"\nlet b = '\"'; let c = \"x\\\"y\";";
        assert_eq!(literals(source), vec!["Neue Aufgabe", "x\"y"]);
    }

    #[test]
    fn both_languages_differ_where_they_should() {
        // A text left untranslated in EN shows German in the English interface.
        let same = [
            "nobody",
            "zoom",
            "time_hint",
            "vs_nfd",
            "version",
            "status",
            "name",
            "board",
            "passphrase",
            "new_person",
        ];
        let de = format!("{DE:?}");
        let en = format!("{EN:?}");
        let fields = |debug: &str| -> Vec<(String, String)> {
            debug
                .trim_start_matches("Texts {")
                .split(", ")
                .filter_map(|part| part.split_once(": "))
                .map(|(k, v)| (k.trim().to_owned(), v.to_owned()))
                .collect()
        };
        let english: std::collections::HashMap<_, _> = fields(&en).into_iter().collect();
        // Every text field of the struct (from the source) must have been compared, so a
        // change of the Debug format cannot make this test check nothing.
        let declared: std::collections::BTreeSet<String> = include_str!("text.rs")
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub "))
            .filter_map(|rest| rest.strip_suffix(": &'static str,"))
            .map(str::to_owned)
            .collect();
        let mut compared = std::collections::BTreeSet::new();
        for (key, german) in fields(&de) {
            if !declared.contains(&key) {
                continue; // a piece of a value that contains ", "
            }
            compared.insert(key.clone());
            if same.contains(&key.as_str()) {
                continue;
            }
            assert_ne!(english.get(&key), Some(&german), "{key} is not translated");
        }
        assert!(declared.len() > 100, "the fields were found");
        assert_eq!(compared, declared, "every text was compared");
    }

    #[test]
    fn days_read_naturally() {
        assert_eq!(days_left(0), "heute fällig");
        assert_eq!(days_left(1), "morgen fällig");
        assert_eq!(days_left(5), "in 5 Tagen");
        assert_eq!(days_left(-1), "seit 1 Tag überfällig");
        assert_eq!(days_left(-3), "seit 3 Tagen überfällig");
    }

    #[test]
    fn deadlines_with_a_time_read_naturally() {
        let day: jiff::civil::Date = "2026-10-14".parse().unwrap();
        let noon = Some("12:00".parse().unwrap());
        let at = |t: &str| format!("2026-10-14T{t}").parse().unwrap();
        assert_eq!(
            deadline_state(day, noon, at("11:00")),
            ("heute bis 12:00 Uhr".to_owned(), false)
        );
        assert_eq!(
            deadline_state(day, noon, at("15:00")),
            ("seit 12:00 Uhr überfällig".to_owned(), true)
        );
        assert_eq!(
            deadline_state(day, None, at("23:00")),
            ("heute fällig".to_owned(), false)
        );
        assert_eq!(
            deadline_state(day, None, "2026-10-16T08:00".parse().unwrap()),
            ("seit 2 Tagen überfällig".to_owned(), true)
        );
    }

    #[test]
    fn every_status_has_a_distinct_name() {
        let names: std::collections::BTreeSet<_> =
            TaskStatus::ALL.iter().map(|s| task_status(*s)).collect();
        assert_eq!(names.len(), TaskStatus::ALL.len());
    }
}
