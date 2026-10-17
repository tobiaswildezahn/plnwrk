//! The session owns the workspace and its log. [`Session::run`] is the only way to change data.

use plnwrk_core::{Clock, PersonId, Workspace};
use serde::Serialize;
use serde_json::{Value, json};

use crate::command::{CommandSpec, Runner, find};
use crate::params::Params;
use crate::{Change, CommandError, Event, Origin, Tx};

/// What a successful command did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// The log entry, or `None` if nothing changed.
    pub change: Option<u64>,
    /// Entities that did not exist before.
    pub created: Vec<plnwrk_core::EntityKey>,
}

/// One open workspace with its history.
pub struct Session {
    ws: Workspace,
    log: Vec<Change>,
    undo: Vec<u64>,
    redo: Vec<u64>,
    clock: Box<dyn Clock>,
    ids: Box<dyn plnwrk_core::IdSource>,
    actor: Option<PersonId>,
}

impl Session {
    /// An empty workspace.
    pub fn new(clock: Box<dyn Clock>, ids: Box<dyn plnwrk_core::IdSource>) -> Self {
        Self::with_workspace(Workspace::default(), Vec::new(), clock, ids)
    }

    /// A loaded workspace and its log. Undo starts empty.
    pub fn with_workspace(
        ws: Workspace,
        log: Vec<Change>,
        clock: Box<dyn Clock>,
        ids: Box<dyn plnwrk_core::IdSource>,
    ) -> Self {
        Self {
            ws,
            log,
            undo: Vec::new(),
            redo: Vec::new(),
            clock,
            ids,
            actor: None,
        }
    }

    /// The data, read-only.
    pub fn workspace(&self) -> &Workspace {
        &self.ws
    }

    /// What `command` would change at milestones – run on a copy, nothing is kept. Empty for
    /// commands that cannot touch an existing milestone, and for one that would be refused
    /// (running it says why). Phase 9d: such changes are confirmed first.
    pub fn milestone_changes(
        &mut self,
        command: &str,
        params: &Value,
    ) -> Vec<crate::milestones::MilestoneChange> {
        if !crate::milestones::TOUCHING.contains(&command) {
            return Vec::new();
        }
        // Without any milestone, only a command that makes one could change one: no copy.
        if !self.ws.tasks().any(|t| t.milestone) && params.get("milestone").is_none() {
            return Vec::new();
        }
        let Some(spec) = find(command) else {
            return Vec::new();
        };
        let Runner::Edit(edit) = spec.run else {
            return Vec::new();
        };
        if (spec.enabled)(self).is_err() {
            return Vec::new();
        }
        let Ok(checked) = Params::check(params, spec.params) else {
            return Vec::new();
        };
        let mut copy = self.ws.clone();
        {
            let mut tx = Tx::new(
                &mut copy,
                self.clock.as_ref(),
                self.ids.as_mut(),
                self.actor,
            );
            if edit(&mut tx, &checked).is_err() {
                return Vec::new();
            }
            let _ = tx.finish();
        }
        if !copy.check().is_empty() {
            return Vec::new();
        }
        crate::milestones::compare(&self.ws, &copy)
    }

    /// Every change so far, oldest first.
    pub fn log(&self) -> &[Change] {
        &self.log
    }

