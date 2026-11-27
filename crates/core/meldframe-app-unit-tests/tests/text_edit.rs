//! The text being edited (audit finding T12, decision 43 clause Q6): begin, commit and cancel as state of the app layer,
//! driven without a front end. The rules are the ones `apps/web` kept in TypeScript before they moved (`Editor.svelte`
//! and `textedit.ts`); each test names the rule.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, App, Connection, Effect, Event, TextEdit, TextEditOp};
use meldframe_protocol::{Command, NodePatch};
use serde_json::json;

fn doc_with_text(text: &str) -> Vec<meldframe_protocol::Node> {
    let mut nodes = doc();
    nodes[2] = serde_json::from_value(
        json!({"id": "t1", "type": "text", "parentId": "root", "text": text}),
    )
    .unwrap();
    nodes.push(node("t2", "text", Some("root")));
    nodes
}

fn live(text: &str) -> Loopback {
    let mut app = App::new("user:me", "c0", 1, doc_with_text(text), 0).unwrap();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.take_events();
    Loopback {
        app,
        seq: 0,
        events: vec![],
    }
}

fn op(op: TextEditOp) -> Action {
    Action::TextEdit { op }
}
fn begin(id: &str) -> Action {
    op(TextEditOp::Begin { id: id.into() })
}
fn commit(text: &str) -> Action {
    commit_of("t1", text)
}
fn commit_of(id: &str, text: &str) -> Action {
    op(TextEditOp::Commit {
        id: id.into(),
        text: text.into(),
    })
}
fn edit_of(l: &Loopback) -> Option<(String, String)> {
    l.app
        .state()
        .text_edit
        .clone()
        .map(|e: TextEdit| (e.id, e.text))
}
fn changed(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| **e == Event::TextEditChanged {})
        .count()
}
fn remote(l: &mut Loopback, command: Command) {
    l.remote("agent:claude", "k", 1, command);
}
fn set_text(id: &str, text: &str) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            text: Some(text.into()),
            ..Default::default()
        },
    }
}

#[test]
fn begin_on_a_text_node_selects_it_and_holds_its_text() {
    let mut l = live("hello");
    let fx = l.send(begin("t1"));
    assert!(fx.is_empty(), "view state: no op, no effect");
    assert_eq!(edit_of(&l), Some(("t1".into(), "hello".into())));
    assert_eq!(l.app.state().selection, ids(&["t1"]));
    assert!(!l.app.state().history.can_undo);
    assert_eq!(changed(&l.events), 1);
}

#[test]
fn begin_on_another_kind_or_an_unknown_node_changes_nothing_and_says_nothing() {
    let mut l = live("hello");
    l.send(Action::Select { ids: ids(&["t1"]) });
    l.events.clear();
    for id in ["r1", "root", "ghost"] {
        let fx = l.send(begin(id));
        assert!(fx.is_empty());
        assert_eq!(edit_of(&l), None);
        assert_eq!(l.app.state().selection, ids(&["t1"]), "the selection stays");
    }
    // A refusal is a notice on the page, and the page showed none for a double click on a frame.
    assert!(l.events.is_empty(), "{:?}", l.events);
}

#[test]
fn the_text_is_held_exactly_as_the_file_has_it() {
    let text = "a\u{200B}b\nline two\t\u{202E}x  y ";
    let mut l = live(text);
    l.send(begin("t1"));
    assert_eq!(edit_of(&l).unwrap().1, text, "a field shows what is stored");
}

#[test]
fn enter_begins_on_exactly_one_selected_text_node() {
    let mut l = live("hello");
    for selection in [ids(&[]), ids(&["r1"]), ids(&["t1", "t2"]), ids(&["root"])] {
        l.send(Action::Select { ids: selection });
        l.events.clear();
        let fx = l.send(op(TextEditOp::BeginSelected {}));
        assert!(fx.is_empty());
        assert_eq!(edit_of(&l), None);
        assert!(
            l.events.is_empty(),
            "a key that starts no edit says nothing"
        );
    }
    l.send(Action::Select { ids: ids(&["t1"]) });
    l.events.clear();
    l.send(op(TextEditOp::BeginSelected {}));
    assert_eq!(edit_of(&l), Some(("t1".into(), "hello".into())));
    assert_eq!(changed(&l.events), 1);
}

