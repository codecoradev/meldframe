//! Sibling order: the effective order is `(pos, id)`. Resolving the `pos` a command wants into the
//! `pos` the node gets, against the current nodes.

use meldframe_check::{ApplyError, Limits, position};
use meldframe_order::fracindex;
use meldframe_protocol::Node;

/// Where a command wants a node among its siblings.
pub(super) enum Want<'a> {
    /// After the last sibling.
    End,
    /// At this `pos` (or right behind the sibling that holds it).
    Pos(&'a str),
    /// Right in front of (below in order) this sibling.
    Before(&'a str),
    /// Right behind (above in order) this sibling.
    After(&'a str),
}

fn within_limit(k: String, l: &Limits) -> Result<String, ApplyError> {
    if k.len() > l.max_pos {
        Err(ApplyError::PosTooLong { max: l.max_pos })
    } else {
        Ok(k)
    }
}

fn mid(lo: Option<&str>, hi: Option<&str>, l: &Limits) -> Result<String, ApplyError> {
    let k = fracindex::between(lo, hi).map_err(|e| ApplyError::InvalidPos(e.to_string()))?;
    within_limit(k, l)
}

/// The `pos` of an anchor sibling.
fn anchor_pos<'a>(nodes: &'a [Node], id: &str) -> Result<&'a str, ApplyError> {
    position(nodes, id)
        .map(|i| nodes[i].pos.as_str())
        .filter(|p| !p.is_empty())
        .ok_or_else(|| ApplyError::BadReorder(format!("anchor has no pos yet: {id}")))
}

/// The `pos` that node `exclude` (or a new node) gets under `parent`. Pure with respect to the current
/// nodes, so it is the same on the server and on every replica. One O(n) scan, no sorting (except on the
/// deprecated `Index` path). On a collision (an explicit `pos` that another sibling holds) the result lies
/// between that `pos` and the next larger sibling `pos`, that is, behind the colliding group: `pos` stays
/// unique per parent.
pub(super) fn resolve_pos(
    nodes: &[Node],
    parent: Option<&str>,
    exclude: Option<&str>,
    want: Want,
    l: &Limits,
) -> Result<String, ApplyError> {
    let sibs = || {
        nodes.iter().filter(move |n| {
            n.parent_id.as_deref() == parent && Some(n.id.as_str()) != exclude && !n.pos.is_empty()
        })
    };
    // The smallest sibling pos above `p` and the largest below it.
    let next_above = |p: &str| sibs().map(|n| n.pos.as_str()).filter(|q| *q > p).min();
    let prev_below = |p: &str| sibs().map(|n| n.pos.as_str()).filter(|q| *q < p).max();
    match want {
        Want::End => mid(sibs().map(|n| n.pos.as_str()).max(), None, l),
        Want::Pos(p) => {
            if sibs().any(|n| n.pos == p) {
                mid(Some(p), next_above(p), l)
            } else {
                Ok(p.to_string())
            }
        }
        Want::Before(x) => {
            let xp = anchor_pos(nodes, x)?;
            mid(prev_below(xp), Some(xp), l)
        }
        Want::After(x) => {
            let xp = anchor_pos(nodes, x)?;
            mid(Some(xp), next_above(xp), l)
        }
    }
}
