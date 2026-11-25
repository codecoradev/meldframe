//! The checkpoints of the file in the app layer (E5.3, ADR-0023), the parts that read no `App`: what makes a row of the
//! server fit to show (the second check), the list as the server last said it, how a row announced by a frame joins it,
//! and what a revert or a restore is waiting for. The rules themselves (the label, the shape of a row, the roll) are
//! `meldframe-checkpoints`, called here and not written again.

use meldframe_app_model::{Checkpoint, CheckpointKind, CheckpointRow};
use meldframe_checkpoints::{self as rules, Kind, Label, NewCheckpoint};
use meldframe_protocol::Node;

fn kind_of(kind: CheckpointKind) -> Kind {
    match kind {
        CheckpointKind::RunStart => Kind::RunStart,
        CheckpointKind::BeforeRevert => Kind::BeforeRevert,
        CheckpointKind::BeforeRestore => Kind::BeforeRestore,
        CheckpointKind::Manual => Kind::Manual,
    }
}

/// The label of a row as it is shown: the person's text made safe by `meldframe_text::label` (hidden and control
/// characters gone, white space folded, cut at the server's limit), then held to the rule of a label. Nothing left is no
/// label. The server refuses a text that needs this; the app layer cleans what it is given, once, where it enters.
fn shown(text: Option<&str>) -> Option<Label> {
    let text = meldframe_text::label(text?, rules::MAX_LABEL_CHARS);
    Label::parse(&text).ok().flatten()
}

/// The shape the server would have made the row in; `None` for one it never makes. The label is the person's text,
/// shown as [`shown`] says, and the row stays whatever is left of it, because it exists.
fn shaped(row: &CheckpointRow) -> Option<(NewCheckpoint, Option<Label>)> {
    let kind = kind_of(row.kind);
    // Only a manual row has a label, and a run start is by nobody: the server makes no other.
    if row.label.is_some() && kind != Kind::Manual {
        return None;
    }
    let label = shown(row.label.as_deref());
    let new = match (kind, row.by.as_deref(), row.about.as_deref()) {
        (Kind::RunStart, None, Some(about)) => NewCheckpoint::run_start(about, row.seq),
        (Kind::RunStart, _, _) | (_, None, _) => return None,
        (_, Some(by), about) => {
            let about = about.map(str::to_owned);
            NewCheckpoint::by_person(kind, row.seq, label.clone(), about, by)
        }
    };
    Some((new.ok()?, label))
}

/// The second check of a row from a route or a frame: `None` when it cannot be a row of the server.
pub fn clean(row: CheckpointRow) -> Option<Checkpoint> {
    let (_, label) = shaped(&row).filter(|_| row.n > 0)?;
    Some(Checkpoint {
        id: row.n.to_string(),
        n: row.n,
        seq: row.seq,
        kind: row.kind,
        label: label.map(|l| l.as_str().to_owned()),
        about: row.about,
        by: row.by,
        at_ms: row.at_ms,
    })
}

/// The list the server sent: the rows that pass the check, one for each number, newest first, at most a file's total.
pub fn load(rows: Vec<CheckpointRow>) -> Vec<Checkpoint> {
    let mut list: Vec<Checkpoint> = rows.into_iter().filter_map(clean).collect();
    list.sort_by_key(|c| std::cmp::Reverse(c.n));
    list.dedup_by_key(|c| c.n);
    list.truncate(rules::MAX_PER_FILE);
    list
}

/// The list an answer gives, with the rows held that are newer than it: a read that was out while a frame (or the answer
/// of a post) brought a row has an answer made before that row, and the row must not go with the list it replaces. A
/// held row at or below the answer's highest number that the answer does not have was rolled out, and is not kept.
pub fn merge(held: &[Checkpoint], rows: Vec<CheckpointRow>) -> Vec<Checkpoint> {
    let mut list = load(rows);
    let top = list.first().map_or(0, |c| c.n);
    let newer: Vec<Checkpoint> = held.iter().filter(|c| c.n > top).cloned().collect();
    list.splice(0..0, newer);
    list.sort_by_key(|c| std::cmp::Reverse(c.n));
    list.truncate(rules::MAX_PER_FILE);
    list
}

/// What a row announced by a frame did to the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    /// Not a row of the server, or one that is held already (a row never changes).
    Same,
    /// The row joined the list, and its number is above the highest held by more than one: a row is missing, so the
    /// list is read again.
    Gap,
    /// The row joined the list.
    Added,
}

/// A row announced by a frame (or by the answer of a create) joins `list`. A row above the highest held rolls the rows
/// its share no longer holds out, by the one function of the server (`meldframe_checkpoints::roll`), which is how the
/// roll is known here: the server does not announce it.
pub fn take(list: &mut Vec<Checkpoint>, row: CheckpointRow) -> Taken {
    let top = list.first().map_or(0, |c| c.n);
    let Some(shaped) = shaped(&row) else {
        return Taken::Same;
    };
    let Some(row) = clean(row).filter(|c| !list.iter().any(|h| h.n == c.n)) else {
        return Taken::Same;
    };
    if row.n > top {
        let held: Vec<rules::Checkpoint> = list.iter().filter_map(held).collect();
        if let Ok(rolled) = rules::roll(&held, row.n, &shaped.0) {
            list.retain(|c| !rolled.delete.contains(&c.n));
        }
    }
    let gap = row.n > top + 1;
    let at = list.iter().position(|c| c.n < row.n).unwrap_or(list.len());
    list.insert(at, row);
    list.truncate(rules::MAX_PER_FILE);
    if gap { Taken::Gap } else { Taken::Added }
}

/// A row of the list as the rules of the roll read it.
fn held(c: &Checkpoint) -> Option<rules::Checkpoint> {
    Some(rules::Checkpoint {
        n: c.n,
        seq: c.seq,
        kind: kind_of(c.kind),
        label: Label::parse_arg(c.label.as_deref()).ok()?,
        about: c.about.clone(),
        by: c.by.clone(),
        at_ms: c.at_ms,
    })
}

/// What the app waits for before it changes the file: one revert or restore at a time.
#[derive(Debug, Clone)]
pub enum Waiting {
    /// A restore asked for the file as it was at checkpoint `n` (id `id`).
    State { n: u32, id: String },
    /// A restore holds the file at the checkpoint and waits for its `beforeRestore` row, sent with `key`.
    Restore {
        key: String,
        id: String,
        target: Vec<Node>,
    },
    /// A revert waits for its `beforeRevert` row, sent with `key`.
    Revert {
        key: String,
        actor: String,
        key_prefix: String,
    },
}

/// The app's part of the checkpoints: what it waits for, whether a read of the list is out, and the key counter.
#[derive(Debug, Default)]
pub struct Checkpoints {
    pub waiting: Option<Waiting>,
    pub listing: bool,
    next_key: u64,
}

impl Checkpoints {
    /// A key no earlier request of this session used; the server remembers a key, and `prefix` is the session's.
    pub fn next_key(&mut self, prefix: &str) -> String {
        self.next_key += 1;
        format!("{prefix}-ck{}", self.next_key)
    }
}
