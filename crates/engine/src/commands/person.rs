//! People: `person.create`, `person.update`.

use plnwrk_core::{Entity, EntityKind, Person, PersonId};

use crate::command::{CommandSpec, Runner, always};
use crate::params::{self as p, ParamKind as K, ParamSpec as P, Params};
use crate::{CommandError, Tx};

/// Initials from the first letters of the first two words, upper case.
pub fn initials_of(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .flat_map(char::to_uppercase)
        .collect()
}

fn create(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let name = params.req("name", p::title)?;
    let initials = match params.opt("initials", p::label)? {
        Some(initials) => initials,
        None => initials_of(&name),
    };
    let id: PersonId = tx.new_id();
    tx.insert(Entity::Person(Person {
        id,
        name,
        email: params.opt("email", p::label)?,
        role: params.opt("role", p::label)?,
        initials,
    }))
}

fn update(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let id: PersonId = params.req("person", p::id)?;
    let mut person = tx.ws().person(id).cloned().ok_or(CommandError::NotFound {
        entity: plnwrk_core::EntityKey::Person(id),
    })?;
    if let Some(name) = params.patch_required("name", p::title)? {
        // Initials that were derived from the old name follow the new one.
        if person.initials == initials_of(&person.name) {
            person.initials = initials_of(&name);
        }
        person.name = name;
    }
    params.patch("email", p::label)?.apply_to(&mut person.email);
    params.patch("role", p::label)?.apply_to(&mut person.role);
    if let Some(initials) = params.patch_required("initials", p::label)? {
        person.initials = initials;
    }
    tx.put(Entity::Person(person));
    Ok(())
}

/// `person.create`
pub const CREATE: CommandSpec = CommandSpec {
    id: "person.create",
    summary: "Adds a person who can own projects, do tasks and write comments.",
    params: &[
        P::required("name", K::Title, "Full name"),
        P::optional("email", K::Label, "E-mail address"),
        P::optional("role", K::Label, "Role or function"),
        P::optional(
            "initials",
            K::Label,
            "Short form; default: first letters of the name",
        ),
    ],
    enabled: always,
    run: Runner::Edit(create),
};

/// `person.update`
pub const UPDATE: CommandSpec = CommandSpec {
    id: "person.update",
    summary: "Changes name, e-mail, role or initials of a person.",
    params: &[
        P::required("person", K::Id(EntityKind::Person), "The person"),
        P::optional("name", K::Title, "Full name"),
        P::clearable("email", K::Label, "E-mail address; null removes it"),
        P::clearable("role", K::Label, "Role or function; null removes it"),
        P::optional("initials", K::Label, "Short form"),
    ],
    enabled: always,
    run: Runner::Edit(update),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_from_names() {
        assert_eq!(initials_of("Tobias Wildezahn"), "TW");
        assert_eq!(initials_of("ömer"), "Ö");
        assert_eq!(initials_of("Anna Maria Schmidt"), "AM");
    }
}
