//! The desktop window against the real server binary (E8.2.2): the real Rune `Runtime` with no window and no GPU, the real
//! `meldframe-net` thread, and a development server started for the test with a throwaway token in its environment. The file
//! is loaded, a click selects and a button moves a node, the change reaches the server and comes back over the socket, a cut
//! socket shows as Offline and heals, and a wrong token shows as a refusal. Without `MELDFRAME_SERVER_BIN` every test prints
//! SKIPPED and passes (`common/server.rs` of `meldframe-net` says how the binary is built and what makes that a failure).

use meldframe_app::Connection;
use meldframe_desktop::{Notice, State, WINDOW_SIZE, spec};
use meldframe_net::{Config, Problem};
use meldframe_ui_rune::BAR_HEIGHT;
use rune::{Env, Runtime};
use std::time::{Duration, Instant};

/// What the Move button asks for: the coarse nudge of the app layer, 10 units to the right.
const NUDGE: f64 = 10.0;

#[path = "../../../crates/os/meldframe-net/tests/common/server.rs"]
mod server;
use server::{ACTOR, OTHER_ACTOR, Server};

/// A page with a card, and a rectangle in the card whose world rectangle is (110, 110) to (160, 150).
const FILE: &str = r##"[
  {"key":"k1","command":{"type":"addNode","node":{"id":"f1","type":"frame","x":100,"y":100,"w":300,"h":200,"fill":"#ffffff"}}},
  {"key":"k2","command":{"type":"addNode","node":{"id":"r1","type":"rect","parentId":"f1","x":10,"y":10,"w":50,"h":40,"fill":"#3366cc"}}}
]"##;

fn window(config: Config) -> Runtime<State> {
    rune::text::set_system_fonts(false);
    let mut rt = rune::App::new(spec(State::offline))
        .init(move || State::connect(config))
        .headless_runtime(WINDOW_SIZE, Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn status(rt: &Runtime<State>) -> String {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("status"))
        .and_then(|e| e.text)
        .expect("the status shows text")
}

/// Run frames, as the window does, until `done` or ten seconds have passed.
fn pump(rt: &mut Runtime<State>, what: &str, done: impl Fn(&Runtime<State>) -> bool) {
    let start = Instant::now();
    let mut t = 0.0;
    loop {
        t += 0.02;
        rt.frame(t);
        if done(rt) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}; the status says {:?}",
            status(rt)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn click(rt: &mut Runtime<State>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

fn click_key(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element keyed {key}"));
    click(rt, x, y);
}

/// The x of a node as the server holds it.
fn server_x(server: &Server, file: &str, id: &str) -> f64 {
    let (status, body) = server.request("GET", &format!("/v1/files/{file}"), None);
    assert_eq!(status, 200, "{body}");
    let snapshot: serde_json::Value = serde_json::from_str(&body).expect("a snapshot");
    let node = snapshot["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|n| n["id"] == id)
        .expect("the node");
    node["x"].as_f64().expect("an x")
}

fn live(server: &Server, file: &str) -> Runtime<State> {
    assert_eq!(server.apply(file, FILE).0, 200);
    let config = Config::new(&server.url(), file, ACTOR, &server.token).expect("a config");
    let mut rt = window(config);
    assert_eq!(
        status(&rt),
        "Connecting",
        "the window opens at once, before the file is loaded"
    );
    pump(&mut rt, "the file and a live connection", |rt| {
        rt.state.app().state().connection == Connection::Live && rt.state.app().nodes().len() == 2
    });
    rt
}

#[test]
fn the_window_loads_the_file_and_goes_live() {
    let Some(server) = Server::start() else {
        return;
    };
    let rt = live(&server, "w1");
    assert_eq!(status(&rt), "Connected");
    assert!(rt.state.notice().is_none());
    assert_eq!(rt.state.app().state().confirmed_seq, 2);
    // Who changed each node is what the server said, made safe by the app layer.
    assert_eq!(
        rt.state
            .app()
            .state()
            .last_actor
            .get("r1")
            .map(String::as_str),
        Some(ACTOR)
    );
    assert_eq!(
        rt.state.shape().builds(),
        2,
        "the shape of the empty file, then the file's"
    );
}

#[test]
fn a_click_selects_and_move_reaches_the_server_and_comes_back_confirmed() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut rt = live(&server, "w2");
    click(&mut rt, 120.0, BAR_HEIGHT + 120.0);
    assert_eq!(rt.state.app().state().selection, ["r1"]);
    click_key(&mut rt, "move");
    assert_eq!(
        rt.state.app().state().pending.len(),
        1,
        "an op waits for the server"
    );
    pump(&mut rt, "the op to be confirmed", |rt| {
        rt.state.app().state().pending.is_empty()
    });
    assert_eq!(
        rt.state.app().state().confirmed_seq,
        3,
        "the event came back over the socket"
    );
    assert_eq!(server_x(&server, "w2", "r1"), 10.0 + NUDGE);
}

#[test]
fn a_cut_socket_shows_as_offline_and_a_change_made_meanwhile_arrives_after_the_reconnect() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut rt = live(&server, "w3");
    click(&mut rt, 120.0, BAR_HEIGHT + 120.0);
    server.cut();
    pump(&mut rt, "the cut to show", |rt| {
        rt.state.app().state().connection != Connection::Live
    });
    assert_ne!(status(&rt), "Connected");
    click_key(&mut rt, "move");
    assert_eq!(
        server_x(&server, "w3", "r1"),
        10.0,
        "nothing reached the server while the socket was down"
    );
    pump(&mut rt, "the reconnect and the confirmation", |rt| {
        rt.state.app().state().connection == Connection::Live
            && rt.state.app().state().pending.is_empty()
    });
    assert_eq!(status(&rt), "Connected");
    assert_eq!(server_x(&server, "w3", "r1"), 10.0 + NUDGE);
}

#[test]
fn a_wrong_token_shows_as_a_refusal_and_the_window_stays_offline() {
    let Some(server) = Server::start() else {
        return;
    };
    let config =
        Config::new(&server.url(), "w4", ACTOR, "not-the-token-0123456789").expect("a config");
    let mut rt = window(config);
    pump(&mut rt, "the refusal", |rt| rt.state.notice().is_some());
    assert_eq!(
        rt.state.notice(),
        Some(&Notice::Net(Problem::Refused { status: 401 }))
    );
    assert!(
        status(&rt).ends_with("the server did not accept the token (401)"),
        "{}",
        status(&rt)
    );
    assert_ne!(rt.state.app().state().connection, Connection::Live);
    assert!(rt.state.app().nodes().is_empty());
}

#[test]
fn a_change_of_another_actor_made_while_the_socket_was_cut_shows_after_the_reconnect() {
    let Some(server) = Server::start() else {
        return;
    };
    let mut rt = live(&server, "w5");
    server.cut();
    pump(&mut rt, "the cut to show", |rt| {
        rt.state.app().state().connection != Connection::Live
    });
    let op = r#"[{"key":"o1","command":{"type":"addNode","node":{"id":"z1","type":"rect","x":0,"y":0,"w":5,"h":5}}}]"#;
    assert_eq!(server.apply_as_other("w5", op).0, 200);
    pump(&mut rt, "the replay after the reconnect", |rt| {
        rt.state.app().state().connection == Connection::Live && rt.state.app().nodes().len() == 3
    });
    assert_eq!(
        rt.state
            .app()
            .state()
            .last_actor
            .get("z1")
            .map(String::as_str),
        Some(OTHER_ACTOR)
    );
}
