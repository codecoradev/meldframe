//! The scene epoch rests on `Replica::revision`: every way the file can change moves it, the ones that leave the nodes alone
//! included (a variable value, an undo, another actor's op, a reload).

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod scene_fixture;
use meldframe_app::{Action, App, Frame, VariableValue};
use meldframe_protocol::{Command, Doc, Node, NodePatch};
use scene_fixture::*;
use serde_json::json;

fn painted() -> App {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "root", "type": "frame", "pos": "a0", "w": 200, "h": 100, "fill": {"var": "bg"}},
    ]))
    .unwrap();
    let vars = serde_json::from_value(json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "light", "name": "light"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color", "values": {"light": "#ffffff"}}],
    }))
    .unwrap();
    App::with_doc("user:me", "c0", 1, Doc { nodes, vars }, 0).unwrap()
}

#[test]
fn a_variable_value_with_no_node_edit_moves_the_scene_epoch_and_repaints() {
    let mut a = painted();
    let (epoch, before) = (a.scene_epoch(), a.scene());
    a.dispatch(Action::SetVariableValue {
        variable: "bg".into(),
        mode: "light".into(),
        value: VariableValue::Literal {
            text: "#445566".into(),
        },
    });
    assert!(a.scene_epoch() > epoch);
    assert_ne!(a.scene(), before, "the new colour is painted");
}

#[test]
fn an_undo_a_redo_another_actors_op_and_a_reload_each_move_the_scene_epoch() {
    let mut a = app(true);
    a.dispatch(Action::Resize {
        id: "c".into(),
        w: 50.0,
        h: 90.0,
        x: None,
        y: None,
    });
    let mut last = a.scene_epoch();
    let mut moved = |a: &mut App, what: &str| {
        let now = a.scene_epoch();
        assert!(now > last, "{what}");
        assert_eq!(
            a.scene().len(),
            a.nodes().len(),
            "{what}: the scene is current"
        );
        last = now;
    };
    a.dispatch(Action::Undo {});
    moved(&mut a, "an undo");
    a.dispatch(Action::Redo {});
    moved(&mut a, "a redo");
    a.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Event {
            seq: 1,
            actor: "agent:other".into(),
            key: None,
            command: Command::UpdateNode {
                id: "a".into(),
                patch: NodePatch {
                    h: Some(100.0),
                    ..Default::default()
                },
            },
        },
    });
    moved(&mut a, "an event of another actor");
    assert_eq!(drawn_height(&a, "a"), 100.0);
    a.dispatch(Action::Reload {
        now_ms: 2,
        nodes: nodes(),
        variables: Default::default(),
        seq: 5,
        applied: vec![],
    });
    moved(&mut a, "a reload");
}