    /// The clock of this session.
    pub fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
    }

    /// Sets who is working (recorded with each change and as comment author).
    pub fn set_actor(&mut self, actor: Option<PersonId>) {
        self.actor = actor;
    }

    /// Brings data of an older format up to date (see `workspace.upgrade`), as the first thing
    /// after opening a file. Returns whether anything was converted; afterwards nothing can be
    /// undone past this point.
    pub fn upgrade(&mut self) -> Result<bool, CommandError> {
        if self.ws.milestones().next().is_none() {
            return Ok(false);
        }
        self.run_spec(
            &crate::commands::workspace::UPGRADE,
            &serde_json::json!({}),
            Origin::Script,
        )?;
        Ok(true)
    }

    /// The person working, if set.
    pub fn actor(&self) -> Option<PersonId> {
        self.actor
    }

    /// Direct write access for tests that simulate damaged or outside-edited data.
    #[cfg(test)]
    pub(crate) fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.ws
    }

    /// Whether `undo` would do something.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether `redo` would do something.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Runs a command by id. On error nothing changes: not the data, not the log.
    pub fn run(
        &mut self,
        command: &str,
        params: &Value,
        origin: Origin,
    ) -> Result<Outcome, CommandError> {
        let spec = find(command).ok_or_else(|| CommandError::UnknownCommand {
            command: command.chars().take(60).collect(),
        })?;
        self.run_spec(spec, params, origin)
    }

    pub(crate) fn run_spec(
        &mut self,
        spec: &CommandSpec,
        params: &Value,
        origin: Origin,
    ) -> Result<Outcome, CommandError> {
        (spec.enabled)(self).map_err(|reason| CommandError::Disabled {
            command: spec.id.to_owned(),
            reason,
        })?;
        let checked = Params::check(params, spec.params)?;
        match spec.run {
            Runner::Edit(edit) => self.edit(spec.id, edit, &checked, params, origin),
            Runner::Undo => self.undo(origin),
            Runner::Redo => self.redo(origin),
        }
    }

    fn edit(
        &mut self,
        id: &str,
        edit: crate::command::EditFn,
        params: &Params<'_>,
        raw: &Value,
        origin: Origin,
    ) -> Result<Outcome, CommandError> {
        let mut tx = Tx::new(
            &mut self.ws,
            self.clock.as_ref(),
            self.ids.as_mut(),
            self.actor,
        );
        if let Err(error) = edit(&mut tx, params) {
            tx.rollback();
            return Err(error);
        }
        let violations = tx.ws().check();
        if !violations.is_empty() {
            tx.rollback();
            return Err(CommandError::Invalid { violations });
        }
        let (events, created) = tx.finish();
        if events.is_empty() {
            return Ok(Outcome {
                change: None,
                created,
            });
        }
        let seq = self.append(id, raw.clone(), events, origin, None, None);
        if id == crate::commands::workspace::UPGRADE.id {
            // Bringing older data up to date is a barrier: nothing before it can be undone
            // (it would bring back the old form), and it cannot be undone itself.
            self.undo.clear();
        } else {
            self.undo.push(seq);
        }
        self.redo.clear();
        Ok(Outcome {
            change: Some(seq),
            created,
        })
    }

    fn append(
        &mut self,
        command: &str,
        params: Value,
        events: Vec<Event>,
        origin: Origin,
        undoes: Option<u64>,
        redoes: Option<u64>,
    ) -> u64 {
        let seq = self.log.last().map_or(1, |c| c.seq.saturating_add(1));
        self.log.push(Change {
            seq,
            at: self.clock.now(),
            origin,
            actor: self.actor,
            command: command.to_owned(),
            params,
            events,
            undoes,
            redoes,
        });
        seq
    }

    fn events_of(&self, seq: u64) -> Result<Vec<Event>, CommandError> {
        self.log
            .iter()
            .find(|c| c.seq == seq)
            .map(|c| c.events.clone())
            .ok_or_else(|| CommandError::Conflict {
                reason: format!("change {seq} is not in the log"),
            })
    }

    /// Applies `events` in order; on any failure restores the state and reports it.
    fn apply_all(&mut self, events: &[Event]) -> Result<(), CommandError> {
        for (done, event) in events.iter().enumerate() {
            if let Err(error) = event.apply(&mut self.ws) {
                for applied in events.iter().take(done).rev() {
                    // Restoring what we just wrote cannot conflict.
                    let _ = applied.inverse().apply(&mut self.ws);
                }
                return Err(error);
            }
        }
        let violations = self.ws.check();
        if !violations.is_empty() {
            for applied in events.iter().rev() {
                let _ = applied.inverse().apply(&mut self.ws);
            }
            return Err(CommandError::Invalid { violations });
        }
        Ok(())
    }

    fn undo(&mut self, origin: Origin) -> Result<Outcome, CommandError> {
        let seq = *self.undo.last().ok_or_else(|| CommandError::Disabled {
            command: "undo".to_owned(),
            reason: "nothing to undo".to_owned(),
        })?;
        let inverse: Vec<Event> = self
            .events_of(seq)?
            .iter()
            .rev()
            .map(Event::inverse)
            .collect();
        self.apply_all(&inverse)?;
        self.undo.pop();
        self.redo.push(seq);
        let change = self.append("undo", json!({}), inverse, origin, Some(seq), None);
        Ok(Outcome {
            change: Some(change),
            created: Vec::new(),
        })
    }

    fn redo(&mut self, origin: Origin) -> Result<Outcome, CommandError> {
        let seq = *self.redo.last().ok_or_else(|| CommandError::Disabled {
            command: "redo".to_owned(),
            reason: "nothing to redo".to_owned(),
        })?;
        let events = self.events_of(seq)?;
        self.apply_all(&events)?;
        self.redo.pop();
        self.undo.push(seq);
        let change = self.append("redo", json!({}), events, origin, None, Some(seq));
        Ok(Outcome {
            change: Some(change),
            created: Vec::new(),
        })
    }
}
