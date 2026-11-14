//! The app layer's data is the contract with every front end: its JSON shape is pinned here.

use meldframe_app::{
    Action, AppState, Connection, Effect, Event, History, Outcome, Panel, Panels, Tool,
};
use meldframe_protocol::Command;
use serde_json::{Value, json};

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("serialises")
}

#[test]
fn default_state_is_an_empty_editor() {
    let state = AppState::default();
    assert_eq!(
        to_json(&state),
        json!({
            "selection": [],
            "tool": "select",
            "panels": { "left": true, "right": true },
            "connection": "connecting",
            "history": { "canUndo": false, "canRedo": false },
            "pending": []
        })
    );
}

#[test]
fn state_round_trips() {
    let state = AppState {
        selection: vec!["a".into(), "b".into()],
        tool: Tool::Rectangle,
        panels: Panels {
            left: false,
            right: true,
        },
        connection: Connection::Connected,
        history: History {
            can_undo: true,
            can_redo: false,
        },
        pending: vec![3, 4],
    };
    let back: AppState = serde_json::from_value(to_json(&state)).expect("parses");
    assert_eq!(back, state);
}

#[test]
fn every_tool_and_connection_has_a_stable_name() {
    for (tool, name) in [
        (Tool::Select, "select"),
        (Tool::Frame, "frame"),
        (Tool::Rectangle, "rectangle"),
        (Tool::Text, "text"),
    ] {
        assert_eq!(to_json(&tool), json!(name));
    }
    for (connection, name) in [
        (Connection::Connecting, "connecting"),
        (Connection::Connected, "connected"),
        (Connection::Disconnected, "disconnected"),
    ] {
        assert_eq!(to_json(&connection), json!(name));
    }
}

#[test]
fn actions_use_a_type_tag_and_camel_case_fields() {
    let cases = [
        (
            Action::Select {
                ids: vec!["n1".into()],
            },
            json!({ "type": "select", "ids": ["n1"] }),
        ),
        (
            Action::ToggleSelect { id: "n1".into() },
            json!({ "type": "toggleSelect", "id": "n1" }),
        ),
        (
            Action::ClearSelection {},
            json!({ "type": "clearSelection" }),
        ),
        (
            Action::SetTool { tool: Tool::Text },
            json!({ "type": "setTool", "tool": "text" }),
        ),
        (
            Action::SetPanel {
                panel: Panel::Right,
                open: false,
            },
            json!({ "type": "setPanel", "panel": "right", "open": false }),
        ),
        (
            Action::TogglePanel { panel: Panel::Left },
            json!({ "type": "togglePanel", "panel": "left" }),
        ),
        (Action::Undo {}, json!({ "type": "undo" })),
        (Action::Redo {}, json!({ "type": "redo" })),
        (
            Action::ConnectionChanged {
                connection: Connection::Disconnected,
            },
            json!({ "type": "connectionChanged", "connection": "disconnected" }),
        ),
        (
            Action::HistoryChanged {
                can_undo: true,
                can_redo: true,
            },
            json!({ "type": "historyChanged", "canUndo": true, "canRedo": true }),
        ),
        (
            Action::CommandResult {
                op: 7,
                outcome: Outcome::Accepted { seq: 12 },
            },
            json!({
                "type": "commandResult",
                "op": 7,
                "outcome": { "type": "accepted", "seq": 12 }
            }),
        ),
        (
            Action::CommandResult {
                op: 8,
                outcome: Outcome::Rejected {
                    code: "conflict".into(),
                    retryable: false,
                },
            },
            json!({
                "type": "commandResult",
                "op": 8,
                "outcome": { "type": "rejected", "code": "conflict", "retryable": false }
            }),
        ),
    ];
    for (action, expected) in cases {
        assert_eq!(to_json(&action), expected);
        let back: Action = serde_json::from_value(expected).expect("parses");
        assert_eq!(to_json(&back), to_json(&action));
    }
}

#[test]
fn a_submitted_command_is_the_protocol_command() {
    let wire = json!({
        "type": "submit",
        "command": { "type": "removeNode", "id": "n1" }
    });
    let action: Action = serde_json::from_value(wire.clone()).expect("parses");
    assert!(matches!(
        action,
        Action::Submit {
            command: Command::RemoveNode { .. }
        }
    ));
    assert_eq!(to_json(&action), wire);
}

#[test]
fn effects_are_plain_data() {
    let send = Effect::SendCommand {
        op: 1,
        command: Command::RemoveNode { id: "n1".into() },
    };
    assert_eq!(
        to_json(&send),
        json!({
            "type": "sendCommand",
            "op": 1,
            "command": { "type": "removeNode", "id": "n1" }
        })
    );
    assert_eq!(
        to_json(&Effect::RequestUndo {}),
        json!({ "type": "requestUndo" })
    );
    assert_eq!(
        to_json(&Effect::RequestRedo {}),
        json!({ "type": "requestRedo" })
    );
    assert_eq!(
        to_json(&Effect::Reconnect {}),
        json!({ "type": "reconnect" })
    );
}

#[test]
fn events_describe_one_change_each() {
    let cases = [
        (
            Event::SelectionChanged {
                ids: vec!["n1".into()],
            },
            json!({ "type": "selectionChanged", "ids": ["n1"] }),
        ),
        (
            Event::ToolChanged { tool: Tool::Frame },
            json!({ "type": "toolChanged", "tool": "frame" }),
        ),
        (
            Event::PanelChanged {
                panel: Panel::Left,
                open: true,
            },
            json!({ "type": "panelChanged", "panel": "left", "open": true }),
        ),
        (
            Event::ConnectionChanged {
                connection: Connection::Connected,
            },
            json!({ "type": "connectionChanged", "connection": "connected" }),
        ),
        (
            Event::HistoryChanged {
                can_undo: false,
                can_redo: true,
            },
            json!({ "type": "historyChanged", "canUndo": false, "canRedo": true }),
        ),
        (
            Event::CommandRejected {
                op: 2,
                code: "invalid".into(),
                retryable: true,
            },
            json!({ "type": "commandRejected", "op": 2, "code": "invalid", "retryable": true }),
        ),
    ];
    for (event, expected) in cases {
        assert_eq!(to_json(&event), expected);
        let back: Event = serde_json::from_value(expected).expect("parses");
        assert_eq!(back, event);
    }
}

#[test]
fn unknown_fields_are_refused() {
    let action = json!({ "type": "undo", "extra": 1 });
    assert!(serde_json::from_value::<Action>(action).is_err());
    let state = json!({
        "selection": [], "tool": "select", "panels": { "left": true, "right": true },
        "connection": "connecting", "history": { "canUndo": false, "canRedo": false },
        "pending": [], "extra": 1
    });
    assert!(serde_json::from_value::<AppState>(state).is_err());
}
