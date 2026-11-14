//! Undo, redo, submitted commands and the answers to them.

use meldframe_app::{Action, App, Effect, Event, Outcome};
use meldframe_protocol::Command;
use serde_json::json;

fn remove(id: &str) -> Action {
    Action::Submit {
        command: Command::RemoveNode { id: id.into() },
    }
}

fn history(can_undo: bool, can_redo: bool) -> Action {
    Action::HistoryChanged { can_undo, can_redo }
}

#[test]
fn undo_and_redo_do_nothing_while_the_history_is_empty() {
    let mut app = App::new();
    assert!(app.dispatch(Action::Undo {}).is_empty());
    assert!(app.dispatch(Action::Redo {}).is_empty());
    assert!(app.take_events().is_empty());
}

#[test]
fn undo_asks_the_core_when_the_history_allows_it() {
    let mut app = App::new();
    app.dispatch(history(true, false));
    let effects = app.dispatch(Action::Undo {});
    assert!(matches!(effects.as_slice(), [Effect::RequestUndo {}]));
    assert!(app.dispatch(Action::Redo {}).is_empty());
}

#[test]
fn redo_asks_the_core_when_the_history_allows_it() {
    let mut app = App::new();
    app.dispatch(history(false, true));
    let effects = app.dispatch(Action::Redo {});
    assert!(matches!(effects.as_slice(), [Effect::RequestRedo {}]));
    assert!(app.dispatch(Action::Undo {}).is_empty());
}

#[test]
fn the_history_report_updates_the_buttons_once() {
    let mut app = App::new();
    app.dispatch(history(true, false));
    app.dispatch(history(true, false));
    assert!(app.state().history.can_undo);
    assert!(!app.state().history.can_redo);
    assert_eq!(
        app.take_events(),
        vec![Event::HistoryChanged {
            can_undo: true,
            can_redo: false
        }]
    );
}

#[test]
fn undo_does_not_change_the_buttons_before_the_core_answers() {
    let mut app = App::new();
    app.dispatch(history(true, false));
    app.dispatch(Action::Undo {});
    assert!(app.state().history.can_undo);
}

#[test]
fn submit_sends_the_command_with_numbered_ops() {
    let mut app = App::new();
    let first = app.dispatch(remove("a"));
    let second = app.dispatch(remove("b"));
    let wire = |effects: &[Effect]| serde_json::to_value(effects).expect("serialises");
    assert_eq!(
        wire(&first),
        json!([{ "type": "sendCommand", "op": 1, "command": { "type": "removeNode", "id": "a" } }])
    );
    assert_eq!(
        wire(&second),
        json!([{ "type": "sendCommand", "op": 2, "command": { "type": "removeNode", "id": "b" } }])
    );
    assert_eq!(app.state().pending, vec![1, 2]);
}

#[test]
fn an_accepted_command_leaves_the_pending_list() {
    let mut app = App::new();
    app.dispatch(remove("a"));
    app.dispatch(remove("b"));
    let effects = app.dispatch(Action::CommandResult {
        op: 1,
        outcome: Outcome::Accepted { seq: 5 },
    });
    assert!(effects.is_empty());
    assert_eq!(app.state().pending, vec![2]);
    assert!(app.take_events().is_empty());
}

#[test]
fn a_rejected_command_leaves_the_pending_list_and_raises_an_event() {
    let mut app = App::new();
    app.dispatch(remove("a"));
    app.dispatch(Action::CommandResult {
        op: 1,
        outcome: Outcome::Rejected {
            code: "conflict".into(),
            retryable: false,
        },
    });
    assert!(app.state().pending.is_empty());
    assert_eq!(
        app.take_events(),
        vec![Event::CommandRejected {
            op: 1,
            code: "conflict".into(),
            retryable: false
        }]
    );
}

#[test]
fn an_answer_for_an_unknown_op_is_ignored() {
    let mut app = App::new();
    app.dispatch(remove("a"));
    app.dispatch(Action::CommandResult {
        op: 9,
        outcome: Outcome::Rejected {
            code: "conflict".into(),
            retryable: false,
        },
    });
    assert_eq!(app.state().pending, vec![1]);
    assert!(app.take_events().is_empty());
}
