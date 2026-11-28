//! The thread that carries out the effects of the app layer. It owns the socket, the timers and the queue of ops; every
//! thread it starts (the socket, the sender of ops, one request) reports back to it or straight to the output. It keeps no
//! editor rule: when to reconnect, how long to back off, what to send again and which frame means what are the app layer's, and
//! the driver only does what an effect asks and says what happened as an action. It follows the shell of the web client
//! (`packages/web-transport/src/shell.ts`) step for step.

use crate::config::Config;
use crate::frames::{Heard, hear};
use crate::http::{Http, Method};
use crate::load::{self, Problem};
use crate::routes::{self, segment};
use crate::socket::{self, Event, Socket};
use crate::{Output, Waker};
use meldframe_app_model::state::Connection;
use meldframe_app_model::wire::Wire;
use meldframe_app_model::{Action, Effect, OutOp};
use meldframe_protocol::ClientMsg;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How often the app is told the time, so it can notice a silent connection or a stuck attempt.
pub const TICK: Duration = Duration::from_secs(1);
/// The server drops a client it has not heard from within 45 s; a JSON ping keeps a quiet one alive.
pub const PING: Duration = Duration::from_secs(15);
/// The tool that applies a list of commands; the server registers it for every transport.
const TOOL: &str = "apply_commands";

/// What reaches the driver.
#[derive(Debug)]
pub enum Input {
    /// The effects of a dispatch and the connection state after it.
    Sync {
        effects: Vec<Effect>,
        connection: Connection,
    },
    Socket {
        id: u64,
        event: Event,
    },
    /// A request of the ops of the current connection got no answer, and the sender ended that connection's epoch.
    PostFailed,
    Stop,
}

/// Where the actions and problems go, and the wake-up that tells the window to look.
#[derive(Clone)]
pub(crate) struct Out {
    tx: Sender<Output>,
    wake: Waker,
}

impl Out {
    pub(crate) fn new(tx: Sender<Output>, wake: Waker) -> Out {
        Out { tx, wake }
    }

    fn put(&self, output: Output) {
        if self.tx.send(output).is_ok() {
            (self.wake)();
        }
    }

    pub(crate) fn action(&self, action: Action) {
        self.put(Output::Action(action));
    }

    fn problem(&self, problem: Option<Problem>) {
        self.put(Output::Problem(problem));
    }
}

pub(crate) fn now_ms() -> u64 {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
}

/// The ops of one `SendOps` effect, with the connection they were made for.
struct Job {
    epoch: u64,
    ops: Vec<OutOp>,
}

#[derive(Clone, Copy)]
enum Later {
    Reconnect,
    Tick,
}

struct Driver {
    config: Config,
    http: Http,
    input: Sender<Input>,
    out: Out,
    ops: Sender<Job>,
    /// Counts lost connections: ops queued for an older one are not sent.
    epoch: Arc<AtomicU64>,
    connection: Connection,
    /// Whether the file was loaded: the app is not given a socket before it holds a snapshot.
    loaded: bool,
    socket: Option<Socket>,
    id: u64,
    last_sub: Option<String>,
    sub_id: u64,
    waits: Vec<(Instant, Later)>,
    next_tick: Instant,
    next_ping: Option<Instant>,
}

