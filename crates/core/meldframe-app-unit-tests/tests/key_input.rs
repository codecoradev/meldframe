//! A key press as an action (`Action::Key`, audit finding T7a, decision 43 clause Q1), driven without a front end: each editor key
//! does what it says, a key that has nothing to act on raises nothing, and a key of an input method that is composing, or of a
//! field, is never an editor key. The rules are the ones `apps/web` kept in TypeScript (`edit.ts`, `keys.ts` and `Editor.svelte`).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::keys::*;
use common::*;
use meldframe_app::{
    Action, App, Connection, Event, KeyFocus, Modifiers, Overlay, Panel, TextEditOp, Tool,
};
use meldframe_protocol::{Command, NodePatch};
use serde_json::json;

#[test]
fn the_tool_letters_set_the_tool_in_either_case_and_say_the_key_was_used() {
    let mut l = live();
    for (k, tool) in [
        ("f", Tool::Frame),
        ("R", Tool::Rectangle),
        ("e", Tool::Ellipse),
        ("t", Tool::Text),
        ("v", Tool::Select),
    ] {
        l.send(key(k));
        assert_eq!(l.app.state().tool, tool, "{k}");
        let events = drain(&mut l);
        assert!(
            events.contains(&Event::ToolChanged { tool }) && handled(&events),
            "{k}"
        );
    }
    // The key of the tool that is already picked is the editor's all the same: the browser must not act on it.
    l.send(key("v"));
    assert_eq!(drain(&mut l), vec![Event::KeyHandled {}]);
}

#[test]
fn delete_and_backspace_remove_the_selection_as_one_step() {
    for k in ["Delete", "Backspace"] {
        let mut l = live();
        l.send(Action::Select {
            ids: ids(&["r1", "t1"]),
        });
        drain(&mut l);
        l.send(key(k));
        assert!(
            !exists(&l, "r1") && !exists(&l, "t1") && exists(&l, "root"),
            "{k}"
        );
        let events = drain(&mut l);
        assert!(changed(&events) && handled(&events), "{k}");
        l.send(Action::Undo {});
        assert!(
            exists(&l, "r1") && exists(&l, "t1"),
            "{k}: one undo takes both back"
        );
    }
}

#[test]
fn delete_with_nothing_selected_does_nothing_and_raises_nothing() {
    let mut l = live();
    nothing(&mut l, key("Delete"));
    nothing(&mut l, key("Backspace"));
    assert_eq!(l.app.nodes().len(), 3);
}

#[test]
fn undo_and_redo_follow_ctrl_and_command_with_the_shift_and_y_variants() {
    let mut l = live();
    l.send(Action::Move {
        id: "r1".into(),
        x: 40.0,
        y: 0.0,
    });
    assert_eq!(get(&l.app, "r1").x, 40.0);
    drain(&mut l);
    for (undo, redo) in [
        (
            ctrl("z"),
            press(
                "Z",
                "",
                Modifiers {
                    ctrl: true,
                    shift: true,
                    ..NONE
                },
            ),
        ),
        (
            cmd("z"),
            press(
                "Z",
                "",
                Modifiers {
                    meta: true,
                    shift: true,
                    ..NONE
                },
            ),
        ),
        (ctrl("Z"), ctrl("y")),
        (cmd("z"), cmd("Y")),
    ] {
        l.send(undo);
        assert_eq!(get(&l.app, "r1").x, 0.0);
        let events = drain(&mut l);
        assert!(changed(&events) && handled(&events));
        l.send(redo);
        assert_eq!(get(&l.app, "r1").x, 40.0);
        let events = drain(&mut l);
        assert!(changed(&events) && handled(&events));
    }
}

#[test]
fn undo_with_nothing_to_undo_changes_nothing_but_the_key_is_the_editors() {
    let mut l = live();
    let fx = l.send(ctrl("z"));
    assert!(fx.is_empty());
    assert_eq!(drain(&mut l), vec![Event::KeyHandled {}]);
}

#[test]
fn z_without_ctrl_or_command_and_ctrl_alt_z_are_no_undo() {
    let mut l = live();
    l.send(Action::Move {
        id: "r1".into(),
        x: 40.0,
        y: 0.0,
    });
    drain(&mut l);
    nothing(&mut l, key("z"));
    nothing(
        &mut l,
        press(
            "Z",
            "",
            Modifiers {
                shift: true,
                ..NONE
            },
        ),
    );
    nothing(
        &mut l,
        press(
            "z",
            "",
            Modifiers {
                ctrl: true,
                alt: true,
                ..NONE
            },
        ),
    );
    assert_eq!(get(&l.app, "r1").x, 40.0);
}

