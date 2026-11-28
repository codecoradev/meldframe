//! The network half against the real server binary, with no app layer and no window: effects go in, actions come out. See
//! `common/server.rs` for how the server is found; without a binary every test prints SKIPPED and passes. The cases that need a
//! server that misbehaves are in `stubs.rs`.

use meldframe_app_model::state::Connection;
use meldframe_app_model::{Action, Effect, Frame, Wire};
use meldframe_net::{Config, Net, Output, Problem};
use std::sync::Arc;
use std::time::Duration;

mod common;
use common::pump::{Pump, add, op};
use common::server::{ACTOR, OTHER_ACTOR, Server};

#[test]
fn a_file_the_server_does_not_know_loads_empty_and_the_subscription_connects() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut p = Pump::new(&server, "new_file", &server.token);
    p.action(0, |a| {
        matches!(a, Action::Reload { nodes, seq: 0, .. } if nodes.is_empty()).then_some(())
    });
    p.sync(
        vec![Effect::Subscribe { after_seq: 0 }],
        Connection::Connecting,
    );
    p.action(0, |a| matches!(a, Action::Connected { .. }).then_some(()));
}

#[test]
fn a_file_with_nodes_loads_with_who_changed_each() {
    let Some(server) = Server::start() else {
        return;
    };
    assert_eq!(
        server
            .apply("f1", &format!(r#"[{{"key":"k1","command":{}}}]"#, add("a")))
            .0,
        200
    );
    let mut p = Pump::new(&server, "f1", &server.token);
    p.action(0, |a| {
        matches!(a, Action::Reload { nodes, seq: 1, .. } if nodes.len() == 1).then_some(())
    });
    let actors = p.action(0, |a| match a {
        Action::Attribute { actors } => Some(actors.clone()),
        _ => None,
    });
    assert_eq!(actors["a"], ACTOR);
}

#[test]
fn an_op_sent_by_effect_comes_back_as_the_event_that_confirms_it() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut p = Pump::new(&server, "f2", &server.token);
    p.live(0);
    p.sync(
        vec![Effect::SendOps {
            ops: vec![op("mine-1", &add("n1"))],
        }],
        Connection::Live,
    );
    let (seq, key, actor) = p.action(0, |a| match a {
        Action::Frame {
            frame: Frame::Event {
                seq, key, actor, ..
            },
            ..
        } => Some((*seq, key.clone(), actor.clone())),
        _ => None,
    });
    assert_eq!(
        (seq, key.as_deref(), actor.as_str()),
        (1, Some("mine-1"), ACTOR)
    );
    // The touch ring of the actor is an ephemeral hint: handed over as the text the socket sent, for the app layer to read.
    let hint = p.action(0, |a| match a {
        Action::Wire {
            wire: Wire::Hint { text, .. },
        } => Some(text.clone()),
        _ => None,
    });
    assert!(
        hint.contains(r#""type":"touch""#) && hint.contains(r#""n1""#),
        "{hint}"
    );
}

#[test]
fn a_refused_op_is_handed_over_with_the_status_and_the_text() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut p = Pump::new(&server, "f3", &server.token);
    p.live(0);
    // A node under a parent that does not exist.
    let bad = r#"{"type":"addNode","node":{"id":"x","type":"rect","parentId":"nope"}}"#;
    p.sync(
        vec![Effect::SendOps {
            ops: vec![op("bad-1", bad)],
        }],
        Connection::Live,
    );
    let (key, status, body) = p.action(0, |a| match a {
        Action::Wire {
            wire: Wire::OpRefused {
                key, status, body, ..
            },
        } => Some((key.clone(), *status, body.clone())),
        _ => None,
    });
    assert_eq!(key, "bad-1");
    assert!((400..500).contains(&status), "{status}");
    assert!(body.contains("error"), "{body}");
}

#[test]
fn a_cut_socket_is_a_disconnect_and_the_timers_the_app_asks_for_come_back() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut p = Pump::new(&server, "f4", &server.token);
    p.live(0);
    let seen = p.seen.len();
    server.cut();
    p.action(seen, |a| matches!(a, Action::Disconnected {}).then_some(()));
    // The app answers with a backoff; the shell waits and says it is over.
    let seen = p.seen.len();
    p.sync(
        vec![Effect::Reconnect { after_ms: 50 }],
        Connection::Offline,
    );
    p.action(seen, |a| {
        matches!(a, Action::Reconnect { .. }).then_some(())
    });
    // And the next subscription opens a new socket on the same file.
    p.sync(
        vec![Effect::Subscribe { after_seq: 0 }],
        Connection::Reconnecting { attempt: 1 },
    );
    p.action(seen, |a| {
        matches!(a, Action::Connected { .. }).then_some(())
    });
}

#[test]
fn the_ticks_the_app_asks_for_and_the_regular_one_arrive() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut p = Pump::new(&server, "f5", &server.token);
    p.action(0, |a| matches!(a, Action::Tick { .. }).then_some(())); // the one a second
    let seen = p.seen.len();
    p.sync(
        vec![Effect::WakeAfter { after_ms: 20 }],
        Connection::Connecting,
    );
    p.action(seen, |a| matches!(a, Action::Tick { .. }).then_some(()));
}

