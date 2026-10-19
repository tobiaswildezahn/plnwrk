//! Session, commands, events, undo and queries of plnwrk (layer 1).
//!
//! The only way to change state is [`Session::run`]. Pure: no I/O. See `docs/PLAN.md` §3.3.

pub mod command;
pub mod commands;
mod error;
mod event;
pub mod fixture;
pub mod milestones;
pub mod params;
pub mod places;
pub mod query;
mod session;
pub mod sync;
mod tx;

pub use command::{CommandSpec, find, registry, search};
pub use error::CommandError;
pub use event::{Change, Event, Origin};
pub use session::{Checkpoint, Outcome, Session};
pub use tx::Tx;

#[cfg(test)]
mod session_tests;
