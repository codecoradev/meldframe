//! Helpers over `NodePatch`: merge, and compact a command list.

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
