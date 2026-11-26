//! Rule U9 at the app seam, for the names a person reads (issue 319, the Rust half): `AppState` carries the display
//! string wherever it carries a name, so a front end cannot forget to clean. A golden table of the characters that
//! change how a name reads without showing, run through every field: a layer row, the properties, the export target,
//! the dialog and its findings, the Variables panel (a mode name), a checkpoint label, the names of a batch's summary and
//! the actor of a lane. A collection and a variable name cannot hold a hidden character in a file that loads (the core
//! holds them to a lowercase path), so those fields are driven through the pure functions that build them, in
//! `golden_names_parts.rs`.
//!
//! The file keeps the names as written (`App::nodes`, `App::vars`): only the state is cleaned.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, App, CheckpointKind, CheckpointRow, Panel};
use meldframe_protocol::{Command, Doc, Node, Variables};
use serde_json::json;

const T: u64 = 1_700_000_000_000;

/// A file whose node and mode names are `text`: a frame (its fill bound to the colour `bg`), a text node that the export
/// warns about, and a collection with two modes. The collection and the variables have the names the core allows.
fn file(text: &str) -> Doc {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "root", "name": text, "type": "frame", "pos": "a0", "fill": {"var": "bg"}},
        {"id": "note", "name": text, "type": "text", "parentId": "root", "pos": "a0", "text": "a\tb"},
    ]))
    .unwrap();
    let vars: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c1", "name": "theme",
            "modes": [{"id": "m1", "name": text}, {"id": "m2", "name": "plain"}]}],
        "variables": [
            {"id": "bg", "collection": "c1", "name": "color/bg", "type": "color",
             "values": {"m1": "#ffffff", "m2": {"var": "alt"}}},
            {"id": "alt", "collection": "c1", "name": "color/alt", "type": "color",
             "values": {"m1": "#000000", "m2": "#111111"}},
        ],
    }))
    .unwrap();
    Doc { nodes, vars }
}

fn opened(text: &str) -> App {
    let mut a = App::with_doc("user:me", "c0", 1, file(text), 0).expect("the file loads");
    a.dispatch(Action::Connected { now_ms: 0 });
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: true,
    });
    a.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    a
}

#[test]
fn the_file_keeps_the_names_as_written() {
    let a = opened("a\u{202E}b");
    assert_eq!(get(&a, "root").name, "a\u{202E}b");
    assert_eq!(a.vars().collections[0].modes[0].name, "a\u{202E}b");
}

#[test]
fn layers_row_name_hides_bidi_override() {
    let a = opened("Pay\u{202E}gnp.exe");
    let names: Vec<&str> = a.state().layers.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Paygnp.exe", "Paygnp.exe"]);
    let all = serde_json::to_string(a.state()).unwrap();
    assert!(!all.contains("\\u202e") && !all.contains('\u{202E}'));
}

#[test]
fn every_name_in_the_state_shows_the_display_string() {
    for c in CASES {
        let mut a = opened(c.given);
        let s = a.state().clone();
        let what = c.what;
        // A layer row and the properties.
        assert!(
            s.layers.iter().all(|r| r.name == c.name),
            "layer row: {what}"
        );
        let p = s.properties.as_ref().expect("properties");
        assert_eq!(p.name, c.name, "properties name: {what}");
        // The Variables panel: a mode name; the names the core allows come back as they are.
        let col = &s.variables.collections[0];
        assert_eq!(col.modes[0].name, c.name, "mode name: {what}");
        assert_eq!(col.modes[1].name, "plain");
        assert_eq!(col.name, "theme");
        assert_eq!(p.bindings[0].name, "color/bg");
        // The export target, the dialog and a finding about a node.
        assert_eq!(
            s.export.target.as_ref().map(|t| t.name.as_str()),
            Some(c.name),
            "export target: {what}"
        );
        a.dispatch(Action::OpenExport {});
        let d = a.state().export.dialog.clone().expect("the dialog");
        assert_eq!(d.name, c.name, "export dialog: {what}");
        let finding = d
            .warnings
            .iter()
            .find(|f| f.code == "text_outside_contract");
        let named = finding.and_then(|f| f.node_name.as_deref());
        assert_eq!(named, Some(c.name), "export finding: {what}");
        // The stored names are what the file holds.
        assert_eq!(get(&a, "root").name, c.given, "stored node name: {what}");
        assert_eq!(
            a.vars().collections[0].modes[0].name,
            c.given,
            "stored mode name: {what}"
        );
    }
}

