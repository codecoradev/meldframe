//! The real server binary for a test: started on a random port with a throwaway token in its environment (never on its command
//! line), files in memory, and a relay in front of it whose connections a test can cut. The binary is built from the main
//! workspace (`cargo build -p meldframe-server`) and named by `MELDFRAME_SERVER_BIN`; without it a test prints SKIPPED and
//! passes, and `MELDFRAME_REQUIRE_SERVER=1` makes that a failure (as `MELDFRAME_REQUIRE_PG` does for the Postgres tests).
//! Shared by path with the tests of the desktop crate.

#![allow(dead_code)]

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const ACTOR: &str = "agent:test";
/// The actor of the second token of the server.
pub const OTHER_ACTOR: &str = "agent:other";

fn random_hex() -> String {
    let mut h = RandomState::new().build_hasher();
    h.write_u32(std::process::id());
    format!("{:016x}", h.finish())
}

pub struct Server {
    child: Child,
    /// The port of the server itself.
    pub port: u16,
    /// The token the server was started with.
    pub token: String,
    /// A second token, of another actor in the same workspace.
    pub other_token: String,
    assets: PathBuf,
    relay: Relay,
}

impl Server {
    /// A running server, or `None` (and a printed SKIPPED) when no binary is named.
    pub fn start() -> Option<Server> {
        let Some(bin) = std::env::var_os("MELDFRAME_SERVER_BIN") else {
            assert!(
                std::env::var_os("MELDFRAME_REQUIRE_SERVER").is_none(),
                "MELDFRAME_SERVER_BIN is not set"
            );
            println!(
                "SKIPPED: MELDFRAME_SERVER_BIN names no server binary (cargo build -p meldframe-server in the main workspace)"
            );
            return None;
        };
        let token = format!("test-{}-{}", random_hex(), random_hex());
        let other_token = format!("test-{}-{}", random_hex(), random_hex());
        let assets = std::env::temp_dir().join(format!("meldframe-net-test-{}", random_hex()));
        for _ in 0..5 {
            let port = free_port();
            // Nothing of the parent's environment reaches the server: a DATABASE_URL (the documented development setup) would
            // make it write into the development database, and MELDFRAME_REVIEWERS or another variable would change what it does.
            let mut child = Command::new(&bin)
                .env_clear()
                .env("MELDFRAME_LISTEN", format!("127.0.0.1:{port}"))
                .env(
                    "MELDFRAME_DEV_TOKENS",
                    format!("{token}:ws_test:{ACTOR},{other_token}:ws_test:{OTHER_ACTOR}"),
                )
                .env("MELDFRAME_ASSETS_DIR", &assets)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("the server binary starts");
            let until = Instant::now() + Duration::from_secs(10);
            while Instant::now() < until {
                if child.try_wait().expect("the child can be asked").is_some() {
                    break; // the port was taken meanwhile: try another
                }
                if get(port, "/v1/health", None).is_some_and(|(s, _)| s == 200) {
                    return Some(Server {
                        child,
                        port,
                        token,
                        other_token,
                        assets,
                        relay: Relay::new(port),
                    });
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        panic!("the server did not start");
    }

    /// The address a client should use: the relay's, so a test can cut the connection.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.relay.port)
    }

    /// Close every connection that went through the relay (new ones are accepted again).
    pub fn cut(&self) {
        self.relay.cut();
    }

    /// A request straight to the server as this token (the status and the body).
    pub fn request(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
        request(self.port, method, path, Some(&self.token), body).expect("the server answers")
    }

    /// Apply commands to a file as the second actor.
    pub fn apply_as_other(&self, file: &str, ops: &str) -> (u16, String) {
        let path = format!("/v1/files/{file}/tools/apply_commands");
        request(
            self.port,
            "POST",
            &path,
            Some(&self.other_token),
            Some(&format!(r#"{{"ops":{ops}}}"#)),
        )
        .expect("the server answers")
    }

    /// Apply commands to a file as the token's actor.
    pub fn apply(&self, file: &str, ops: &str) -> (u16, String) {
        self.request(
            "POST",
            &format!("/v1/files/{file}/tools/apply_commands"),
            Some(&format!(r#"{{"ops":{ops}}}"#)),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.assets);
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free port")
        .local_addr()
        .expect("an address")
        .port()
}

pub fn get(port: u16, path: &str, token: Option<&str>) -> Option<(u16, String)> {
    request(port, "GET", path, token, None)
}

/// HTTP/1.0 over a plain socket: the answer ends when the server closes it.
pub fn request(
    port: u16,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let auth = token.map_or(String::new(), |t| format!("Authorization: Bearer {t}\r\n"));
    let body = body.unwrap_or("");
    let head = format!(
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body.as_bytes()).ok()?;
    let mut text = String::new();
    stream.read_to_string(&mut text).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status = head.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, body.to_string()))
}

/// A TCP relay to the server: the connections of a client go through it, and `cut` closes them all.
struct Relay {
    port: u16,
    open: Arc<Mutex<Vec<TcpStream>>>,
    stop: Arc<AtomicBool>,
}

impl Relay {
    fn new(target: u16) -> Relay {
        let listener = TcpListener::bind("127.0.0.1:0").expect("the relay listens");
        let port = listener.local_addr().expect("an address").port();
        let (open, stop) = (
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(AtomicBool::new(false)),
        );
        let (held, done) = (open.clone(), stop.clone());
        std::thread::spawn(move || {
            for client in listener.incoming() {
                let Ok(client) = client else { continue };
                if done.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(server) = TcpStream::connect(("127.0.0.1", target)) else {
                    continue;
                };
                for s in [&client, &server] {
                    held.lock()
                        .expect("the list")
                        .push(s.try_clone().expect("a clone"));
                }
                pipe(
                    client.try_clone().expect("a clone"),
                    server.try_clone().expect("a clone"),
                );
                pipe(server, client);
            }
        });
        Relay { port, open, stop }
    }

    fn cut(&self) {
        for s in self.open.lock().expect("the list").drain(..) {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.cut();
        let _ = TcpStream::connect(("127.0.0.1", self.port)); // wakes the accept loop so it ends
    }
}

fn pipe(mut from: TcpStream, mut to: TcpStream) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = from.read(&mut buf) {
            if n == 0 || to.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        let _ = to.shutdown(Shutdown::Both);
    });
}