#[test]
fn enter_opens_the_edit_of_the_one_selected_text_and_only_then() {
    let mut l = live();
    nothing(&mut l, key("Enter"));
    l.send(Action::Select { ids: ids(&["r1"]) });
    drain(&mut l);
    nothing(&mut l, key("Enter"));
    l.send(Action::Select {
        ids: ids(&["t1", "r1"]),
    });
    drain(&mut l);
    nothing(&mut l, key("Enter"));
    l.send(Action::Select { ids: ids(&["t1"]) });
    drain(&mut l);
    nothing(&mut l, focused(key("Enter"), KeyFocus::Control));
    nothing(&mut l, ctrl("Enter"));
    assert!(l.app.state().text_edit.is_none());
    l.send(key("Enter"));
    assert_eq!(
        l.app.state().text_edit.as_ref().map(|e| e.id.as_str()),
        Some("t1")
    );
    let events = drain(&mut l);
    assert!(events.contains(&Event::TextEditChanged {}) && handled(&events));
}

#[test]
fn escape_cancels_the_text_edit_then_closes_the_overlay_and_otherwise_does_nothing() {
    let mut l = live();
    nothing(&mut l, key("Escape"));
    l.send(Action::Select { ids: ids(&["t1"]) });
    l.send(Action::TextEdit {
        op: TextEditOp::BeginSelected {},
    });
    l.send(key("?"));
    drain(&mut l);
    assert!(l.app.state().text_edit.is_some() && l.app.state().overlay == Some(Overlay::Help));
    l.send(key("Escape"));
    assert!(
        l.app.state().text_edit.is_none(),
        "the edit is cancelled first"
    );
    assert_eq!(l.app.state().overlay, Some(Overlay::Help));
    assert!(handled(&drain(&mut l)));
    l.send(key("Escape"));
    assert_eq!(l.app.state().overlay, None);
    let events = drain(&mut l);
    assert!(events.contains(&Event::OverlayChanged { overlay: None }) && handled(&events));
    nothing(&mut l, key("Escape"));
}

#[test]
fn the_panel_keys_toggle_by_the_physical_key_whatever_character_option_types() {
    let mut l = live();
    let alt = Modifiers { alt: true, ..NONE };
    for (character, code, panel) in [
        ("\u{a1}", "Digit1", Panel::Layers),
        ("\u{2122}", "Digit2", Panel::Properties),
    ] {
        assert!(open_of(&l, panel));
        l.send(press(character, code, alt));
        assert!(!open_of(&l, panel));
        let events = drain(&mut l);
        assert!(events.contains(&Event::PanelChanged { panel, open: false }) && handled(&events));
        l.send(press("1", code, alt));
        assert!(open_of(&l, panel));
        drain(&mut l);
    }
    nothing(&mut l, press("1", "Digit1", NONE));
}

fn open_of(l: &Loopback, panel: Panel) -> bool {
    match panel {
        Panel::Layers => l.app.state().panels.layers,
        Panel::Properties => l.app.state().panels.properties,
        Panel::Variables => l.app.state().panels.variables,
    }
}

#[test]
fn the_sheet_and_the_palette_open_one_at_a_time_and_the_palette_stays_shut_over_a_dialog() {
    let mut l = live();
    l.send(press(
        "?",
        "Slash",
        Modifiers {
            shift: true,
            ..NONE
        },
    ));
    assert_eq!(l.app.state().overlay, Some(Overlay::Help));
    assert!(handled(&drain(&mut l)));
    l.send(ctrl("k"));
    assert_eq!(
        l.app.state().overlay,
        Some(Overlay::Palette),
        "the sheet closes as the palette opens"
    );
    l.send(cmd("k"));
    assert_eq!(l.app.state().overlay, None, "the same key closes it");
    drain(&mut l);
    select(&mut l, "root");
    open(&mut l);
    drain(&mut l);
    assert!(l.app.state().export.dialog.is_some());
    l.send(ctrl("k"));
    assert_eq!(l.app.state().overlay, None, "no palette over a dialog");
    assert!(
        handled(&drain(&mut l)),
        "the browser's own Ctrl+K is the editor's all the same"
    );
}

#[test]
fn a_key_of_an_input_method_that_is_composing_or_of_a_field_is_never_an_action() {
    let mut l = live();
    l.send(Action::Select { ids: ids(&["r1"]) });
    drain(&mut l);
    let composing = |mut a: Action| {
        if let Action::Key { input } = &mut a {
            input.composing = true;
        }
        a
    };
    for a in [
        key("Delete"),
        key("v"),
        ctrl("z"),
        key("Backspace"),
        key("Enter"),
    ] {
        nothing(&mut l, composing(a.clone()));
        nothing(&mut l, focused(a.clone(), KeyFocus::TextField));
    }
    assert!(exists(&l, "r1") && l.app.state().tool == Tool::Select);
}

