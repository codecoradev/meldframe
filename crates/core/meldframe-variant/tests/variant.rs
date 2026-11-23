//! E5.4: a variant of a frame is a copy of the frame and its whole subtree, with new ids, placed beside the original
//! under the same parent. The commands apply in order on the file as it is, and the original is untouched.

use meldframe_check::Limits;
use meldframe_core::apply_command;
use meldframe_protocol::{Command, Node, Variables};
use meldframe_variant::{GAP, VariantError, variant_of};
use serde_json::json;

fn node(v: serde_json::Value) -> Node {
    serde_json::from_value(v).unwrap()
}

/// A page frame with a hero frame (a title and a button with a label) and a footer.
fn file() -> Vec<Node> {
    let mut nodes = Vec::new();
    for v in [
        json!({"id": "page", "type": "frame", "name": "Page", "w": 2000, "h": 3000}),
        json!({"id": "hero", "type": "frame", "name": "Hero", "parentId": "page", "x": 10, "y": 20, "w": 400, "h": 300, "layout": "column", "gap": 8}),
        json!({"id": "title", "type": "text", "name": "Title", "parentId": "hero", "text": "Hello"}),
        json!({"id": "cta", "type": "frame", "name": "Call to action", "parentId": "hero", "w": 120, "h": 40}),
        json!({"id": "cta-label", "type": "text", "name": "Label", "parentId": "cta", "text": "Buy"}),
        json!({"id": "footer", "type": "frame", "name": "Footer", "parentId": "page", "y": 400, "w": 400, "h": 100}),
    ] {
        let cmd = Command::AddNode {
            node: node(v),
            index: None,
        };
        apply_command(&mut nodes, &cmd, &Limits::default()).unwrap();
    }
    nodes
}

fn fresh() -> impl FnMut() -> String {
    let mut n = 0;
    move || {
        n += 1;
        format!("v{n}")
    }
}

fn get<'a>(nodes: &'a [Node], id: &str) -> &'a Node {
    nodes.iter().find(|n| n.id == id).unwrap()
}