#[test]
fn a_wrong_token_is_a_refusal_and_the_app_is_told_the_connection_failed() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut p = Pump::new(&server, "f6", "not-the-token-0123456789");
    let problem = p.wait(0, |o| match o {
        Output::Problem(Some(p)) => Some(*p),
        _ => None,
    });
    assert_eq!(problem, Problem::Refused { status: 401 });
    p.action(0, |a| matches!(a, Action::Disconnected {}).then_some(()));
    // A later subscription tries the load again, and is refused again: no socket is opened for a file that was not loaded.
    let seen = p.seen.len();
    p.sync(
        vec![Effect::Subscribe { after_seq: 0 }],
        Connection::Offline,
    );
    p.wait(seen, |o| {
        matches!(o, Output::Problem(Some(Problem::Refused { status: 401 }))).then_some(())
    });
    assert!(
        !p.seen
            .iter()
            .any(|o| matches!(o, Output::Action(Action::Connected { .. })))
    );
}

#[test]
fn a_server_named_localhost_is_reached_whichever_loopback_address_the_name_resolves_to_first() {
    let Some(server) = Server::start() else {
        return;
    };
    // The server listens on 127.0.0.1 only; the name may give ::1 first, and the load and the socket must both get through.
    let url = server.url().replace("127.0.0.1", "localhost");
    let config = Config::new(&url, "f10", ACTOR, &server.token).expect("a config");
    let mut p = Pump {
        net: Net::start(config, Arc::new(|| {})),
        seen: Vec::new(),
    };
    p.live(0);
}

#[test]
fn an_op_of_another_actor_made_while_the_socket_was_cut_is_replayed_after_the_last_number() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut p = Pump::new(&server, "f11", &server.token);
    p.live(0);
    let seen = p.seen.len();
    server.cut();
    p.action(seen, |a| matches!(a, Action::Disconnected {}).then_some(()));
    let ops = format!(r#"[{{"key":"o1","command":{}}}]"#, add("z1"));
    assert_eq!(server.apply_as_other("f11", &ops).0, 200);
    // The app asks again from the last number it holds (0): the op it missed comes first, as the actor that made it.
    let seen = p.seen.len();
    p.sync(
        vec![Effect::Subscribe { after_seq: 0 }],
        Connection::Reconnecting { attempt: 1 },
    );
    let (seq, actor) = p.action(seen, |a| match a {
        Action::Frame {
            frame: Frame::Event { seq, actor, .. },
            ..
        } => Some((*seq, actor.clone())),
        _ => None,
    });
    assert_eq!((seq, actor.as_str()), (1, OTHER_ACTOR));
}

#[test]
fn the_routes_of_the_batches_the_assets_and_the_checkpoints_are_answered_by_the_server() {
    let Some(server) = Server::start() else {
        return;
    };
    assert_eq!(
        server
            .apply(
                "f12",
                &format!(r#"[{{"key":"k1","command":{}}}]"#, add("a"))
            )
            .0,
        200
    );
    let mut p = Pump::new(&server, "f12", &server.token);
    p.live(1);
    let effects = vec![
        Effect::FetchBatches {
            before_n: None,
            after_n: None,
            limit: 20,
        },
        Effect::FetchCheckpoints {},
        Effect::FetchAssets {
            after: None,
            limit: 10,
            request: 7,
        },
    ];
    p.sync(effects, Connection::Live);
    let batches = p.action(0, |a| match a {
        Action::Wire {
            wire: Wire::Batches { status, body },
        } => Some((*status, body.clone())),
        _ => None,
    });
    assert_eq!(batches.0, 200, "{}", batches.1);
    assert!(batches.1.contains("batches"), "{}", batches.1);
    let checkpoints = p.action(0, |a| match a {
        Action::Wire {
            wire: Wire::Checkpoints { status, body },
        } => Some((*status, body.clone())),
        _ => None,
    });
    assert_eq!(checkpoints.0, 200, "{}", checkpoints.1);
    let assets = p.action(0, |a| match a {
        Action::Wire {
            wire:
                Wire::Assets {
                    request: 7,
                    status,
                    body,
                    ..
                },
        } => Some((*status, body.clone())),
        _ => None,
    });
    assert_eq!(assets.0, 200, "{}", assets.1);
}

#[test]
fn a_server_that_is_not_there_is_a_problem_with_no_status() {
    let Some(server) = Server::start() else {
        return;
    };
    let dead = Config::new("http://127.0.0.1:1", "f8", ACTOR, &server.token).expect("a config");
    let net = Net::start(dead, Arc::new(|| {}));
    let outputs = net.wait(Duration::from_secs(10));
    assert!(
        outputs
            .iter()
            .any(|o| matches!(o, Output::Problem(Some(Problem::Server { status: 0 })))),
        "{outputs:?}"
    );
}