#[test]
fn a_menu_or_a_dialog_keeps_the_plain_keys_but_not_undo() {
    let mut l = live();
    l.send(Action::Select { ids: ids(&["r1"]) });
    drain(&mut l);
    nothing(&mut l, focused(key("Delete"), KeyFocus::Region));
    nothing(&mut l, focused(key("r"), KeyFocus::Region));
    assert!(exists(&l, "r1"));
    l.send(focused(ctrl("z"), KeyFocus::Region));
    assert!(handled(&drain(&mut l)));
}

#[test]
fn a_held_key_does_not_repeat_its_action() {
    let mut l = live();
    l.send(Action::Move {
        id: "r1".into(),
        x: 40.0,
        y: 0.0,
    });
    l.send(Action::Move {
        id: "r1".into(),
        x: 80.0,
        y: 0.0,
    });
    drain(&mut l);
    let mut held = ctrl("z");
    if let Action::Key { input } = &mut held {
        input.repeat = true;
    }
    nothing(&mut l, held);
    assert_eq!(get(&l.app, "r1").x, 80.0);
}

#[test]
fn an_unknown_key_makes_no_action_and_no_event() {
    let mut l = live();
    for k in ["", "F13", "Dead", "q", "\u{202e}", "x"] {
        nothing(&mut l, key(k));
        nothing(&mut l, ctrl(k));
    }
    for k in ["+", "-", "=", " ", "Tab"] {
        nothing(&mut l, key(k)); // the front end's: the camera
    }
    nothing(
        &mut l,
        press(
            "1",
            "Digit1",
            Modifiers {
                shift: true,
                ..NONE
            },
        ),
    );
    assert_eq!(l.app.take_events(), vec![]);
}

#[test]
fn a_halted_app_still_raises_key_handled_for_delete_with_a_selection_but_writes_nothing() {
    let mut l = live();
    l.send(Action::Select { ids: ids(&["r1"]) });
    let ghost = Command::UpdateNode {
        id: "ghost".into(),
        patch: NodePatch {
            text: Some("x".into()),
            ..Default::default()
        },
    };
    l.remote("agent:a", "k", 1, ghost);
    assert_eq!(l.app.state().connection, Connection::Halted);
    drain(&mut l);
    // A tool is view state: it still changes.
    l.send(key("e"));
    assert_eq!(l.app.state().tool, Tool::Ellipse);
    drain(&mut l);
    // Delete is a write: the app is halted, so nothing is deleted, and the key is the editor's all the same (the browser's
    // Delete or Backspace must not go back a page), as it was when the page dispatched it and kept the key.
    let fx = l.send(key("Delete"));
    assert!(fx.is_empty());
    assert!(exists(&l, "r1"), "a halted app writes nothing");
    assert_eq!(drain(&mut l), vec![Event::KeyHandled {}]);
}

#[test]
fn an_arrow_key_with_nothing_selected_is_a_no_op_and_raises_nothing() {
    let nodes = serde_json::from_value(json!([
        {"id": "col", "type": "frame", "pos": "a0", "w": 200, "h": 300, "layout": "column"},
        {"id": "a", "type": "rect", "parentId": "col", "pos": "a0", "w": 50, "h": 20},
        {"id": "b", "type": "rect", "parentId": "col", "pos": "a1", "w": 50, "h": 20},
    ]))
    .unwrap();
    let mut app = App::new("user:me", "c0", 1, nodes, 0).unwrap();
    app.set_layout_on_canvas(true);
    app.dispatch(Action::Connected { now_ms: 0 });
    app.take_events();
    for k in ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"] {
        let fx = app.dispatch(key(k));
        assert!(fx.is_empty(), "{k}");
        assert_eq!(app.take_events(), vec![], "{k}");
    }
    assert_eq!(app.state().history.undo_steps, 0);
}

#[test]
fn a_key_with_no_focus_is_refused_when_the_action_is_read() {
    let full = json!({"type": "key", "input": {"key": "Delete", "code": "Delete", "modifiers": {},
                      "repeat": false, "composing": false, "focus": "textField"}});
    assert!(serde_json::from_value::<Action>(full.clone()).is_ok());
    // The flags, the code and the modifiers may be left out; the focus may not, or a key typed in a field would act.
    let mut lean = full.clone();
    for field in ["repeat", "composing", "modifiers", "code"] {
        lean["input"].as_object_mut().unwrap().remove(field);
    }
    assert!(serde_json::from_value::<Action>(lean.clone()).is_ok());
    lean["input"].as_object_mut().unwrap().remove("focus");
    assert!(serde_json::from_value::<Action>(lean).is_err());
}
