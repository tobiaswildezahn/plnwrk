//! Rebuilds the workspace from the change log. The stored state must equal the result, so the
//! log really is the history of exactly this data.

use plnwrk_core::Workspace;
use plnwrk_engine::Change;

use crate::StoreError;

/// Applies every change, in order, to an empty workspace.
pub fn replay(log: &[Change]) -> Result<Workspace, StoreError> {
    let mut ws = Workspace::default();
    for change in log {
        for event in &change.events {
            event.apply(&mut ws).map_err(|_| {
                StoreError::corrupt(format!(
                    "change {} does not fit the data before it",
                    change.seq
                ))
            })?;
        }
    }
    Ok(ws)
}
