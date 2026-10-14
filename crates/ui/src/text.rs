//! Every text of the user interface, in one place (German; English follows in Phase 3b).

use plnwrk_core::{Priority, ProjectStatus, TaskStatus};
use plnwrk_engine::CommandError;
use plnwrk_engine::query::MilestoneState;
use plnwrk_store::StoreError;

pub const UNTITLED: &str = "Unbenannt";
pub const WELCOME: &str = "Projekte und Vorgänge – in einer Datei, die dir gehört.";

pub const NEW_FILE: &str = "Neue Datei";
pub const OPEN_FILE: &str = "Datei öffnen…";
pub const OPEN_SAMPLE: &str = "Beispiel öffnen";
pub const SAVE: &str = "Speichern";
pub const SAVE_AS: &str = "Speichern unter…";
pub const CLOSE_FILE: &str = "Datei schließen";
pub const RECENT: &str = "Zuletzt geöffnet";
pub const RECOVERED: &str = "Wiederherstellen (automatisch gesichert, nicht gespeichert)";
pub const FILE_MENU: &str = "Datei";
pub const EDIT_MENU: &str = "Bearbeiten";
pub const UNDO: &str = "Rückgängig";
pub const REDO: &str = "Wiederholen";

pub const OVERVIEW: &str = "Übersicht";
pub const PROJECTS: &str = "Projekte und Vorgänge";
pub const TEMPLATES: &str = "Vorlagen";
pub const PEOPLE: &str = "Personen";
pub const NEW_PROJECT: &str = "+ Neu";
pub const FROM_TEMPLATE: &str = "+ Aus Vorlage";
pub const NOTHING_YET: &str = "Noch keine Projekte oder Vorgänge. Lege mit „+ Neu“ eines an.";
pub const OVERDUE: &str = "Überfällig";
pub const NOTHING_OVERDUE: &str = "Nichts ist überfällig.";

pub const LIST: &str = "Liste";
pub const BOARD: &str = "Board";
pub const MILESTONES: &str = "Meilensteine";
pub const EDIT: &str = "Bearbeiten";
pub const ARCHIVE: &str = "Archivieren";
pub const APPLY: &str = "Übernehmen";
pub const CANCEL: &str = "Abbrechen";
pub const CREATE: &str = "Anlegen";
pub const DELETE: &str = "Löschen";

pub const TITLE: &str = "Titel";
pub const KIND: &str = "Art";
pub const REFERENCE: &str = "Aktenzeichen";
pub const START: &str = "Beginn";
pub const DUE: &str = "Frist";
pub const OWNER: &str = "Verantwortlich";
pub const STATUS: &str = "Status";
pub const PRIORITY: &str = "Priorität";
pub const ASSIGNEE: &str = "Zuständig";
pub const PROGRESS: &str = "Fortschritt";
pub const TAGS: &str = "Schlagworte";
pub const DESCRIPTION: &str = "Beschreibung";
pub const TEMPLATE: &str = "Vorlage";
pub const NOBODY: &str = "–";

pub const NEW_TASK_HINT: &str = "Neue Aufgabe … (Enter)";
pub const NEW_SUBTASK_HINT: &str = "Neue Unteraufgabe … (Enter)";
pub const ADD_SUBTASK: &str = "Unteraufgabe hinzufügen";
pub const DELETE_MILESTONE: &str = "Meilenstein löschen";
pub const CLOSE_DETAILS: &str = "Details schließen";
pub const RENAME_HINT: &str = "Doppelklick: umbenennen";
pub const NO_TASKS: &str = "Noch keine Aufgaben.";
pub const NEW_MILESTONE: &str = "+ Meilenstein";
pub const NO_MILESTONES: &str = "Noch keine Meilensteine.";
pub const REACHED: &str = "erreicht";

pub const SUBTASKS: &str = "Unteraufgaben";
pub const COMMENTS: &str = "Kommentare";
pub const NEW_COMMENT_HINT: &str = "Kommentar schreiben …";
pub const ADD_COMMENT: &str = "Kommentieren";
pub const HISTORY: &str = "Verlauf";
pub const DELETE_TASK: &str = "Aufgabe löschen";
pub const DELETE_TASK_QUESTION: &str =
    "Die Aufgabe wird mit allen Unteraufgaben und Kommentaren gelöscht (rückgängig machbar).";

