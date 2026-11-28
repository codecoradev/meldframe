//! The network half of a native shell (E8.2.2). A shell owns a window and a socket; this crate is the socket. It loads the file
//! from a development server, keeps the live connection, carries out the effects of the app layer (`SendOps`, `Subscribe`, the
//! timers, the routes of the Activity panel, the image picker and the checkpoints) and hands back what happened as actions.
//! It has no window, no toolkit and no editor rule, so it runs and is tested without a GPU.
//!
//! The loop of a shell around it:
//!
//! 1. `Net::start(config, wake)`: a thread loads the file; the first outputs are `Action::Reload` and `Action::Attribute`.
//! 2. After each `App::dispatch`, `net.sync(effects, &app.state().connection)`.
//! 3. When `wake` fires, `net.poll()` and dispatch each `Output::Action`; an `Output::Problem` is for the status line.
//!
//! What the shell sends back as actions: `Connected` when the server accepts the subscription, `Frame` for an op, a heartbeat
//! and a resync marker, `Wire` for everything the app layer reads itself (hints, a refused subscribe, a refused op, the
//! answers of the routes), `Disconnected` when the socket is lost, and the `Reconnect` and `Tick` timers the app asked for. The
//! backoff, the replay and the resend of unconfirmed ops stay in the app layer.
//!
//! Native only: the crate uses threads, `TcpStream`, `tungstenite` and `ureq`, so it does not build for `wasm32-unknown-unknown`
//! (`docs/architecture.md` section 3b records the carve-out from L5). The token is in the handshake request, which `tungstenite`
//! logs at trace level (and `ureq` logs requests at debug level) through the `log` crate: no logger may be installed at those
//! levels for these crates. None is installed today.
//!
//! Not in this slice: TLS (the configuration refuses a server that is not on a loopback address), a token from the system
//! keychain, a PNG for an export or an agent's capture request, and a second try at the first load other than the app's own
//! reconnect.

mod config;
mod driver;
mod frames;
mod http;
mod load;
mod routes;
mod socket;

pub use config::{
    Config, ConfigError, DEFAULT_ACTOR, DEFAULT_FILE, DEFAULT_SERVER, Server, TOKEN_FILE, Token,
};
pub use http::{Http, Method, NoAnswer};
pub use load::{Loaded, Problem, load};

use driver::{Input, Out};
use meldframe_app_model::state::Connection;
use meldframe_app_model::{Action, Effect};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

/// What a thread of another kind calls when there is something to read: the same type as Rune's `Waker`.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// What the network has for the shell.
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "short-lived values; boxing would leak into every consumer pattern"
)]
pub enum Output {
    /// An action to dispatch to the app layer.
    Action(Action),
    /// What is wrong with reaching the server, or `None` once the server answered again. Words for it are the shell's.
    Problem(Option<Problem>),
}

/// The running network half. Dropping it stops its threads (a request in flight finishes on its own).
#[derive(Debug)]
pub struct Net {
    input: Sender<Input>,
    output: Receiver<Output>,
    last: Option<Connection>,
}

impl Net {
    /// Start loading the file and carrying out effects. `wake` is called from another thread each time an output is queued.
    pub fn start(config: Config, wake: Waker) -> Net {
        let (input, rx) = channel();
        let (tx, output) = channel();
        let (own, out) = (input.clone(), Out::new(tx, wake));
        std::thread::spawn(move || driver::run(config, rx, own, out));
        Net {
            input,
            output,
            last: None,
        }
    }

    /// Hand over the effects of a dispatch and the connection state after it. Nothing is sent when there is no effect and the
    /// state is the one the last call gave.
    pub fn sync(&mut self, effects: Vec<Effect>, connection: &Connection) {
        if effects.is_empty() && self.last.as_ref() == Some(connection) {
            return;
        }
        self.last = Some(*connection);
        let _ = self.input.send(Input::Sync {
            effects,
            connection: *connection,
        });
    }

    /// Everything queued since the last call, oldest first.
    pub fn poll(&self) -> Vec<Output> {
        self.output.try_iter().collect()
    }

    /// The same, waiting up to `timeout` for the first output: for a test and for a shell with no wake-up.
    pub fn wait(&self, timeout: std::time::Duration) -> Vec<Output> {
        match self.output.recv_timeout(timeout) {
            Ok(first) => std::iter::once(first)
                .chain(self.output.try_iter())
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        let _ = self.input.send(Input::Stop);
    }
}
