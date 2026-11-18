//! Helpers over `NodePatch`: merge, drop fields, invert, and compact a command list.

use crate::patch_fields::patch_fields;
use meldframe_protocol::{Command, Node, NodePatch};

/// Copy into `into` every field that `later` sets; `later` wins.
pub fn merge_patch(into: &mut NodePatch, later: &NodePatch) {
    if later.image.is_some() {
        into.image = later.image.clone();
    }
    if later.gradient.is_some() {
        into.gradient = later.gradient.clone();
    }
    for (i, l) in [
        (&mut into.pad_t, &later.pad_t),
        (&mut into.pad_r, &later.pad_r),
        (&mut into.pad_b, &later.pad_b),
        (&mut into.pad_l, &later.pad_l),
    ] {
        if l.is_some() {
            *i = *l;
        }
    }
    macro_rules! merge {
        ($($f:ident),*) => { $( if later.$f.is_some() { into.$f = later.$f.clone(); } )* };
    }
    patch_fields!(merge);
}

/// Remove from `q` every field that `p` sets.
pub fn drop_fields(q: &mut NodePatch, p: &NodePatch) {
    if p.image.is_some() {
        q.image = None;
    }
    if p.gradient.is_some() {
        q.gradient = None;
    }
    for (qs, ps) in [
        (&mut q.pad_t, &p.pad_t),
        (&mut q.pad_r, &p.pad_r),
        (&mut q.pad_b, &p.pad_b),
        (&mut q.pad_l, &p.pad_l),
    ] {
        if ps.is_some() {
            *qs = None;
        }
    }
    macro_rules! drop_set {
        ($($f:ident),*) => { $( if p.$f.is_some() { q.$f = None; } )* };
    }
    patch_fields!(drop_set);
}

/// A patch that sets nothing.
pub fn patch_is_empty(p: &NodePatch) -> bool {
    macro_rules! none {
        ($($f:ident),*) => { true $( && p.$f.is_none() )* };
    }
    p.image.is_none()
        && p.gradient.is_none()
        && [p.pad_t, p.pad_r, p.pad_b, p.pad_l]
            .iter()
            .all(Option::is_none)
        && patch_fields!(none)
}

/// A patch that holds the old value of every field `p` touches.
pub fn inverse_patch(old: &Node, p: &NodePatch) -> NodePatch {
    let mut r = NodePatch::default();
    if p.image.is_some() {
        // The old value: `None` (no image) becomes the patch `null`.
        r.image = Some(old.image.clone());
    }
    if p.gradient.is_some() {
        r.gradient = Some(old.gradient.clone());
    }
    // A side that was not set becomes the patch `null`.
    for (r, p, old) in [
        (&mut r.pad_t, &p.pad_t, old.pad_t),
        (&mut r.pad_r, &p.pad_r, old.pad_r),
        (&mut r.pad_b, &p.pad_b, old.pad_b),
        (&mut r.pad_l, &p.pad_l, old.pad_l),
    ] {
        if p.is_some() {
            *r = Some(old);
        }
    }
    macro_rules! copy {
        ($($f:ident),*) => { $( if p.$f.is_some() { r.$f = Some(old.$f.clone()); } )* };
    }
    patch_fields!(copy);
    r
}

/// Join runs of `UpdateNode` on the same id into one; a later value for a field wins. Other commands
/// keep their place, so the order they apply in does not change.
pub fn compact(list: Vec<Command>) -> Vec<Command> {
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