pub(crate) fn run(config: Config, rx: Receiver<Input>, input: Sender<Input>, out: Out) {
    let http = Http::new(&config);
    let epoch = Arc::new(AtomicU64::new(0));
    let (ops, queued) = channel();
    start_sender(
        &config,
        &http,
        queued,
        epoch.clone(),
        out.clone(),
        input.clone(),
    );
    let mut d = Driver {
        config,
        http,
        input,
        out,
        ops,
        epoch,
        connection: Connection::Connecting,
        loaded: false,
        socket: None,
        id: 0,
        last_sub: None,
        sub_id: 0,
        waits: Vec::new(),
        next_tick: Instant::now() + TICK,
        next_ping: None,
    };
    d.load();
    loop {
        let due = d
            .waits
            .iter()
            .map(|w| w.0)
            .chain([d.next_tick])
            .chain(d.next_ping)
            .min();
        let wait = due.map_or(TICK, |t| t.saturating_duration_since(Instant::now()));
        match rx.recv_timeout(wait) {
            Ok(Input::Stop) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            Ok(input) => d.handle(input),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        d.fire();
    }
}

impl Driver {
    /// Fetch the file and give it to the app. A failure is told as a problem and as a lost connection, so the app's own
    /// backoff decides when to try again (its next `Subscribe` loads the file first).
    fn load(&mut self) {
        match load::load(&self.http, &self.config.file) {
            Ok(file) => {
                self.loaded = true;
                self.out.problem(None);
                for action in file.actions(now_ms()) {
                    self.out.action(action);
                }
            }
            Err(problem) => {
                self.out.problem(Some(problem));
                self.out.action(Action::Disconnected {});
            }
        }
    }

    fn handle(&mut self, input: Input) {
        match input {
            Input::Sync {
                effects,
                connection,
            } => self.sync(effects, connection),
            Input::Socket { id, event } if self.socket.as_ref().is_some_and(|s| s.id == id) => {
                self.heard(event);
            }
            // The sender has already ended the epoch of the failed request (so the ops queued behind it do not go out
            // before the app sends everything again); only a live connection is lost by it, one the app already knows is
            // down is on its way back.
            Input::PostFailed if self.connection == Connection::Live => self.lost(),
            _ => {}
        }
    }

    fn sync(&mut self, effects: Vec<Effect>, connection: Connection) {
        let before = std::mem::replace(&mut self.connection, connection);
        for effect in effects {
            self.effect(effect);
        }
        // A halted app ignores everything but the view: nothing is left to keep alive until the file is loaded again.
        // A live connection the app found silent is dropped too: a socket kept until the next `Subscribe` would go on pinging.
        if self.connection == Connection::Halted
            || (self.connection == Connection::Offline && before == Connection::Live)
        {
            self.drop_socket();
        }
        if self.connection == Connection::Halted {
            self.waits.clear();
        }
    }

    fn effect(&mut self, effect: Effect) {
        match effect {
            Effect::Subscribe { after_seq } => self.subscribe(after_seq),
            Effect::SendOps { ops } => {
                let epoch = self.epoch.load(Ordering::SeqCst);
                let _ = self.ops.send(Job { epoch, ops });
            }
            Effect::Reconnect { after_ms } => {
                // The app has taken the connection down: whatever socket there was is not trusted any more.
                self.drop_socket();
                self.later(after_ms, Later::Reconnect);
            }
            Effect::WakeAfter { after_ms } => self.later(after_ms, Later::Tick),
            // Nothing in this shell draws a PNG or answers a capture request (it never says it can), and the views redraw
            // from the state.
            Effect::Render { .. }
            | Effect::Raster { .. }
            | Effect::FinishRaster { .. }
            | Effect::FailCapture { .. } => {}
            other => {
                if let Some(call) = routes::call(&self.config.file, &other) {
                    let (http, out) = (self.http.clone(), self.out.clone());
                    std::thread::spawn(move || {
                        let action = match http.call(call.method, &call.path, call.body.as_deref())
                        {
                            Ok((status, text)) => (call.answer)(status, text),
                            Err(_) => call.failure,
                        };
                        out.action(action);
                    });
                }
            }
        }
    }

    fn later(&mut self, after_ms: u64, what: Later) {
        let after = Duration::from_millis(after_ms.min(u64::from(u32::MAX)));
        self.waits.push((Instant::now() + after, what));
    }

    fn subscribe(&mut self, after_seq: u64) {
        if !self.loaded {
            return self.load();
        }
        if self.socket.is_some() && self.connection == Connection::Live {
            // A resync marker or a gap: the stream waits for a subscribe on the same socket.
            return self.resubscribe(after_seq);
        }
        // Connecting or reconnecting: whatever socket there was is not trusted any more.
        self.drop_socket();
        self.id += 1;
        self.socket = Some(socket::open(
            &self.config,
            after_seq,
            self.id,
            self.input.clone(),
        ));
        self.next_ping = Some(Instant::now() + PING);
    }

    fn resubscribe(&mut self, after_seq: u64) {
        self.sub_id += 1;
        let id = format!("s{}", self.sub_id);
        self.last_sub = Some(id.clone());
        let message = ClientMsg::Subscribe {
            id,
            room: self.config.file.clone(),
            since_seq: Some(after_seq),
        };
        self.send(&message);
    }

    /// Write a message on the socket; a socket that cannot take it is lost.
    fn send(&mut self, message: &ClientMsg) {
        let sent = match (&self.socket, serde_json::to_string(message)) {
            (Some(socket), Ok(text)) => socket.send(text),
            _ => false,
        };
        if !sent {
            self.lost();
        }
    }

    fn heard(&mut self, event: Event) {
        match event {
            Event::Text(text) => match hear(&text, now_ms(), self.last_sub.as_deref()) {
                Heard::Subscribed => {
                    self.out.problem(None);
                    self.out.action(Action::Connected { now_ms: now_ms() });
                }
                Heard::Do(action) => self.out.action(action),
                Heard::Lost => self.lost(),
                Heard::Ignore => {}
            },
            Event::Closed => self.lost(),
            Event::Refused(status) => {
                self.out.problem(Some(Problem::of_status(status)));
                self.lost();
            }
            Event::Failed => {
                self.out.problem(Some(Problem::Server { status: 0 }));
                self.lost();
            }
        }
    }

    /// Forget the socket and close it; what it says from now on is ignored.
    fn drop_socket(&mut self) {
        // Requests queued or in flight belong to the connection that is gone: the app sends everything again, with the same
        // keys, once it is back.
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.socket = None;
        self.last_sub = None;
        self.next_ping = None;
    }

    /// The connection is lost for the app: drop the socket and tell it.
    fn lost(&mut self) {
        self.drop_socket();
        self.out.action(Action::Disconnected {});
    }

    /// Run what is due: the timers the app asked for, the tick and the ping.
    fn fire(&mut self) {
        let now = Instant::now();
        let (due, waiting): (Vec<_>, Vec<_>) = self.waits.drain(..).partition(|w| w.0 <= now);
        self.waits = waiting;
        for (_, what) in due {
            let now_ms = now_ms();
            self.out.action(match what {
                Later::Reconnect => Action::Reconnect { now_ms },
                Later::Tick => Action::Tick { now_ms },
            });
        }
        if self.next_tick <= now {
            self.next_tick = now + TICK;
            self.out.action(Action::Tick { now_ms: now_ms() });
        }
        if self.next_ping.is_some_and(|t| t <= now) {
            self.next_ping = Some(now + PING);
            self.send(&ClientMsg::Ping {});
        }
    }
}

/// One request per op, in order: the thread that sends the ops of every `SendOps` effect one after the other.
fn start_sender(
    config: &Config,
    http: &Http,
    queued: Receiver<Job>,
    epoch: Arc<AtomicU64>,
    out: Out,
    input: Sender<Input>,
) {
    let path = format!("/v1/files/{}/tools/{TOOL}", segment(&config.file));
    let http = http.clone();
    std::thread::spawn(move || {
        for job in queued {
            for op in job.ops {
                // The connection was lost: the app sends everything again.
                if epoch.load(Ordering::SeqCst) != job.epoch {
                    break;
                }
                let body = serde_json::json!({
                    "ops": [{ "key": op.key, "command": op.command, "deps": op.deps }]
                })
                .to_string();
                match http.call(Method::Post, &path, Some(&body)) {
                    // The live event confirms an accepted op.
                    Ok((200, _)) => {}
                    // Anything else is a refusal: the app layer reads the body (it may come from a proxy).
                    Ok((status, text)) => out.action(Action::Wire {
                        wire: Wire::OpRefused {
                            now_ms: now_ms(),
                            key: op.key,
                            status,
                            body: text,
                        },
                    }),
                    // The later ops depend on this one: stop, and the app sends every unconfirmed op again.
                    Err(_) => {
                        // End the epoch here, before the next job is looked at (the driver would be too late: the next
                        // job could be sent first, ahead of the op it depends on). A failure of an epoch that was already
                        // over says nothing about the current connection.
                        let current = epoch.compare_exchange(
                            job.epoch,
                            job.epoch + 1,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        );
                        if current.is_ok() {
                            let _ = input.send(Input::PostFailed);
                        }
                        break;
                    }
                }
            }
        }
    });
}
