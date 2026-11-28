//! The live socket of one file (`GET /v1/live/{file}?after={seq}`): a thread that opens it with the token in the handshake
//! header, reads text frames, writes what it is given (a ping, a `subscribe`), and tells its owner what happened. The owner
//! drops the `Socket` to close it. The thread reads with a short timeout, so a write waits at most one poll.

use crate::config::Config;
use crate::driver::Input;
use crate::routes::segment;
use std::io;
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;
use tungstenite::client::{IntoClientRequest, client};
use tungstenite::error::Error;
use tungstenite::handshake::HandshakeError;
use tungstenite::http::HeaderValue;
use tungstenite::{Message, WebSocket};

/// How long to wait for the server to accept the connection and finish the handshake.
const CONNECT: Duration = Duration::from_secs(5);
/// How long a read waits before the thread looks for something to write.
const POLL: Duration = Duration::from_millis(50);

/// What a socket tells its owner.
#[derive(Debug)]
pub enum Event {
    /// A text frame.
    Text(String),
    /// The server closed it, or it failed after it was open.
    Closed,
    /// The handshake was answered with this HTTP status (401: the token; 429 and 503: the server is busy).
    Refused(u16),
    /// No connection was made.
    Failed,
}

/// An open (or opening) socket. Dropping it closes the connection.
#[derive(Debug)]
pub struct Socket {
    /// Which socket of the owner this is: what a late event of an old one is told apart by.
    pub id: u64,
    out: Sender<String>,
}

impl Socket {
    /// Queue a text frame. `false` when the socket is gone.
    pub fn send(&self, text: String) -> bool {
        self.out.send(text).is_ok()
    }
}

/// Open the socket of `file` from `after` on a thread of its own; its events reach `input` tagged with `id`.
pub fn open(config: &Config, after: u64, id: u64, input: Sender<Input>) -> Socket {
    let (out, queued) = channel();
    let config = config.clone();
    std::thread::spawn(move || {
        let tell = |event| input.send(Input::Socket { id, event }).is_ok();
        match connect(&config, after) {
            Ok(ws) => run(ws, &queued, &tell),
            Err(event) => {
                tell(event);
            }
        }
    });
    Socket { id, out }
}

fn connect(config: &Config, after: u64) -> Result<WebSocket<TcpStream>, Event> {
    // The first address that takes the connection: `localhost` may resolve to `::1` first while the server listens on `127.0.0.1`.
    let stream = config
        .server
        .addresses()
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, CONNECT).ok())
        .ok_or(Event::Failed)?;
    // Timeouts bound the handshake; the poll timeout replaces the read one once the socket is open.
    let _ = stream.set_nodelay(true);
    stream
        .set_read_timeout(Some(CONNECT))
        .map_err(|_| Event::Failed)?;
    stream
        .set_write_timeout(Some(CONNECT))
        .map_err(|_| Event::Failed)?;
    let url = config
        .server
        .ws(&format!("/v1/live/{}?after={after}", segment(&config.file)));
    let mut request = url.into_client_request().map_err(|_| Event::Failed)?;
    let bearer = HeaderValue::from_str(&config.token.bearer()).map_err(|_| Event::Failed)?;
    request.headers_mut().insert("authorization", bearer);
    match client(request, stream) {
        Ok((ws, _)) => Ok(ws),
        Err(HandshakeError::Failure(Error::Http(response))) => {
            Err(Event::Refused(response.status().as_u16()))
        }
        Err(_) => Err(Event::Failed),
    }
}

fn run(mut ws: WebSocket<TcpStream>, queued: &Receiver<String>, tell: &dyn Fn(Event) -> bool) {
    let _ = ws.get_ref().set_read_timeout(Some(POLL));
    loop {
        loop {
            match queued.try_recv() {
                Ok(text) => {
                    if ws.send(Message::text(text)).is_err() {
                        tell(Event::Closed);
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                // The owner dropped the socket: say goodbye and go.
                Err(TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    return;
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                if !tell(Event::Text(text.as_str().to_string())) {
                    return;
                }
            }
            Ok(Message::Close(_)) => {
                tell(Event::Closed);
                return;
            }
            Ok(_) => {}
            Err(Error::Io(e))
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {
                tell(Event::Closed);
                return;
            }
        }
    }
}