pub const NAME: &str = "Name";
pub const ROLE: &str = "Funktion";
pub const INITIALS: &str = "Kürzel";
pub const EMAIL: &str = "E-Mail";
pub const NEW_PERSON: &str = "+ Person";
pub const I_AM: &str = "Ich arbeite als";

pub const PASSPHRASE: &str = "Passphrase";
pub const PASSPHRASE_AGAIN: &str = "Passphrase wiederholen";
pub const PROTECT: &str = "Mit Passphrase schützen…";
pub const PROTECT_BUTTON: &str = "Schützen";
pub const PROTECT_HINT: &str = "Mindestens 12 Zeichen. Ohne Passphrase ist die Datei nicht mehr zu öffnen – sie lässt sich nicht zurücksetzen.";
pub const UNPROTECT: &str = "Schutz entfernen…";
pub const UNPROTECT_BUTTON: &str = "Schutz entfernen";
pub const UNPROTECT_QUESTION: &str = "Die Datei wird danach unverschlüsselt gespeichert.";
pub const PROTECTED: &str = "Die Datei ist jetzt mit Passphrase geschützt.";
pub const UNPROTECTED: &str = "Die Datei ist jetzt unverschlüsselt.";
pub const PROTECTION_UNCHANGED: &str = "Schutz nicht geändert.";
pub const PASSPHRASES_DIFFER: &str = "Die beiden Eingaben stimmen nicht überein.";
pub const PASSPHRASE_TOO_SHORT: &str = "Die Passphrase braucht mindestens 12 Zeichen.";
pub const PASSPHRASE_QUESTION: &str = "Die Datei ist verschlüsselt. Passphrase:";
pub const UNSAVED_TITLE: &str = "Ungespeicherte Änderungen";
pub const UNSAVED_QUESTION: &str = "Die Änderungen sind noch nicht gespeichert.";
pub const DISCARD: &str = "Verwerfen";
pub const SAVED: &str = "Gespeichert.";
pub const ALREADY_OPEN: &str = "Diese Datei ist bereits geöffnet.";
pub const RECOVERY_FAILED: &str = "Wiederherstellungskopie nicht möglich:";
pub const RECOVERY_SAVED: &str = "Wiederherstellungskopie";
pub const RECOVERY_SAVED_HINT: &str =
    "Automatisch gesichert. Nach einem Absturz steht sie beim nächsten Start auf der Startseite.";
pub const RECOVERY_WRITER_STOPPED: &str = "Das Schreiben wurde unterbrochen.";
pub const DELETE_COPY: &str = "Kopie löschen";
pub const DELETE_COPY_QUESTION: &str =
    "Die Wiederherstellungskopie wird endgültig gelöscht. Ihr Inhalt ist dann verloren.";
pub const PREVIOUS_MONTH: &str = "Vormonat";
pub const NEXT_MONTH: &str = "Folgemonat";
pub const RECOVERED_OPENED: &str =
    "Wiederhergestellt. Bitte mit „Speichern unter…“ an ihren Platz speichern.";
pub const NOT_SAVED: &str = "• ungespeichert";

pub const VS_NFD: &str = "VS – NUR FÜR DEN DIENSTGEBRAUCH";

pub const PICK_DATE: &str = "Im Kalender wählen";
pub const TODAY: &str = "Heute";
pub const CLEAR: &str = "Leeren";
pub const TIME_HINT: &str = "HH:MM";
pub const ADD_TIME: &str = "Uhrzeit hinzufügen";
pub const MILESTONE: &str = "Meilenstein";
pub const MILESTONE_HINT: &str =
    "Ein Datum, das zählt: erreicht, wenn erledigt. Nur oberste Ebene, ohne Unteraufgaben.";
pub const UPGRADED: &str =
    "Datei auf das neue Format umgestellt: Meilensteine sind jetzt Aufgaben. Bitte speichern.";

