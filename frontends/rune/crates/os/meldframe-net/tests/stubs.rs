//! The network half against stand-in servers that misbehave in a way the real one cannot be made to: a refused handshake after a
//! good load, and a POST that gets no answer. They need no real server, so they always run.

use meldframe_app_model::state::Connection;
use meldframe_app_model::{Action, Effect};
use meldframe_net::{Config, Net, Output, Problem};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod common;
use common::pump::{add, op};
use common::server::ACTOR;

/// A stand-in that loads an empty file (404) and answers the upgrade of the live socket with `status`: the handshake of the
/// real server cannot be refused with a token that loaded the file a moment before.
fn refuses_the_socket(status: u16) -> u16 {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let upgrade = String::from_utf8_lossy(&buf[..n])
                .to_ascii_lowercase()
                .contains("upgrade: websocket");
            let code = if upgrade { status } else { 404 };
            let _ = write!(
                stream,
                "HTTP/1.1 {code} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
        }
    });
    let config = Config::new(
        &format!("http://127.0.0.1:{port}"),
        "f7",
        ACTOR,
        "any-token-0123456789",
    )
    .expect("a config");
    let mut net = Net::start(config, Arc::new(|| {}));
    let mut seen = Vec::new();
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until
        && !seen
            .iter()
            .any(|o| matches!(o, Output::Action(Action::Reload { .. })))
    {
        seen.extend(net.wait(Duration::from_millis(100)));
    }
    net.sync(
        vec![Effect::Subscribe { after_seq: 0 }],
        &Connection::Connecting,
    );
    let mut after = Vec::new();
    while Instant::now() < until
        && !after
            .iter()
            .any(|o| matches!(o, Output::Action(Action::Disconnected {})))
    {
        after.extend(net.wait(Duration::from_millis(100)));
    }
    assert!(
        after
            .iter()
            .any(|o| matches!(o, Output::Action(Action::Disconnected {}))),
        "{after:?}"
    );
    after
        .iter()
        .find_map(|o| match o {
            Output::Problem(Some(Problem::Refused { status } | Problem::Server { status })) => {
                Some(*status)
            }
            _ => None,
        })
        .expect("a problem")
}

#[test]
fn a_handshake_that_is_refused_is_a_problem_with_its_status_and_a_lost_connection() {
    assert_eq!(refuses_the_socket(401), 401);
    assert_eq!(refuses_the_socket(503), 503);
}

/// A stand-in server: it loads an empty file (404), leaves the first POST without an answer after a pause (the network failed),
/// answers the later ones, and writes down the body of every POST in the order it came.
fn failing_first_post() -> (Net, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{Read, Write};
    use std::sync::Mutex;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let posts = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen = posts.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let seen = seen.clone();
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                let mut text = String::new();
                let mut buf = [0u8; 4096];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    text.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                if !text.starts_with("POST") {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 404 X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    );
                    return;
                }
                let first = {
                    let mut seen = seen.lock().expect("the list");
                    seen.push(text);
                    seen.len() == 1
                };
                if !first {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 X\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{{}}"
                    );
                }
                // The first one is dropped with no answer.
            });
        }
    });
    let config = Config::new(
        &format!("http://127.0.0.1:{port}"),
        "f9",
        ACTOR,
        "any-token-0123456789",
    )
    .expect("a config");
    (Net::start(config, Arc::new(|| {})), posts)
}

#[test]
fn ops_queued_behind_a_failed_post_are_not_sent_before_the_app_sends_them_again() {
    let mut now = Instant::now();
    let (mut net, posts) = failing_first_post();
    let until = now + Duration::from_secs(5);
    while now < until
        && !net
            .wait(Duration::from_millis(100))
            .iter()
            .any(|o| matches!(o, Output::Action(Action::Reload { .. })))
    {
        now = Instant::now();
    }
    // Two calls of the same connection: the second waits in the queue while the first is on the wire. The first gets no answer,
    // and the second depends on it, so it must not reach the server before the app sends the first again.
    net.sync(
        vec![Effect::SendOps {
            ops: vec![op("a1", &add("n1"))],
        }],
        &Connection::Connecting,
    );
    std::thread::sleep(Duration::from_millis(50));
    net.sync(
        vec![Effect::SendOps {
            ops: vec![op("b1", &add("n2"))],
        }],
        &Connection::Connecting,
    );
    std::thread::sleep(Duration::from_millis(800));
    let posts = posts.lock().expect("the list");
    assert_eq!(
        posts.len(),
        1,
        "only the first reached the server: {posts:#?}"
    );
    assert!(posts[0].contains("a1"), "{posts:#?}");
}
