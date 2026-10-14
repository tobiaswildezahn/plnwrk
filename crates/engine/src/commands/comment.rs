//! Comments on tasks: add.

use plnwrk_core::{Comment, CommentId, Entity, EntityKind};

use crate::command::{CommandSpec, Runner, always};
use crate::params::{self as p, ParamKind as K, ParamSpec as P, Params};
use crate::{CommandError, Tx};

fn add(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let id: CommentId = tx.new_id();
    let comment = Comment {
        id,
        task: params.req("task", p::id)?,
        author: tx.actor(),
        created: tx.now(),
        body: params.req("body", p::body)?,
    };
    tx.put(Entity::Comment(comment));
    Ok(())
}

/// `comment.add`
pub const ADD: CommandSpec = CommandSpec {
    id: "comment.add",
    summary: "Adds a comment to a task, signed by the person working in the session.",
    params: &[
        P::required("task", K::Id(EntityKind::Task), "The task"),
        P::required("body", K::LongText, "Markdown"),
    ],
    enabled: always,
    run: Runner::Edit(add),
};