/// Name of a task status (board column).
pub const fn task_status(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "Offen",
        TaskStatus::InProgress => "In Arbeit",
        TaskStatus::Blocked => "Blockiert",
        TaskStatus::Review => "Prüfung",
        TaskStatus::Done => "Erledigt",
    }
}

/// Name of a priority.
pub const fn priority(priority: Priority) -> &'static str {
    match priority {
        Priority::Low => "Niedrig",
        Priority::Normal => "Normal",
        Priority::High => "Hoch",
        Priority::Critical => "Kritisch",
    }
}

/// Name of a project status.
pub const fn project_status(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::Draft => "Entwurf",
        ProjectStatus::Active => "Aktiv",
        ProjectStatus::OnHold => "Pausiert",
        ProjectStatus::Done => "Abgeschlossen",
        ProjectStatus::Archived => "Archiviert",
    }
}

/// Name of the state of a milestone.
pub const fn milestone_state(state: MilestoneState) -> &'static str {
    match state {
        MilestoneState::Open => "offen",
        MilestoneState::Reached => "erreicht",
        MilestoneState::Missed => "verpasst",
    }
}

/// "in 3 Tagen", "heute", "seit 2 Tagen überfällig".
pub fn days_left(days: i64) -> String {
    match days {
        0 => "heute fällig".to_owned(),
        1 => "morgen fällig".to_owned(),
        -1 => "seit 1 Tag überfällig".to_owned(),
        d if d > 1 => format!("in {d} Tagen"),
        d => format!("seit {} Tagen überfällig", d.unsigned_abs()),
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
        (Some(time), 0, true) => format!("seit {} Uhr überfällig", crate::dates::format_time(time)),
        (Some(time), 0, false) => format!("heute bis {} Uhr", crate::dates::format_time(time)),
        _ => days_left(days),
    };
    (text, past)
}

/// A refused command, for people.
pub fn command_error(error: &CommandError) -> String {
    match error {
        CommandError::BadParam { param, reason } => {
            format!("Eingabe nicht gültig ({param}): {reason}")
        }
        CommandError::Disabled { .. } => "Das ist gerade nicht möglich.".to_owned(),
        CommandError::NotFound { .. } => "Das gibt es nicht mehr.".to_owned(),
        CommandError::Invalid { violations } => match violations.first() {
            Some(v) => format!("Nicht erlaubt: {}", v.message),
            None => "Nicht erlaubt.".to_owned(),
        },
        CommandError::Conflict { reason } => format!("Konflikt: {reason}"),
        CommandError::UnknownCommand { command } => format!("Unbekannter Befehl {command}"),
    }
}

/// A file that could not be read or written, for people.
pub fn store_error(error: &StoreError) -> String {
    match error {
        StoreError::NotPlnwrk => "Das ist keine plnwrk-Datei.".to_owned(),
        StoreError::NewerFormat { .. } => {
            "Die Datei stammt aus einer neueren plnwrk-Version. Bitte plnwrk aktualisieren."
                .to_owned()
        }
        StoreError::UnsupportedFormat { found } => {
            format!("Dateiformat {found} wird nicht unterstützt.")
        }
        StoreError::Corrupt { reason } => format!("Die Datei ist beschädigt: {reason}"),
        StoreError::TooLarge { .. } => "Die Datei ist zu groß.".to_owned(),
        StoreError::PassphraseRequired => "Die Datei ist verschlüsselt.".to_owned(),
        StoreError::WrongPassphrase => "Die Passphrase ist falsch.".to_owned(),
        StoreError::ClassifiedUnencrypted => {
            "VS-NfD-Inhalte dürfen nur verschlüsselt gespeichert werden.".to_owned()
        }
        StoreError::ChainBroken { seq } => {
            format!("Das Änderungsprotokoll wurde bei Änderung {seq} verändert.")
        }
        StoreError::SnapshotMismatch => {
            "Datenstand und Änderungsprotokoll passen nicht zusammen.".to_owned()
        }
        StoreError::Invalid { violations } => {
            format!("Die Daten verletzen {} Regel(n).", violations.len())
        }
        StoreError::Encryption { reason } => format!("Verschlüsselung fehlgeschlagen: {reason}"),
        StoreError::Io { reason } => format!("Dateizugriff fehlgeschlagen: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
