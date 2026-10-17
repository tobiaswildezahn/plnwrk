//! Where a command is found in the app's ribbon (Phase 9b), in German and English, so an AI can
//! tell a person where to click ("Start › Neu"). Only commands with a button in the ribbon are
//! listed; the others are reached through fields of the views. The interface checks that this
//! table matches its ribbon (`plnwrk-ui`, test `the_engine_knows_where_each_command_lives`).

/// A place in the ribbon: tab › group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub de: &'static str,
    pub en: &'static str,
}

const NEW: Place = Place {
    de: "Start › Neu",
    en: "Home › New",
};
const UNDO: Place = Place {
    de: "Start › Rückgängig",
    en: "Home › Undo",
};
const VIEWS: Place = Place {
    de: "Ansicht › Ansichten",
    en: "View › Views",
};
const AREAS: Place = Place {
    de: "Ansicht › Bereiche",
    en: "View › Areas",
};
const STATUS: Place = Place {
    de: "Aufgabe › Status",
    en: "Task › Status",
};
const TASK: Place = Place {
    de: "Aufgabe › Aufgabe",
    en: "Task › Task",
};

/// Command id and its place.
pub const PLACES: &[(&str, Place)] = &[
    ("project.create", NEW),
    ("project.instantiate", NEW),
    ("task.create", NEW),
    ("undo", UNDO),
    ("redo", UNDO),
    ("task.reschedule", VIEWS),
    ("project.update", AREAS),
    ("project.archive", AREAS),
    ("person.create", AREAS),
    ("person.update", AREAS),
    ("task.set_status", STATUS),
    ("task.delete", TASK),
];

/// Where a command is found in the ribbon, if it has a button there.
pub fn place(command: &str) -> Option<Place> {
    PLACES
        .iter()
        .find(|(id, _)| *id == command)
        .map(|(_, place)| *place)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_place_names_a_command_of_the_register() {
        for (id, _) in PLACES {
            assert!(crate::command::find(id).is_some(), "{id}");
        }
        assert_eq!(place("task.delete"), Some(TASK));
        assert_eq!(place("nope"), None);
    }
}