#[test]
fn a_variant_copies_the_frame_and_its_subtree_with_new_ids_beside_the_original() {
    let mut nodes = file();
    let before = nodes.clone();
    let v = variant_of(
        &nodes,
        &Variables::default(),
        "hero",
        fresh(),
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(v.root, "v1");
    assert_eq!(v.commands.len(), 4, "the hero and its three descendants");
    for c in &v.commands {
        apply_command(&mut nodes, c, &Limits::default()).unwrap();
    }
    // The original and everything else are unchanged.
    for n in &before {
        let same = |a: &Node| serde_json::to_value(a).unwrap();
        assert_eq!(same(get(&nodes, &n.id)), same(n), "{} changed", n.id);
    }
    let root = get(&nodes, "v1");
    assert_eq!(root.parent_id.as_deref(), Some("page"));
    assert_eq!(
        (root.x, root.y),
        (10.0 + 400.0 + GAP, 20.0),
        "to the right of the original"
    );
    assert_eq!((root.w, root.h, root.gap), (400.0, 300.0, 8.0));
    assert_eq!(root.name, "Hero variant");
    // The subtree has the same shape, names and content, under the new ids.
    let kids = |nodes: &[Node], p: &str| -> Vec<(String, String)> {
        let mut k: Vec<&Node> = nodes
            .iter()
            .filter(|n| n.parent_id.as_deref() == Some(p))
            .collect();
        k.sort_by(|a, b| a.pos.cmp(&b.pos));
        k.iter().map(|n| (n.name.clone(), n.text.clone())).collect()
    };
    assert_eq!(kids(&nodes, "v1"), kids(&nodes, "hero"));
    let cta = nodes
        .iter()
        .find(|n| n.parent_id.as_deref() == Some("v1") && n.name == "Call to action")
        .unwrap();
    assert_eq!(kids(&nodes, &cta.id), kids(&nodes, "cta"));
    assert_eq!(nodes.len(), before.len() + 4);
}

#[test]
fn a_frame_at_the_top_level_gets_a_variant_at_the_top_level() {
    let mut nodes = file();
    let v = variant_of(
        &nodes,
        &Variables::default(),
        "page",
        fresh(),
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(v.commands.len(), 6, "the page and its five descendants");
    for c in &v.commands {
        apply_command(&mut nodes, c, &Limits::default()).unwrap();
    }
    let root = get(&nodes, &v.root);
    assert_eq!((root.parent_id.clone(), root.x), (None, 2000.0 + GAP));
}

#[test]
fn only_a_frame_has_a_variant_and_the_node_must_exist() {
    let nodes = file();
    assert_eq!(
        variant_of(
            &nodes,
            &Variables::default(),
            "title",
            fresh(),
            &Limits::default()
        )
        .unwrap_err(),
        VariantError::NotAFrame
    );
    assert_eq!(
        variant_of(
            &nodes,
            &Variables::default(),
            "nope",
            fresh(),
            &Limits::default()
        )
        .unwrap_err(),
        VariantError::NotFound
    );
    assert_eq!(VariantError::NotAFrame.code(), "E_NOT_A_FRAME");
    assert_eq!(VariantError::NotFound.code(), "E_NODE_NOT_FOUND");
}

#[test]
fn an_id_the_file_already_has_is_never_used() {
    let nodes = file();
    // The id source offers ids of the file, and ids the variant already took; both are skipped.
    let mut offered = vec!["hero", "v1", "footer", "v1", "v2", "v2", "v3", "v4"].into_iter();
    let v = variant_of(
        &nodes,
        &Variables::default(),
        "hero",
        move || offered.next().unwrap().to_string(),
        &Limits::default(),
    )
    .unwrap();
    let ids: Vec<String> = v
        .commands
        .iter()
        .map(|c| match c {
            Command::AddNode { node, .. } => node.id.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(ids, ["v1", "v2", "v3", "v4"]);
}

#[test]
fn a_variant_that_would_not_fit_the_limits_is_refused_whole() {
    let nodes = file();
    // Room for two more nodes, the variant of the hero needs four: nothing is built.
    let l = Limits {
        max_nodes: nodes.len() + 2,
        ..Limits::default()
    };
    match variant_of(&nodes, &Variables::default(), "hero", fresh(), &l).unwrap_err() {
        VariantError::Apply(e) => assert_eq!(e.code(), "E_TOO_MANY_NODES"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_variant_goes_right_of_every_sibling_and_its_name_fits_the_limit() {
    let mut nodes = file();
    // A sibling placed beside the hero, where a plain offset would put the variant.
    let side = node(
        json!({"id": "side", "type": "frame", "name": "Side", "parentId": "page", "x": 450, "y": 20, "w": 100, "h": 100}),
    );
    apply_command(
        &mut nodes,
        &Command::AddNode {
            node: side,
            index: None,
        },
        &Limits::default(),
    )
    .unwrap();
    let long = "N".repeat(Limits::default().max_name);
    let hero = nodes.iter_mut().find(|n| n.id == "hero").unwrap();
    hero.name = long;
    let v = variant_of(
        &nodes,
        &Variables::default(),
        "hero",
        fresh(),
        &Limits::default(),
    )
    .unwrap();
    for c in &v.commands {
        apply_command(&mut nodes, c, &Limits::default()).unwrap();
    }
    let root = get(&nodes, &v.root);
    assert_eq!(root.x, 450.0 + 100.0 + GAP);
    assert!(root.name.ends_with(" variant"));
    assert_eq!(root.name.chars().count(), Limits::default().max_name);
}

#[test]
fn a_name_limit_smaller_than_the_suffix_still_gives_a_name_that_fits() {
    // A lone frame, so only the root's name meets the small limit.
    let nodes = vec![node(
        json!({"id": "f", "type": "frame", "name": "Hero", "w": 10, "h": 10}),
    )];
    for max_name in [3, 8] {
        let l = Limits {
            max_name,
            ..Limits::default()
        };
        let v = variant_of(&nodes, &Variables::default(), "f", fresh(), &l).unwrap();
        let Command::AddNode { node, .. } = &v.commands[0] else {
            panic!()
        };
        assert!(
            node.name.chars().count() <= max_name,
            "{max_name}: {:?}",
            node.name
        );
        assert!(!node.name.is_empty());
    }
}

/// Fields bound to `{var: id}` keep their binding in the copy: the variables are part of the file the copy is
/// checked against, and the copy needs none of its own (issue 223).
#[test]
fn a_frame_bound_to_variables_keeps_its_bindings() {
    let vars: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c1", "name": "brand", "modes": [{"id": "light", "name": "Light"}]}],
        "variables": [
            {"id": "v-primary", "collection": "c1", "name": "color/primary", "type": "color", "values": {"light": "#112233"}},
            {"id": "v-round", "collection": "c1", "name": "radius/card", "type": "number", "values": {"light": 8}}
        ]
    }))
    .unwrap();
    let nodes = vec![
        node(
            json!({"id": "hero", "type": "frame", "name": "Hero", "w": 400, "h": 300,
            "fill": {"var": "v-primary"}, "radius": {"var": "v-round"}}),
        ),
        node(
            json!({"id": "card", "type": "rect", "name": "card", "parentId": "hero", "w": 10, "h": 10,
            "fill": {"var": "v-primary"}}),
        ),
    ];
    let v = variant_of(&nodes, &vars, "hero", fresh(), &Limits::default()).unwrap();
    assert_eq!(v.commands.len(), 2);
    let added: Vec<&Node> = v
        .commands
        .iter()
        .map(|c| match c {
            Command::AddNode { node, .. } => node,
            other => panic!("not an addNode: {other:?}"),
        })
        .collect();
    assert_eq!(added[0].fill, nodes[0].fill, "the frame's fill binding");
    assert_eq!(
        added[0].radius, nodes[0].radius,
        "the frame's radius binding"
    );
    assert_eq!(added[1].fill, nodes[1].fill, "the child's fill binding");
}

/// A binding to a variable the file does not hold is still refused: the check is the file's own, not looser.
#[test]
fn a_binding_to_a_variable_the_file_lacks_is_still_refused() {
    let nodes = vec![node(
        json!({"id": "hero", "type": "frame", "name": "Hero", "w": 400, "h": 300, "fill": {"var": "gone"}}),
    )];
    let err = variant_of(
        &nodes,
        &Variables::default(),
        "hero",
        fresh(),
        &Limits::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "E_VARIABLE_NOT_FOUND");
}
