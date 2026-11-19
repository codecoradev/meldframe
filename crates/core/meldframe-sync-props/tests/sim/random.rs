//! Random edits, made from what a replica sees.

use meldframe_order::{fracindex, sib_cmp};
use meldframe_protocol::{Command, Node, NodeKind, NodePatch};

/// Small deterministic generator, so a failing seed can be replayed.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

pub fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

pub fn initial() -> Vec<Node> {
    vec![
        node("root", "frame", None),
        node("f1", "frame", Some("root")),
        node("f2", "frame", Some("root")),
        node("s1", "rect", Some("f1")),
        node("s2", "rect", Some("f1")),
        node("s3", "rect", Some("f2")),
        node("s4", "text", Some("root")),
    ]
}

/// A random `pos` between two neighbouring siblings (or at an edge), as the replica sees them.
fn rand_pos(
    nodes: &[Node],
    parent: Option<&str>,
    skip: Option<&str>,
    r: &mut Rng,
) -> Option<String> {
    let mut sibs: Vec<&Node> = nodes
        .iter()
        .filter(|n| n.parent_id.as_deref() == parent && Some(n.id.as_str()) != skip)
        .collect();
    sibs.sort_by(|a, b| sib_cmp(a, b));
    let g = r.below(sibs.len() + 1);
    let a = (g > 0).then(|| sibs[g - 1].pos.as_str());
    let b = sibs.get(g).map(|n| n.pos.as_str());
    fracindex::between(a, b).ok()
}

fn rand_patch(r: &mut Rng) -> NodePatch {
    let mut p = NodePatch::default();
    match r.below(7) {
        0 => p.x = Some(r.below(500) as f64),
        1 => p.fill = Some(format!("#{:06x}", r.below(0xffffff)).into()),
        2 => p.text = Some(format!("t{}", r.below(99))),
        3 => {
            p.x = Some(r.below(500) as f64);
            p.y = Some(r.below(500) as f64);
        }
        4 => p.font_weight = Some(100 * (1 + r.below(9) as u16)),
        5 => p.fill2 = Some(format!("#{:06x}", r.below(0xffffff))),
        _ => p.w = Some(r.below(300) as f64),
    }
    p
}

/// One random command for replica `i`, leaning on shared nodes so that actors collide.
pub fn rand_edit(nodes: &[Node], i: usize, next_id: &mut u32, r: &mut Rng) -> Command {
    let frames: Vec<&Node> = nodes.iter().filter(|n| n.kind == NodeKind::Frame).collect();
    let pick_parent = |r: &mut Rng| match frames.len() {
        0 => None,
        n if r.below(5) > 0 => Some(frames[r.below(n)].id.clone()),
        _ => None,
    };
    match r.below(12) {
        0..=5 if !nodes.is_empty() => {
            let n = if r.below(3) > 0 {
                &nodes[r.below(nodes.len().min(7))]
            } else {
                &nodes[r.below(nodes.len())]
            };
            Command::UpdateNode {
                id: n.id.clone(),
                patch: rand_patch(r),
            }
        }
        6..=8 => {
            *next_id += 1;
            let parent = pick_parent(r);
            let kind = if r.below(3) == 0 { "frame" } else { "rect" };
            let mut nn = node(&format!("u{i}n{next_id}"), kind, parent.as_deref());
            let mut index = None;
            match r.below(4) {
                0 => index = Some(r.below(nodes.len() + 1)),
                1 | 2 => nn.pos = rand_pos(nodes, parent.as_deref(), None, r).unwrap_or_default(),
                _ => {}
            }
            Command::AddNode { node: nn, index }
        }
        9 if !nodes.is_empty() => Command::RemoveNode {
            id: nodes[r.below(nodes.len())].id.clone(),
        },
        _ if nodes.len() > 1 => {
            let n = &nodes[r.below(nodes.len())];
            if r.below(4) == 0 {
                let sibs: Vec<&Node> = nodes
                    .iter()
                    .filter(|m| m.parent_id == n.parent_id && m.id != n.id)
                    .collect();
                let t = (!sibs.is_empty()).then(|| sibs[r.below(sibs.len())].id.clone());
                let pos = if r.below(3) == 0 {
                    rand_pos(nodes, n.parent_id.as_deref(), Some(&n.id), r)
                } else {
                    None
                };
                let (before, after) = match (r.below(3), t) {
                    (0, Some(t)) => (Some(t), None),
                    (1, Some(t)) => (None, Some(t)),
                    _ => (None, None),
                };
                let (before, after) = if pos.is_some() {
                    (None, None)
                } else {
                    (before, after)
                };
                return Command::Reorder {
                    id: n.id.clone(),
                    before,
                    after,
                    pos,
                };
            }
            let parent = pick_parent(r);
            let pos = if r.below(2) == 0 {
                rand_pos(nodes, parent.as_deref(), Some(&n.id), r)
            } else {
                None
            };
            Command::Reparent {
                id: n.id.clone(),
                parent_id: parent,
                pos,
            }
        }
        _ => Command::UpdateNode {
            id: "root".into(),
            patch: NodePatch {
                x: Some(1.0),
                ..Default::default()
            },
        },
    }
}
