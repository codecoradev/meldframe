//! The editor rules, driven without a front end: dispatch an action, then read state, effects and events.

use meldframe_app::{Action, App, Connection, Effect, Event, Panel, Tool};

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn select(app: &mut App, list: &[&str]) {
    app.dispatch(Action::Select { ids: ids(list) });
    app.take_events();
}

#[test]
fn a_new_app_starts_with_the_default_state_and_no_events() {
    let mut app = App::new();
    assert_eq!(*app.state(), meldframe_app::AppState::default());
    assert!(app.take_events().is_empty());
}

#[test]
fn select_replaces_the_selection_and_drops_duplicates() {
    let mut app = App::new();
    let effects = app.dispatch(Action::Select {
        ids: ids(&["a", "b", "a"]),
    });
    assert!(effects.is_empty());
    assert_eq!(app.state().selection, ids(&["a", "b"]));
    assert_eq!(
        app.take_events(),
        vec![Event::SelectionChanged {
            ids: ids(&["a", "b"])
        }]
    );
}

#[test]
fn selecting_what_is_already_selected_raises_no_event() {
    let mut app = App::new();
    select(&mut app, &["a", "b"]);
    app.dispatch(Action::Select {
        ids: ids(&["a", "b"]),
    });
    assert!(app.take_events().is_empty());
}

#[test]
fn toggle_adds_then_removes_a_node() {
    let mut app = App::new();
    select(&mut app, &["a"]);
    app.dispatch(Action::ToggleSelect { id: "b".into() });
    assert_eq!(app.state().selection, ids(&["a", "b"]));
    app.dispatch(Action::ToggleSelect { id: "a".into() });
    assert_eq!(app.state().selection, ids(&["b"]));
    assert_eq!(
        app.take_events(),
        vec![
            Event::SelectionChanged {
                ids: ids(&["a", "b"])
            },
            Event::SelectionChanged { ids: ids(&["b"]) },
        ]
    );
}

#[test]
fn clearing_an_empty_selection_raises_no_event() {
    let mut app = App::new();
    app.dispatch(Action::ClearSelection {});
    assert!(app.take_events().is_empty());
    select(&mut app, &["a"]);
    app.dispatch(Action::ClearSelection {});
    assert!(app.state().selection.is_empty());
    assert_eq!(
        app.take_events(),
        vec![Event::SelectionChanged { ids: vec![] }]
    );
}

#[test]
fn set_tool_changes_the_tool_once() {
    let mut app = App::new();
    app.dispatch(Action::SetTool { tool: Tool::Text });
    app.dispatch(Action::SetTool { tool: Tool::Text });
    assert_eq!(app.state().tool, Tool::Text);
    assert_eq!(
        app.take_events(),
        vec![Event::ToolChanged { tool: Tool::Text }]
    );
}

#[test]
fn panels_open_close_and_toggle_independently() {
    let mut app = App::new();
    app.dispatch(Action::SetPanel {
        panel: Panel::Left,
        open: false,
    });
    app.dispatch(Action::TogglePanel {
        panel: Panel::Right,
    });
    assert!(!app.state().panels.left);
    assert!(!app.state().panels.right);
    app.dispatch(Action::TogglePanel { panel: Panel::Left });
    assert!(app.state().panels.left);
    assert_eq!(
        app.take_events(),
        vec![
            Event::PanelChanged {
                panel: Panel::Left,
                open: false
            },
            Event::PanelChanged {
                panel: Panel::Right,
                open: false
            },
            Event::PanelChanged {
                panel: Panel::Left,
                open: true
            },
        ]
    );
}

#[test]
fn setting_a_panel_to_its_current_state_raises_no_event() {
    let mut app = App::new();
    app.dispatch(Action::SetPanel {
        panel: Panel::Left,
        open: true,
    });
    assert!(app.take_events().is_empty());
}

#[test]
fn a_dropped_connection_asks_for_a_reconnect() {
    let mut app = App::new();
    app.dispatch(Action::ConnectionChanged {
        connection: Connection::Connected,
    });
    let effects = app.dispatch(Action::ConnectionChanged {
        connection: Connection::Disconnected,
    });
    assert_eq!(app.state().connection, Connection::Disconnected);
    assert!(matches!(effects.as_slice(), [Effect::Reconnect {}]));
    assert_eq!(
        app.take_events(),
        vec![
            Event::ConnectionChanged {
                connection: Connection::Connected
            },
            Event::ConnectionChanged {
                connection: Connection::Disconnected
            },
        ]
    );
}

#[test]
fn reconnecting_and_reconnected_ask_for_nothing() {
    let mut app = App::new();
    app.dispatch(Action::ConnectionChanged {
        connection: Connection::Disconnected,
    });
    let connecting = app.dispatch(Action::ConnectionChanged {
        connection: Connection::Connecting,
    });
    let connected = app.dispatch(Action::ConnectionChanged {
        connection: Connection::Connected,
    });
    assert!(connecting.is_empty());
    assert!(connected.is_empty());
}

#[test]
fn repeating_the_same_connection_state_does_nothing() {
    let mut app = App::new();
    app.dispatch(Action::ConnectionChanged {
        connection: Connection::Disconnected,
    });
    app.take_events();
    let effects = app.dispatch(Action::ConnectionChanged {
        connection: Connection::Disconnected,
    });
    assert!(effects.is_empty());
    assert!(app.take_events().is_empty());
}

#[test]
fn the_same_actions_give_the_same_state_and_effects() {
    let run = || {
        let mut app = App::new();
        let mut all = Vec::new();
        for action in [
            Action::Select { ids: ids(&["a"]) },
            Action::SetTool { tool: Tool::Frame },
            Action::ConnectionChanged {
                connection: Connection::Disconnected,
            },
        ] {
            all.extend(app.dispatch(action));
        }
        (
            app.state().clone(),
            serde_json::to_string(&all).expect("serialises"),
        )
    };
    assert_eq!(run(), run());
}
