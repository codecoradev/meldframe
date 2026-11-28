//! The arrow keys through the key input (`Action::Key`, audit finding T7a, E8.1.1): a key moves the selection as `Action::Nudge`
//! does (a child of an auto layout frame one place along its flow, any other node by 1 unit, 10 with Shift), says it was the
//! editor's only when it moved something, and a short sequence of keys (pick a tool, nudge the selection, undo) is read from
//! the state with no front end.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::pointer;
use layout_fixture::*;
use meldframe_app::{Action, App, Event, KeyFocus, KeyInput, Modifiers, Tool};

fn press(a: &mut App, key: &str, modifiers: Modifiers) -> Vec<Event> {
    press_in(a, key, modifiers, KeyFocus::Page)
}
fn press_in(a: &mut App, key: &str, modifiers: Modifiers, focus: KeyFocus) -> Vec<Event> {
    a.take_events();
    a.dispatch(Action::Key {
        input: KeyInput {
            key: key.into(),
            code: String::new(),
            modifiers,
            repeat: false,
            composing: false,
            focus,
        },
    });
    a.take_events()
}
fn plain(a: &mut App, key: &str) -> Vec<Event> {
    press(a, key, Modifiers::default())
}
fn handled(events: &[Event]) -> bool {
    events.contains(&Event::KeyHandled {})
}
fn select(a: &mut App, ids: &[&str]) {
    a.dispatch(Action::Select {
        ids: ids.iter().map(|s| s.to_string()).collect(),
    });
    a.take_events();
}
fn flow(a: &App) -> Vec<String> {
    let mut k = kids(a, Some("f"));
    k.retain(|id| id != "abs");
    k
}

#[test]
fn an_arrow_key_along_the_flow_moves_the_child_one_place_and_is_the_editors() {
    let mut a = app(true);
    select(&mut a, &["a"]);
    let events = plain(&mut a, "ArrowDown");
    assert_eq!(flow(&a), ["b", "a", "c"]);
    assert!(
        handled(&events)
            && events
                .iter()
                .any(|e| matches!(e, Event::DocumentChanged { .. }))
    );
    assert_eq!(steps(&a), 1);
    plain(&mut a, "ArrowUp");
    assert_eq!(flow(&a), ["a", "b", "c"]);
}

#[test]
fn an_arrow_key_that_moves_nothing_raises_no_event_so_the_browser_keeps_it() {
    let mut a = app(true);
    select(&mut a, &["a"]);
    // At the start of the flow going up, and across the axis of a column.
    for key in ["ArrowUp", "ArrowLeft", "ArrowRight"] {
        assert_eq!(plain(&mut a, key), vec![], "{key}");
    }
    // No selection at all.
    select(&mut a, &[]);
    assert_eq!(plain(&mut a, "ArrowDown"), vec![]);
    assert_eq!(flow(&a), ["a", "b", "c"]);
}

#[test]
fn with_the_layout_on_the_canvas_off_an_arrow_key_moves_the_stored_box_of_any_node() {
    let mut a = app(false);
    select(&mut a, &["a"]);
    let events = plain(&mut a, "ArrowDown");
    assert!(handled(&events));
    assert_eq!(flow(&a), ["a", "b", "c"]);
    assert_eq!((node(&a, "a").x, node(&a, "a").y), (0.0, 1.0));
    assert_eq!(steps(&a), 1);
}

#[test]
fn an_arrow_key_moves_a_free_node_by_one_unit_and_shift_by_ten_and_is_the_editors() {
    let mut a = app(true);
    select(&mut a, &["fg"]);
    let events = plain(&mut a, "ArrowRight");
    assert!(handled(&events) && (node(&a, "fg").x, node(&a, "fg").y) == (21.0, 20.0));
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };
    for (key, to) in [
        ("ArrowDown", (21.0, 30.0)),
        ("ArrowLeft", (11.0, 30.0)),
        ("ArrowUp", (11.0, 20.0)),
    ] {
        assert!(handled(&press(&mut a, key, shift)), "{key}");
        assert_eq!((node(&a, "fg").x, node(&a, "fg").y), to, "{key}");
    }
    assert_eq!(steps(&a), 4);
}

#[test]
fn a_held_arrow_key_a_press_in_a_field_or_under_the_pointer_is_not_a_nudge() {
    let mut a = app(true);
    select(&mut a, &["fg"]);
    let held = KeyInput {
        key: "ArrowRight".into(),
        code: String::new(),
        modifiers: Modifiers::default(),
        repeat: true,
        composing: false,
        focus: KeyFocus::Page,
    };
    a.dispatch(Action::Key { input: held });
    assert_eq!(a.take_events(), vec![]);
    for focus in [KeyFocus::TextField, KeyFocus::Region] {
        assert_eq!(
            press_in(&mut a, "ArrowRight", Modifiers::default(), focus),
            vec![]
        );
    }
    // A press is down on the canvas: the gesture is the person's hands' business.
    pointer::press(&mut a, 435.0, 35.0);
    a.take_events();
    assert_eq!(plain(&mut a, "ArrowRight"), vec![]);
    assert_eq!(node(&a, "fg").x, 20.0);
    assert_eq!(steps(&a), 0);
}

#[test]
fn an_arrow_key_with_a_modifier_or_in_a_field_is_not_an_arrow_key_of_the_editor() {
    let mut a = app(true);
    select(&mut a, &["a"]);
    for m in [
        Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        Modifiers {
            alt: true,
            ..Modifiers::default()
        },
        Modifiers {
            ctrl: true,
            ..Modifiers::default()
        },
        Modifiers {
            meta: true,
            ..Modifiers::default()
        },
    ] {
        assert_eq!(press(&mut a, "ArrowDown", m), vec![], "{m:?}");
    }
    for focus in [KeyFocus::TextField, KeyFocus::Region] {
        let events = press_in(&mut a, "ArrowDown", Modifiers::default(), focus);
        assert_eq!(events, vec![], "{focus:?}");
    }
    assert_eq!(flow(&a), ["a", "b", "c"]);
}

#[test]
fn a_sequence_of_keys_picks_a_tool_nudges_the_selection_and_undoes_it() {
    let mut a = app(true);
    assert_eq!(a.state().tool, Tool::Select);
    plain(&mut a, "r");
    assert_eq!(a.state().tool, Tool::Rectangle);
    plain(&mut a, "V");
    assert_eq!(a.state().tool, Tool::Select);
    select(&mut a, &["b"]);
    plain(&mut a, "ArrowDown");
    assert_eq!(flow(&a), ["a", "c", "b"]);
    plain(&mut a, "ArrowUp");
    plain(&mut a, "ArrowUp");
    assert_eq!(flow(&a), ["b", "a", "c"]);
    assert_eq!(steps(&a), 3);
    let undo = Modifiers {
        ctrl: true,
        ..Modifiers::default()
    };
    press(&mut a, "z", undo);
    assert_eq!(flow(&a), ["a", "b", "c"]);
    assert_eq!(steps(&a), 2);
}
