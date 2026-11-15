//! Sibling order: the effective order is `(pos, id)`. Resolving the `pos` a command wants into the
//! `pos` the node gets, against the current nodes.

use meldframe_check::{ApplyError, Limits, check_pos, position};
use meldframe_order::{children_map, fracindex, sib_cmp};
use meldframe_protocol::{Command, Node};
use std::cmp::Ordering;
use std::collections::HashMap;

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
    /// The deprecated index into the flat draw list.
    Index(usize),
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
        Want::Index(i) => {
            // Deprecated path: the index counts the flat draw list (pre-order walk by `(pos, id)`).
            // Insert before the first sibling whose draw index is at least `i`.
            let kids = children_map(nodes);
            let mut draw: HashMap<&str, usize> = HashMap::new();
            let mut stack: Vec<usize> = kids
                .get(&None)
                .map(|v| v.iter().rev().copied().collect())
                .unwrap_or_default();
            while let Some(k) = stack.pop() {
                draw.insert(nodes[k].id.as_str(), draw.len());
                if let Some(c) = kids.get(&Some(nodes[k].id.as_str())) {
                    stack.extend(c.iter().rev().copied());
                }
            }
            let mut ordered: Vec<&Node> = sibs().collect();
            ordered.sort_by(|a, b| sib_cmp(a, b));
            match ordered
                .iter()
                .find(|n| draw.get(n.id.as_str()).is_some_and(|&p| p >= i))
            {
                Some(n) => mid(prev_below(&n.pos), Some(&n.pos), l),
                None => mid(ordered.last().map(|n| n.pos.as_str()), None, l),
            }
        }
    }
}

/// Migrate an old file: give every sibling group that is not in order (an empty, invalid or duplicate
/// `pos`) a `pos` by list order. A group that is already valid and unique is not touched. Deterministic: the
/// server and a client that migrate the same file get the same positions. Returns how many nodes changed.
pub fn normalize_positions(nodes: &mut [Node]) -> usize {
    normalize_positions_with(nodes, &Limits::default())
}

/// [`normalize_positions`] with the limits of the caller: a `pos` longer than `l.max_pos` counts as unusable.
pub fn normalize_positions_with(nodes: &mut [Node], l: &Limits) -> usize {
    let usable = |n: &Node| !n.pos.is_empty() && check_pos(&n.pos, l).is_ok();
    let kids = children_map(nodes);
    let mut todo: Vec<(Vec<usize>, Vec<String>)> = Vec::new();
    for (_, group) in kids {
        let mut last: Option<&str> = None;
        let in_order = group.iter().all(|&i| {
            let p = nodes[i].pos.as_str();
            let ok = usable(&nodes[i]) && last.is_none_or(|q| q < p);
            last = Some(p);
            ok
        });
        if !in_order {
            // Nodes with a usable pos first (by pos, id), then the others by list order.
            let mut ordered = group.clone();
            ordered.sort_by(|&a, &b| match (usable(&nodes[a]), usable(&nodes[b])) {
                (true, true) => sib_cmp(&nodes[a], &nodes[b]),
                (false, false) => a.cmp(&b),
                (false, true) => Ordering::Less,
                (true, false) => Ordering::Greater,
            });
            let keys = fracindex::n_between(None, None, ordered.len()).expect("n keys");
            todo.push((ordered, keys));
        }
    }
    let mut changed = 0;
    for (ordered, keys) in todo {
        for (i, k) in ordered.into_iter().zip(keys) {
            nodes[i].pos = k;
            changed += 1;
        }
    }
    changed
}

/// Commands that rebalance the children of `parent`: one plain `Reorder` to "after the last sibling" for each
/// child, from the one at the front to the one at the back of the current order, so the relative order is
/// kept. Each child gets a key after the current largest one; keys therefore keep climbing across rebalances
/// and are not reset to `a1`, `a2`, ... They are ordinary commands (with inverses, safe to send as ops). They are not atomic across
/// ops: a concurrent insert by a client that has not seen the rebalance stays valid and converges, but its
/// old pos now lies behind every child that was moved (the relative order of the moved children is kept;
/// only that insert shifts).
pub fn rebalance_commands(nodes: &[Node], parent: Option<&str>) -> Vec<Command> {
    let kids = children_map(nodes);
    kids.get(&parent)
        .map(|v| {
            v.iter()
                .map(|&i| Command::Reorder {
                    id: nodes[i].id.clone(),
                    before: None,
                    after: None,
                    pos: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The new `pos` of each child of `parent` after [`rebalance_commands`], worked out once for the whole
/// group in time linear in the number of siblings apart from the sort by `(pos, id)` (O(n log n) overall): `(index into nodes,
/// new pos)` in the order the commands run. It is what the chain of plain `Reorder`s gives, because each
/// child lands behind the largest `pos` at that moment: the first behind the largest of the others, every
/// later one behind its predecessor. Fails, with nothing for the caller to undo, when a key would pass
/// `l.max_pos`.
pub fn rebalance_positions(
    nodes: &[Node],
    parent: Option<&str>,
    l: &Limits,
) -> Result<Vec<(usize, String)>, ApplyError> {
    let kids = children_map(nodes);
    let Some(group) = kids.get(&parent).filter(|g| !g.is_empty()) else {
        return Ok(Vec::new());
    };
    let mut prev: Option<String> = group[1..]
        .iter()
        .map(|&i| nodes[i].pos.as_str())
        .filter(|p| !p.is_empty())
        .max()
        .map(String::from);
    let mut out = Vec::with_capacity(group.len());
    for &i in group {
        let k = mid(prev.as_deref(), None, l)?;
        prev = Some(k.clone());
        out.push((i, k));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kid(id: &str, parent: &str, pos: &str) -> Node {
        serde_json::from_value(
            serde_json::json!({"id": id, "type": "rect", "parentId": parent, "pos": pos}),
        )
        .unwrap()
    }

    #[test]
    fn positions_for_zero_one_and_many_siblings() {
        let l = Limits::default();
        let nodes = vec![
            kid("b", "p", "a5"),
            kid("a", "p", "a1"),
            kid("z", "q", "a0"),
        ];
        assert!(
            rebalance_positions(&nodes, Some("none"), &l)
                .unwrap()
                .is_empty()
        );
        let one = rebalance_positions(&nodes, Some("q"), &l).unwrap();
        assert_eq!(one.len(), 1);
        let two = rebalance_positions(&nodes, Some("p"), &l).unwrap();
        // In sibling order (a, then b), each key behind the one before, the first behind the largest.
        assert_eq!(two.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [1, 0]);
        assert!("a5" < two[0].1.as_str() && two[0].1 < two[1].1);
    }

    #[test]
    fn a_key_past_the_limit_is_an_error() {
        let l = Limits {
            max_pos: 1,
            ..Limits::default()
        };
        let nodes = vec![kid("a", "p", "a1"), kid("b", "p", "a2")];
        assert!(matches!(
            rebalance_positions(&nodes, Some("p"), &l),
            Err(ApplyError::PosTooLong { max: 1 })
        ));
    }
}
