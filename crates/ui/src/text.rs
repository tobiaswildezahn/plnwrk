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
    pub nothing_overdue: &'static str,
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
    pub delete_milestone: &'static str,
    pub close_details: &'static str,
    pub rename_hint: &'static str,
    pub no_tasks: &'static str,
    pub new_milestone: &'static str,
    pub no_milestones: &'static str,
    pub reached: &'static str,
    pub subtasks: &'static str,
    pub comments: &'static str,
    pub new_comment_hint: &'static str,
    pub add_comment: &'static str,
    pub history: &'static str,
    /// Marks a change in the history made by an AI (over MCP).
    pub by_ai: &'static str,
    pub delete_task: &'static str,
    pub delete_task_question: &'static str,
    pub close_subtasks_title: &'static str,
    pub whole_project: &'static str,
    pub columns: &'static str,
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
    pub new_project_hint: &'static str,
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
    pub no_scheduled: &'static str,
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
    nothing_yet: "Noch keine Projekte oder Vorgänge. Lege mit „+ Neu“ eines an.",
    overdue: "Überfällig",
    nothing_overdue: "Nichts ist überfällig.",
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
    delete_milestone: "Meilenstein löschen",
    close_details: "Details schließen",
    rename_hint: "Doppelklick: umbenennen",
    no_tasks: "Noch keine Aufgaben.",
    new_milestone: "+ Meilenstein",
    no_milestones: "Noch keine Meilensteine.",
    reached: "erreicht",
    subtasks: "Unteraufgaben",
    comments: "Kommentare",
    new_comment_hint: "Kommentar schreiben …",
    add_comment: "Kommentieren",
    history: "Verlauf",
    by_ai: "durch KI",
    delete_task: "Aufgabe löschen",
    delete_task_question: "Die Aufgabe wird mit allen Unteraufgaben und Kommentaren gelöscht (rückgängig machbar).",
    close_subtasks_title: "Unteraufgaben auch erledigen?",
    whole_project: "Ganzes Projekt",
    columns: "Spalten",
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
    new_project_hint: "Ein Vorgang ist ein Projekt mit Aktenzeichen und Frist.",
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
    no_scheduled: "Noch keine Aufgabe mit Termin. Beginn oder Frist in den Details setzen.",
    unscheduled: "Ohne Termin:",
    critical_hint: "Rot: kritischer Pfad – rutscht eine dieser Aufgaben, rutscht das Ende. Balken ziehen verschiebt, Nachfolger rutschen mit.",
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
    nothing_yet: "No projects or cases yet. Create one with “+ New”.",
    overdue: "Overdue",
    nothing_overdue: "Nothing is overdue.",
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
    delete_milestone: "Delete milestone",
    close_details: "Close details",
    rename_hint: "Double-click: rename",
    no_tasks: "No tasks yet.",
    new_milestone: "+ Milestone",
    no_milestones: "No milestones yet.",
    reached: "reached",
    subtasks: "Subtasks",
    comments: "Comments",
    new_comment_hint: "Write a comment …",
    add_comment: "Comment",
    history: "History",
    by_ai: "by AI",
    delete_task: "Delete task",
    delete_task_question: "The task is deleted with all its subtasks and comments (can be undone).",
    close_subtasks_title: "Close the subtasks too?",
    whole_project: "Whole project",
    columns: "Columns",
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
    new_project_hint: "A case is a project with a reference and a deadline.",
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
    no_scheduled: "No task with dates yet. Set a start or deadline in the details.",
    unscheduled: "Without dates:",
    critical_hint: "Red: critical path – if one of these slips, the end slips. Drag a bar to move it; what waits for it moves along.",
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
    let days = now
        .date()
        .until(due)
        .map_or(0, |span| i64::from(span.get_days()));
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
pub fn float(days: i32) -> String {
    match (language(), days) {
        (Language::German, 1) => "Puffer: 1 Tag".to_owned(),
        (Language::German, n) => format!("Puffer: {n} Tage"),
        (Language::English, 1) => "Float: 1 day".to_owned(),
        (Language::English, n) => format!("Float: {n} days"),
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
            format!("{path} ist schon in einem anderen plnwrk geöffnet."),
            format!("{path} is already open in another plnwrk."),
        ),
        StoreError::Io { reason } => pick(
            format!("Dateizugriff fehlgeschlagen: {reason}"),
            format!("File access failed: {reason}"),
        ),
    }
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
            if ["text.rs", "tests.rs", "model_tests.rs"].contains(&name.as_str()) {
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
