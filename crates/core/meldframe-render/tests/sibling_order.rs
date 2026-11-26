//! D9 of issue 313: the sibling order `(pos, id)` was written three times (`meldframe-order`, the layout core and the render
//! step). This test asks the three the same question on a table of sibling groups, in every order the nodes can be listed:
//! the order of `meldframe_order::sib_cmp`, the order the render step paints in, and the order the layout core lays a row
//! out in must be one and the same.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::StubMeasurer;
use meldframe_order::sib_cmp;
use meldframe_protocol::{Node, Variables};
use meldframe_render::{ModeChoice, resolve, resolve_laid_out};
use serde_json::json;

/// A group of siblings as `(id, pos)`; an empty `pos` is a node of an older file that has none.
type Group = &'static [(&'static str, &'static str)];

const GROUPS: &[(&str, Group)] = &[
    (
        "different positions",
        &[("c", "a0"), ("a", "a2"), ("b", "a1")],
    ),
    (
        "equal positions, the id breaks the tie",
        &[("z", "a1"), ("a", "a1"), ("m", "a1")],
    ),
    (
        "a position that is a prefix of another, and digits by text",
        &[("a", "a10"), ("b", "a1"), ("c", "a2"), ("d", "a")],
    ),
    (
        "no positions: list order is kept",
        &[("c", ""), ("a", ""), ("b", "")],
    ),
    (
        "no position first, positioned after, each tie by id",
        &[("a", "a0"), ("z", ""), ("0", "a0"), ("b", "")],
    ),
    ("one node", &[("only", "a0")]),
];

fn node(id: &str, pos: &str, parent: &str) -> Node {
    serde_json::from_value(
        json!({"id": id, "type": "rect", "parentId": parent, "pos": pos, "w": 10, "h": 10}),
    )
    .unwrap()
}

/// The group listed in the order `order` picks, under a row frame that places each child by the layout core.
fn file(group: Group, order: &[usize]) -> Vec<Node> {
    let mut nodes: Vec<Node> = vec![
        serde_json::from_value(
            json!({"id": "p", "type": "frame", "w": 1000, "h": 100, "layout": "row", "gap": 0, "padding": 0}),
        )
        .unwrap(),
    ];
    nodes.extend(order.iter().map(|&i| node(group[i].0, group[i].1, "p")));
    nodes
}

/// Every order of `0..n`.
fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn go(rest: &mut Vec<usize>, so_far: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if rest.is_empty() {
            out.push(so_far.clone());
        }
        for k in 0..rest.len() {
            let i = rest.remove(k);
            so_far.push(i);
            go(rest, so_far, out);
            so_far.pop();
            rest.insert(k, i);
        }
    }
    let mut out = Vec::new();
    go(&mut (0..n).collect(), &mut Vec::new(), &mut out);
    out
}

fn ids<'a>(nodes: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    nodes.into_iter().map(str::to_owned).collect()
}

#[test]
fn order_render_and_layout_put_siblings_in_the_same_order() {
    for (name, group) in GROUPS {
        for order in permutations(group.len()) {
            let nodes = file(group, &order);
            let listed: Vec<String> = nodes[1..].iter().map(|n| n.id.clone()).collect();

            // The reference: a stable sort of the listed children by the one comparator.
            let mut sorted: Vec<&Node> = nodes[1..].iter().collect();
            sorted.sort_by(|a, b| sib_cmp(a, b));
            let by_order = ids(sorted.iter().map(|n| n.id.as_str()));

            // The render step paints a parent, then its children in sibling order.
            let painted = resolve(&nodes);
            let by_render = ids(painted.iter().skip(1).map(|b| b.id.as_str()));

            // The layout core places the children of a row left to right in sibling order.
            let mut placed = resolve_laid_out(
                &nodes,
                &Variables::default(),
                &ModeChoice::default(),
                &StubMeasurer,
            );
            placed.retain(|b| b.id != "p");
            placed.sort_by(|a, b| a.x.total_cmp(&b.x));
            let by_layout = ids(placed.iter().map(|b| b.id.as_str()));

            assert_eq!(by_render, by_order, "{name}, listed {listed:?}: render");
            assert_eq!(by_layout, by_order, "{name}, listed {listed:?}: layout");
        }
    }
}