#[test]
fn commit_with_a_changed_text_is_one_op_and_one_undo_step() {
    let mut l = live("hello");
    l.send(begin("t1"));
    l.events.clear();
    let fx = l.send(commit("hello world"));
    assert_eq!(edit_of(&l), None, "the edit is closed");
    assert_eq!(get(&l.app, "t1").text, "hello world");
    let ops: Vec<_> = sent(&fx).iter().map(|c| json!(c)).collect();
    assert_eq!(ops, vec![json!(set_text("t1", "hello world"))]);
    assert_eq!(l.app.state().history.undo_steps, 1);
    assert_eq!(changed(&l.events), 1);
    l.send(Action::Undo {});
    assert_eq!(get(&l.app, "t1").text, "hello", "one undo takes it back");
    assert!(!l.app.state().history.can_undo);
}

#[test]
fn commit_with_an_unchanged_text_makes_no_op_and_no_undo_step() {
    let mut l = live("hello");
    l.send(begin("t1"));
    let fx = l.send(commit("hello"));
    assert!(sent(&fx).is_empty());
    assert_eq!(edit_of(&l), None);
    assert!(!l.app.state().history.can_undo);
    assert_eq!(l.app.state().pending.len(), 0);
}

#[test]
fn cancel_leaves_the_file_untouched_and_a_later_commit_does_nothing() {
    let mut l = live("hello");
    l.send(begin("t1"));
    let fx = l.send(op(TextEditOp::Cancel {}));
    assert!(fx.is_empty());
    assert_eq!(edit_of(&l), None);
    assert_eq!(get(&l.app, "t1").text, "hello");
    // The blur that follows an Escape reports the text once more: the edit is closed, so it is nothing.
    let fx = l.send(commit("typed then escaped"));
    assert!(sent(&fx).is_empty());
    assert_eq!(get(&l.app, "t1").text, "hello");
    assert!(!l.app.state().history.can_undo);
}

#[test]
fn commit_and_cancel_with_no_edit_open_do_nothing_and_raise_nothing() {
    let mut l = live("hello");
    l.events.clear();
    l.send(commit("x"));
    l.send(op(TextEditOp::Cancel {}));
    assert!(l.events.is_empty(), "{:?}", l.events);
    assert_eq!(get(&l.app, "t1").text, "hello");
}

#[test]
fn deleting_the_edited_node_closes_the_edit_with_nothing_to_commit() {
    let mut l = live("hello");
    l.send(begin("t1"));
    l.events.clear();
    remote(&mut l, Command::RemoveNode { id: "t1".into() });
    assert_eq!(edit_of(&l), None);
    assert_eq!(changed(&l.events), 1);
    let fx = l.send(commit("typed meanwhile"));
    assert!(
        sent(&fx).is_empty(),
        "nothing is written for a node that is gone"
    );
    assert!(refused(&mut l).is_empty());
}

#[test]
fn the_persons_own_delete_closes_the_edit_too() {
    let mut l = live("hello");
    l.send(begin("t1"));
    l.send(Action::Delete { ids: ids(&["t1"]) });
    assert_eq!(edit_of(&l), None);
}

#[test]
fn a_change_of_the_text_by_another_actor_leaves_the_edit_open_on_its_first_text() {
    // What the page did: the field stays, the text it began with is kept, and a commit compares with that one.
    let mut l = live("hello");
    l.send(begin("t1"));
    remote(&mut l, set_text("t1", "from the agent"));
    assert_eq!(edit_of(&l), Some(("t1".into(), "hello".into())));
    let fx = l.send(commit("hello"));
    assert!(
        sent(&fx).is_empty(),
        "equal to the text it began with: no op"
    );
    assert_eq!(
        get(&l.app, "t1").text,
        "from the agent",
        "their change stays"
    );

    l.send(begin("t1"));
    assert_eq!(
        edit_of(&l).unwrap().1,
        "from the agent",
        "a new edit begins with what the node holds"
    );
    remote(&mut l, set_text("t1", "again"));
    let fx = l.send(commit("mine"));
    assert_eq!(
        sent(&fx).len(),
        1,
        "a different text is written over theirs"
    );
    assert_eq!(get(&l.app, "t1").text, "mine");
}

#[test]
fn selecting_something_else_does_not_end_an_edit_but_beginning_another_replaces_it() {
    // The field commits when it loses focus (that is a Commit before the new selection); a selection alone changes nothing.
    let mut l = live("hello");
    l.send(begin("t1"));
    l.send(Action::Select { ids: ids(&["r1"]) });
    assert_eq!(edit_of(&l), Some(("t1".into(), "hello".into())));
    l.send(begin("t2"));
    assert_eq!(edit_of(&l), Some(("t2".into(), String::new())));
    assert_eq!(l.app.state().selection, ids(&["t2"]));
    assert_eq!(
        get(&l.app, "t1").text,
        "hello",
        "nothing was committed for t1"
    );
    assert!(!l.app.state().history.can_undo);
}

