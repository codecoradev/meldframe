//! A second front end (standard rule U8): a scripted driver with no UI that plays the platform. It sends
//! actions, carries out effects against a fake server and records what it sees, using only the public interface.

use meldframe_app::{Action, App, AppState, Connection, Effect, Event, Outcome, Tool};
use meldframe_protocol::Command;
use std::collections::VecDeque;

/// Plays the platform: answers commands, and keeps every effect and event it was given.
struct Driver {
    app: App,
    /// Commands the fake server has accepted, in order.
    accepted: Vec<Command>,
    /// The code the fake server refuses the next command with, if any.
    refuse_next: Option<&'static str>,
    effects: Vec<Effect>,
    events: Vec<Event>,
    next_seq: u64,
}

impl Driver {
    fn new() -> Self {
        Self {
            app: App::new(),
            accepted: Vec::new(),
            refuse_next: None,
            effects: Vec::new(),
            events: Vec::new(),
            next_seq: 1,
        }
    }

    fn state(&self) -> &AppState {
        self.app.state()
    }

    /// Dispatches the action, then carries out each effect the way a real front end would.
    fn send(&mut self, action: Action) {
        let mut queue = VecDeque::from([action]);
        while let Some(action) = queue.pop_front() {
            let effects = self.app.dispatch(action);
            self.events.extend(self.app.take_events());
            for effect in effects {
                queue.extend(self.perform(&effect));
                self.effects.push(effect);
            }
        }
    }

    /// The fake platform: what each effect turns into.
    fn perform(&mut self, effect: &Effect) -> Vec<Action> {
        match effect {
            Effect::SendCommand { op, command } => {
                let outcome = match self.refuse_next.take() {
                    Some(code) => Outcome::Rejected {
                        code: code.into(),
                        retryable: false,
                    },
                    None => {
                        self.accepted.push(command.clone());
                        self.next_seq += 1;
                        Outcome::Accepted {
                            seq: self.next_seq - 1,
                        }
                    }
                };
                let can_undo = !self.accepted.is_empty();
                vec![
                    Action::CommandResult { op: *op, outcome },
                    Action::HistoryChanged {
                        can_undo,
                        can_redo: false,
                    },
                ]
            }
            Effect::RequestUndo {} => {
                self.accepted.pop();
                vec![Action::HistoryChanged {
                    can_undo: !self.accepted.is_empty(),
                    can_redo: true,
                }]
            }
            Effect::RequestRedo {} => vec![Action::HistoryChanged {
                can_undo: true,
                can_redo: false,
            }],
            Effect::Reconnect {} => vec![
                Action::ConnectionChanged {
                    connection: Connection::Connecting,
                },
                Action::ConnectionChanged {
                    connection: Connection::Connected,
                },
            ],
        }
    }
}

fn remove(id: &str) -> Action {
    Action::Submit {
        command: Command::RemoveNode { id: id.into() },
    }
}

#[test]
fn edit_then_undo_then_redo() {
    let mut driver = Driver::new();
    driver.send(Action::ConnectionChanged {
        connection: Connection::Connected,
    });
    driver.send(Action::SetTool { tool: Tool::Select });
    driver.send(Action::Select {
        ids: vec!["n1".into()],
    });
    driver.send(remove("n1"));
    assert_eq!(driver.accepted.len(), 1);
    assert!(driver.state().pending.is_empty());
    assert!(driver.state().history.can_undo);

    driver.send(Action::Undo {});
    assert!(driver.accepted.is_empty());
    assert!(!driver.state().history.can_undo);
    assert!(driver.state().history.can_redo);

    driver.send(Action::Redo {});
    assert!(driver.state().history.can_undo);
    assert!(!driver.state().history.can_redo);
}

#[test]
fn a_refused_command_is_reported_and_leaves_nothing_pending() {
    let mut driver = Driver::new();
    driver.refuse_next = Some("conflict");
    driver.send(remove("n1"));
    assert!(driver.accepted.is_empty());
    assert!(driver.state().pending.is_empty());
    assert!(driver.events.iter().any(|event| matches!(
        event,
        Event::CommandRejected { code, .. } if code == "conflict"
    )));
}

#[test]
fn a_dropped_connection_comes_back() {
    let mut driver = Driver::new();
    driver.send(Action::ConnectionChanged {
        connection: Connection::Connected,
    });
    driver.send(Action::ConnectionChanged {
        connection: Connection::Disconnected,
    });
    assert_eq!(driver.state().connection, Connection::Connected);
    assert!(
        driver
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::Reconnect {}))
    );
    let seen: Vec<Connection> = driver
        .events
        .iter()
        .filter_map(|event| match event {
            Event::ConnectionChanged { connection } => Some(*connection),
            _ => None,
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            Connection::Connected,
            Connection::Disconnected,
            Connection::Connecting,
            Connection::Connected
        ]
    );
}

#[test]
fn the_driver_needs_no_toolkit_and_state_is_plain_data() {
    let mut driver = Driver::new();
    driver.send(Action::Select {
        ids: vec!["a".into(), "b".into()],
    });
    let json = serde_json::to_string(driver.state()).expect("serialises");
    let back: AppState = serde_json::from_str(&json).expect("parses");
    assert_eq!(&back, driver.state());
}
