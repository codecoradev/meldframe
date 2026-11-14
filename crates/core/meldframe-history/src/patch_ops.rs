//! Helpers over `NodePatch`: merge, drop fields, and compact a command list.

use crate::patch_fields::patch_fields;
use meldframe_protocol::{Command, NodePatch};

/// Copy into `into` every field that `later` sets; `later` wins.
pub(crate) fn merge_patch(into: &mut NodePatch, later: &NodePatch) {
    if later.image.is_some() {
        into.image = later.image.clone();
    }
    macro_rules! merge {
        ($($f:ident),*) => { $( if later.$f.is_some() { into.$f = later.$f.clone(); } )* };
    }
    patch_fields!(merge);
}

/// Remove from `q` every field that `p` sets.
pub(crate) fn drop_fields(q: &mut NodePatch, p: &NodePatch) {
    if p.image.is_some() {
        q.image = None;
    }
    macro_rules! drop_set {
        ($($f:ident),*) => { $( if p.$f.is_some() { q.$f = None; } )* };
    }
    patch_fields!(drop_set);
}

/// A patch that sets nothing.
pub(crate) fn patch_is_empty(p: &NodePatch) -> bool {
    macro_rules! none {
        ($($f:ident),*) => { true $( && p.$f.is_none() )* };
    }
    p.image.is_none() && patch_fields!(none)
}

/// Join runs of `UpdateNode` on the same id into one; a later value for a field wins. Other commands
/// keep their place, so the order they apply in does not change.
pub(crate) fn compact(list: Vec<Command>) -> Vec<Command> {
    let mut out: Vec<Command> = Vec::with_capacity(list.len());
    for c in list {
        if let (
            Some(Command::UpdateNode {
                id: last,
                patch: into,
            }),
            Command::UpdateNode { id, patch },
        ) = (out.last_mut(), &c)
            && last == id
        {
            merge_patch(into, patch);
            continue;
        }
        out.push(c);
    }
    out
}