#[test]
fn a_commit_that_names_another_node_than_the_open_edit_does_nothing() {
    // A field that is being replaced can report late; its text is not the other node's.
    let mut l = live("hello");
    l.send(begin("t1"));
    l.send(begin("t2"));
    l.events.clear();
    let fx = l.send(commit_of("t1", "late draft of the first field"));
    assert!(sent(&fx).is_empty());
    assert!(l.events.is_empty(), "{:?}", l.events);
    assert_eq!(edit_of(&l), Some(("t2".into(), String::new())));
    assert_eq!(get(&l.app, "t2").text, "");
}

#[test]
fn a_commit_before_the_next_selection_keeps_each_text() {
    let mut l = live("hello");
    l.send(begin("t1"));
    l.send(commit("one"));
    l.send(begin("t2"));
    l.send(commit_of("t2", "two"));
    assert_eq!(get(&l.app, "t1").text, "one");
    assert_eq!(get(&l.app, "t2").text, "two");
    assert_eq!(l.app.state().history.undo_steps, 2);
}

#[test]
fn a_text_the_file_refuses_is_refused_and_the_edit_closes_all_the_same() {
    let mut l = live("hello");
    l.send(begin("t1"));
    l.events.clear();
    let long = "x".repeat(20_001);
    let fx = l.send(commit(&long));
    assert!(sent(&fx).is_empty());
    assert_eq!(edit_of(&l), None);
    assert_eq!(get(&l.app, "t1").text, "hello");
    assert_eq!(refused(&mut l), vec!["limit".to_string()]);
}

#[test]
fn a_halted_app_closes_the_edit_and_drops_the_text() {
    let mut l = live("hello");
    l.send(begin("t1"));
    l.remote("agent:a", "k", 1, set_text("ghost", "x"));
    assert_eq!(l.app.state().connection, Connection::Halted);
    assert!(
        edit_of(&l).is_some(),
        "the edit stays until the field reports"
    );
    l.events.clear();
    let fx = l.send(commit("typed"));
    assert!(fx.iter().all(|e| !matches!(e, Effect::SendOps { .. })));
    assert_eq!(edit_of(&l), None);
    assert_eq!(changed(&l.events), 1);
    assert_eq!(get(&l.app, "t1").text, "hello");
    l.send(begin("t1"));
    assert!(
        edit_of(&l).is_some(),
        "an edit can begin and cancel while halted"
    );
    l.send(op(TextEditOp::Cancel {}));
    assert_eq!(edit_of(&l), None);
}

#[test]
fn a_reload_without_the_node_closes_the_edit() {
    let mut l = live("hello");
    l.send(begin("t1"));
    l.send(Action::Reload {
        now_ms: 0,
        nodes: vec![node("root", "frame", None)],
        variables: Default::default(),
        seq: 5,
        applied: vec![],
    });
    assert_eq!(l.app.state().text_edit, None);
}

#[test]
fn the_wire_shape_of_the_actions_and_the_state() {
    let a: Action =
        serde_json::from_value(json!({"type":"textEdit","op":{"type":"begin","id":"t1"}})).unwrap();
    assert!(matches!(
        a,
        Action::TextEdit {
            op: TextEditOp::Begin { .. }
        }
    ));
    for j in [
        json!({"type":"textEdit","op":{"type":"beginSelected"}}),
        json!({"type":"textEdit","op":{"type":"commit","id":"t1","text":"x"}}),
        json!({"type":"textEdit","op":{"type":"cancel"}}),
    ] {
        assert!(serde_json::from_value::<Action>(j).is_ok());
    }
    let bad = json!({"type":"textEdit","op":{"type":"cancel","x":1}});
    assert!(serde_json::from_value::<Action>(bad).is_err());
    let bad = json!({"type":"textEdit","op":{"type":"commit","text":"x"}});
    assert!(serde_json::from_value::<Action>(bad).is_err());
    let mut l = live("hi");
    let state = serde_json::to_value(l.app.state()).unwrap();
    assert!(state.get("textEdit").is_none());
    l.send(begin("t1"));
    let state = serde_json::to_value(l.app.state()).unwrap();
    assert_eq!(state["textEdit"], json!({"id": "t1", "text": "hi"}));
}