#[test]
fn a_checkpoint_label_shows_the_display_string() {
    for c in CASES {
        let mut a = opened("x");
        let row = CheckpointRow {
            n: 1,
            seq: 0,
            kind: CheckpointKind::Manual,
            label: Some(format!("before {}", c.given)),
            about: None,
            by: Some("user:me".into()),
            at_ms: T,
        };
        a.dispatch(Action::CheckpointsLoaded {
            checkpoints: vec![row],
            can_create: true,
        });
        let list = &a.state().checkpoints;
        assert_eq!(list.len(), 1, "{}", c.what);
        assert_eq!(
            list[0].label.as_deref(),
            Some(format!("before {}", c.name).as_str()),
            "{}",
            c.what
        );
    }
}

#[test]
fn a_label_that_is_too_long_is_cut_and_one_that_is_nothing_is_left_off() {
    let mut a = opened("x");
    let rows = [
        (1, "x".repeat(300)),
        (2, "\u{200B}\u{202E}".to_string()),
        (3, "two\nlines".to_string()),
        (4, "\u{1F600}".repeat(200)),
    ]
    .into_iter()
    .map(|(n, label)| CheckpointRow {
        n,
        seq: 0,
        kind: CheckpointKind::Manual,
        label: Some(label),
        about: None,
        by: Some("user:me".into()),
        at_ms: T,
    })
    .collect();
    a.dispatch(Action::CheckpointsLoaded {
        checkpoints: rows,
        can_create: true,
    });
    let label = |n: u32| {
        let c = a.state().checkpoints.iter().find(|c| c.n == n);
        c.expect("the row stays").label.clone()
    };
    assert_eq!(label(1), Some("x".repeat(128)));
    assert_eq!(label(2), None);
    assert_eq!(label(3), Some("two lines".to_string()));
    assert_eq!(label(4), Some("\u{1F600}".repeat(128)));
}

#[test]
fn the_names_of_a_batch_summary_and_the_actor_of_a_lane_show_the_display_string() {
    for c in CASES {
        let mut l = Loopback::live();
        let node: Node =
            serde_json::from_value(json!({"id": "n1", "name": c.given, "type": "rect"})).unwrap();
        let add = Command::AddNode { node, index: None };
        let actor = format!("agent:{}", c.given);
        l.remote(&actor, "b1:0", T, add);
        let a = &l.app.state().activity;
        let names = &a.rows[0].summary.as_ref().expect("a summary").names;
        assert_eq!(names, &[c.name.to_string()], "summary: {}", c.what);
        assert_eq!(a.lanes[0].actor, c.actor, "lane actor: {}", c.what);
        assert_eq!(
            l.app.state().last_actor["n1"],
            c.actor,
            "layer actor: {}",
            c.what
        );
    }
}

#[test]
fn a_family_emoji_in_a_name_stays_whole_and_the_stored_name_is_kept() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let a = opened(&format!("Team {family}"));
    assert_eq!(a.state().layers[0].name, format!("Team {family}"));
    assert_eq!(get(&a, "root").name, format!("Team {family}"));
}

#[test]
fn a_name_that_is_all_hidden_shows_empty_in_the_row_and_the_properties() {
    // The app layer sends "" for a name with nothing left (the front end words it, "(no name)"); the stored name is kept.
    // (A name over the cap cannot be in a file that loads: the core refuses it at 256 characters.)
    for hidden in ["\u{200B}\u{202E}", "\u{7}\u{85}", "\u{2028}\t \u{00A0}"] {
        let a = opened(hidden);
        assert!(
            a.state().layers.iter().all(|r| r.name.is_empty()),
            "{hidden:?}"
        );
        assert_eq!(
            a.state().properties.as_ref().unwrap().name,
            "",
            "{hidden:?}"
        );
        assert_eq!(get(&a, "root").name, hidden);
    }
}

#[test]
fn the_shown_name_cap_is_the_longest_name_the_file_takes() {
    // MAX_LABEL is a copy of `max_name` of the limits of a file: if the limit moves, a name the file holds would be cut.
    assert_eq!(
        meldframe_text::MAX_LABEL,
        meldframe_check::Limits::default().max_name
    );
}

#[test]
fn a_name_of_256_four_byte_characters_is_shown_whole() {
    let a = opened(&"\u{1F600}".repeat(256));
    assert_eq!(a.state().layers[0].name, "\u{1F600}".repeat(256));
    assert_eq!(a.state().layers[0].name.len(), 1024);
}

#[test]
fn a_frame_from_the_server_with_a_hostile_name_reaches_the_layers_clean() {
    let mut l = Loopback::live();
    let node: Node =
        serde_json::from_value(json!({"id": "n9", "name": "Pay\u{202E}now", "type": "rect"}))
            .unwrap();
    l.remote("agent:x", "b1:0", T, Command::AddNode { node, index: None });
    let row = l.app.state().layers.iter().find(|r| r.id == "n9");
    assert_eq!(row.map(|r| r.name.as_str()), Some("Paynow"));
}
