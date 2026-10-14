//! A transaction: the only handle through which a command writes. It records every write as an
//! [`Event`] and can roll all of them back.

use jiff::Timestamp;
use jiff::civil::Date;
use plnwrk_core::{Clock, Entity, EntityKey, IdSource, PersonId, Workspace};
use ulid::Ulid;

use crate::{CommandError, Event};

/// Write access for one command.
pub struct Tx<'a> {
    ws: &'a mut Workspace,
    clock: &'a dyn Clock,
    ids: &'a mut dyn IdSource,
    actor: Option<PersonId>,
    events: Vec<Event>,
    created: Vec<EntityKey>,
}

impl<'a> Tx<'a> {
    pub(crate) fn new(
        ws: &'a mut Workspace,
        clock: &'a dyn Clock,
        ids: &'a mut dyn IdSource,
        actor: Option<PersonId>,
    ) -> Self {
        Self {
            ws,
            clock,
            ids,
            actor,
            events: Vec::new(),
            created: Vec::new(),
        }
    }

    /// The workspace as it is now, including this transaction's writes.
    pub fn ws(&self) -> &Workspace {
        self.ws
    }

    /// Stores an entity (create or replace). Writing an unchanged value records nothing.
    pub fn put(&mut self, entity: Entity) {
        let key = entity.key();
        let before = self.ws.get(key);
        if before.as_ref() == Some(&entity) {
            return;
        }
        if before.is_none() {
            self.created.push(key);
        }
        self.ws.put(entity.clone());
        self.events.push(Event::Put {
            before: before.map(Box::new),
            after: Box::new(entity),
        });
    }

    /// Removes an entity and returns it.
    pub fn remove(&mut self, key: EntityKey) -> Result<Entity, CommandError> {
        let before = self
            .ws
            .remove(key)
            .ok_or(CommandError::NotFound { entity: key })?;
        self.created.retain(|k| *k != key);
        self.events.push(Event::Remove {
            before: Box::new(before.clone()),
        });
        Ok(before)
    }

    /// The current instant.
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// The current day.
    pub fn today(&self) -> Date {
        self.clock.today()
    }

    /// The person running the command, if known.
    pub fn actor(&self) -> Option<PersonId> {
        self.actor
    }

    /// A new, unused id.
    pub fn new_id<T: From<Ulid>>(&mut self) -> T {
        T::from(self.ids.next_ulid())
    }

    pub(crate) fn finish(self) -> (Vec<Event>, Vec<EntityKey>) {
        (self.events, self.created)
    }

    /// Undoes every write of this transaction, newest first.
    pub(crate) fn rollback(self) {
        for event in self.events.iter().rev() {
            match event.inverse() {
                Event::Put { after, .. } => {
                    self.ws.put(*after);
                }
                Event::Remove { before } => {
                    self.ws.remove(before.key());
                }
            }
        }
    }
}
