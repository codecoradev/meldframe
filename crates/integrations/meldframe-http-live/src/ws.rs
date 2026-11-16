//! `GET /v1/live/{file}?after={seq}`: one WebSocket is one stream of one file (ADR-0009). The handshake
//! authenticates and subscribes, so a refusal is an HTTP status (401, 429, 503). Then the server sends
//! `subscribed`, the ops after `after` as `event` frames (each names the op's idempotency key), and what the stream
//! queues: more events, ephemeral frames, `heartbeat`, and `resync` when the client fell behind. The client sends
//! `ping` (answered with `pong`) to show it is alive, and after a `resync` it sends `subscribe` (with `sinceSeq` =
//! the marker's `afterSeq`, or without it) on the same socket; `subscribe` is refused before a marker, and never
//! starts before the marker. Ops are submitted over REST or `/v1/ws`, not here.
//!
//! Limits: the handshake is a call like any other (client and principal budgets, the REST in-flight slots).
//! After it, polls, resubscribes and unsubscribes use the live slots (`max_live_in_flight`,
//! `max_live_per_principal`) and the `seen` call of each client frame uses slots of its own, so pings are not
//! starved; a `seen` with no slot is owed and done at the next look. Each client frame costs the principal's
//! budget (a `subscribe` costs `SUBSCRIBE_COST`, charged once it has a slot), except that `seen` comes first,
//! so a refused frame still shows the client is alive. `ws_idle` does not apply: a client must send a JSON
//! `ping` (a browser cannot send a WebSocket-level ping) within the server's `dead_after_ms`, or the timer drops
//! it. A call with no free slot is retried at the next look; a poll that fails or times out closes the socket.

use crate::LiveState;
use crate::hub::{Conn, LiveHub, Opened, Out};
use axum::{
    body::Body,
    extract::{
        FromRequestParts, Path, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::{IntoResponse, Response},
};
use meldframe_host::Principal;
use meldframe_http::{Reply, Slot, bearer, peer, valid_file};
use meldframe_protocol::{ClientMsg, ServerMsg};
use meldframe_stream::{Frame, StreamId};
use std::sync::Arc;

/// Frames taken from the stream per blocking call.
const BATCH: usize = 64;

/// What a `subscribe` costs the principal's budget, a ping being 1: it can read the store.
const SUBSCRIBE_COST: u32 = 4;

/// The `after` cursor of a query string: absent is 0, anything but a number is refused.
fn after(query: Option<&str>) -> Option<u64> {
    let found = query
        .into_iter()
        .flat_map(|q| q.split('&'))
        .find_map(|kv| kv.strip_prefix("after="));
    found.map_or(Some(0), |v| v.parse().ok())
}

pub(crate) async fn upgrade(
    State(s): State<Arc<LiveState>>,
    Path(file): Path<String>,
    req: Request<Body>,
) -> Response {
    let hub = s.hub.clone();
    let who = match s
        .shared
        .client_ok(&peer(&req))
        .and_then(|()| s.shared.principal(bearer(req.headers())))
    {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    let Some(from) = after(req.uri().query()).filter(|_| valid_file(&file)) else {
        return Reply::bad_request("a file name is 1 to 128 bytes without slashes or control characters, and after is a sequence number").into_response();
    };
    let (mut parts, _) = req.into_parts();
    let Ok(ws) = WebSocketUpgrade::from_request_parts(&mut parts, &()).await else {
        return Reply::bad_request("not a WebSocket upgrade").into_response();
    };
    // Subscribe before the upgrade, so a refusal (the cap, the store) is a status the client can read.
    let (src, name, w, at) = (hub.source.clone(), file.clone(), who.clone(), hub.now());
    let opened = match s
        .shared
        .blocking(&who, move || src.subscribe(&w, &name, from, at))
        .await
    {
        Err(reply) => return reply.into_response(),
        Ok(Err(failure)) => return Reply::failure(&failure).into_response(),
        Ok(Ok(opened)) => opened,
    };
    let max = s.shared.max_body;
    ws.max_message_size(max)
        .max_frame_size(max)
        .on_upgrade(move |socket| serve(socket, s, hub, who, file, opened))
}

fn text(msg: &ServerMsg) -> Message {
    Message::text(serde_json::to_string(msg).unwrap_or_default())
}

fn wire(room: &str, f: Out) -> ServerMsg {
    match f {
        Frame::Durable { seq, data } => ServerMsg::Event {
            room: room.to_owned(),
            seq,
            actor: data.actor,
            key: Some(data.key),
            command: data.command,
        },
        Frame::Ephemeral(m) => m,
        Frame::Resync {
            after_seq,
            latest_seq,
        } => ServerMsg::Resync {
            room: room.to_owned(),
            after_seq,
            latest_seq,
        },
        Frame::Heartbeat => ServerMsg::Heartbeat {},
    }
}

fn error(id: Option<String>, code: &str, message: &str, retryable: bool) -> ServerMsg {
    ServerMsg::Error {
        id,
        message: message.to_owned(),
        retryable,
        code: Some(code.to_owned()),
        retry_after_ms: None,
    }
}

/// A retryable error that says how long to wait.
fn retry_error(
    id: Option<String>,
    code: &str,
    message: &str,
    wait: std::time::Duration,
) -> ServerMsg {
    let ms = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX).max(1);
    ServerMsg::Error {
        id,
        message: message.to_owned(),
        retryable: true,
        code: Some(code.to_owned()),
        retry_after_ms: Some(ms),
    }
}

struct Session {
    s: Arc<LiveState>,
    hub: Arc<LiveHub>,
    who: Principal,
    file: String,
    id: StreamId,
    /// The `afterSeq` of the last `resync` marker sent and not yet answered by a `subscribe`.
    marker: Option<u64>,
    /// When the client was last heard from, if the `seen` call for it found no slot: done at the next look.
    owed_seen: Option<u64>,
}

/// The outcome of a call on the live source.
enum Call<R> {
    Done(R),
    /// No live slot was free: nothing ran, look again later.
    Busy,
    /// It timed out or failed; it may still run, and what it takes is lost to this connection.
    Lost,
}

impl Session {
    /// Send one frame, giving up when the client does not read.
    async fn send(&self, socket: &mut WebSocket, msg: &ServerMsg) -> bool {
        let sent =
            tokio::time::timeout(self.s.shared.ws_send_timeout, socket.send(text(msg))).await;
        matches!(sent, Ok(Ok(())))
    }

    /// Run a call on the live source in a live slot; see [`Session::exec`].
    async fn call<R: Send + 'static>(
        &self,
        job: impl FnOnce(&dyn crate::LiveSource, &Principal, &str, StreamId, u64) -> R + Send + 'static,
    ) -> Call<R> {
        match self.s.live_slots.take(&self.who) {
            None => Call::Busy,
            Some(slot) => self.exec(slot, job).await.map_or(Call::Lost, Call::Done),
        }
    }

    /// Run `job` on the blocking pool holding `slot` (freed when the job ends, not when this stops waiting),
    /// under the timeout; `None` when it timed out or failed.
    async fn exec<R: Send + 'static>(
        &self,
        slot: Slot,
        job: impl FnOnce(&dyn crate::LiveSource, &Principal, &str, StreamId, u64) -> R + Send + 'static,
    ) -> Option<R> {
        let (src, who, file, id, now) = (
            self.hub.source.clone(),
            self.who.clone(),
            self.file.clone(),
            self.id,
            self.hub.now(),
        );
        let run = tokio::task::spawn_blocking(move || {
            let _slot = slot;
            job(&*src, &who, &file, id, now)
        });
        tokio::time::timeout(self.s.shared.timeout, run)
            .await
            .ok()?
            .ok()
    }

    /// The client was heard from at `at`: tell the server, or, with no `seen` slot free, owe it.
    async fn heard(&mut self, at: u64) {
        let Some(slot) = self.s.seen_slots.take(&self.who) else {
            self.owed_seen = Some(self.owed_seen.map_or(at, |o| o.max(at)));
            return;
        };
        if self
            .exec(slot, move |src, who, file, id, _| {
                src.seen(who, file, id, at)
            })
            .await
            .is_some()
        {
            self.owed_seen = None;
        }
    }

    async fn replay(&self, socket: &mut WebSocket, ops: Vec<meldframe_host::StoredOp>) -> bool {
        for op in ops {
            let msg = wire(
                &self.file,
                Frame::Durable {
                    seq: op.seq,
                    data: op,
                },
            );
            if !self.send(socket, &msg).await {
                return false;
            }
        }
        true
    }

    /// Send everything the stream has queued; false when the connection should close: the socket is gone,
    /// or a poll was lost (see [`Call::Lost`]).
    async fn drain(&mut self, socket: &mut WebSocket) -> bool {
        loop {
            let got = match self
                .call(|src, who, file, id, now| src.poll(who, file, id, now, BATCH))
                .await
            {
                Call::Done(got) => got,
                Call::Busy => return true,
                Call::Lost => return false,
            };
            let more = got.len() == BATCH;
            for frame in got {
                if let Frame::Resync { after_seq, .. } = &frame {
                    self.marker = Some(*after_seq);
                }
                if !self.send(socket, &wire(&self.file, frame)).await {
                    return false;
                }
            }
            if !more {
                return true;
            }
        }
    }

    /// `subscribe`: restart the stream from the last marker, never from before it.
    async fn resubscribe(
        &mut self,
        socket: &mut WebSocket,
        id: String,
        since_seq: Option<u64>,
    ) -> bool {
        let Some(marker) = self.marker else {
            let why = "send subscribe only after a resync marker";
            return self
                .send(socket, &error(Some(id), "bad_request", why, false))
                .await;
        };
        // A place first, then the charge: a busy server costs the client one frame, not four.
        let Some(slot) = self.s.live_slots.take(&self.who) else {
            let why = "the server is busy: send subscribe again";
            let msg = retry_error(Some(id), "unavailable", why, self.hub.poll_every);
            return self.send(socket, &msg).await;
        };
        if let Err(wait) = self.s.shared.principal_wait(&self.who, SUBSCRIBE_COST - 1) {
            drop(slot);
            let why = "too many subscribes: wait, then send it again";
            let msg = retry_error(Some(id), "rate_limited", why, wait);
            return self.send(socket, &msg).await;
        }
        let from = since_seq.map_or(marker, |s| s.max(marker));
        let (who, again) = (self.who.clone(), self.id);
        let done = self
            .exec(slot, move |src, _, file, _, now| {
                src.resubscribe(&who, file, again, from, now)
            })
            .await;
        let text = match done {
            Some(Ok(Opened { replay, .. })) => {
                self.marker = None;
                let ok = ServerMsg::Subscribed {
                    id,
                    room: self.file.clone(),
                };
                return self.send(socket, &ok).await && self.replay(socket, replay).await;
            }
            None => return false,
            // The stream is gone or the store failed: say so, close, and the client connects again.
            Some(Err(_)) => {
                "the stream could not be restarted: connect again from your last number"
            }
        };
        let _ = self
            .send(socket, &error(Some(id), "unavailable", text, true))
            .await;
        false
    }

    /// One text frame from the client; false when the connection should close.
    async fn client(&mut self, socket: &mut WebSocket, text: &str) -> bool {
        let msg: ClientMsg = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => {
                let e = error(
                    None,
                    "bad_request",
                    "send a ping or a subscribe message",
                    false,
                );
                return self.send(socket, &e).await;
            }
        };
        if let Some(h) = &self.hub.hook
            && let Some(reply) = h.message(&self.who, &self.file, self.id, &msg)
        {
            return match reply {
                Some(m) => self.send(socket, &m).await,
                None => true,
            };
        }
        match msg {
            ClientMsg::Ping {} => self.send(socket, &ServerMsg::Pong {}).await,
            ClientMsg::Unsubscribe { room } if room == self.file => false,
            ClientMsg::Subscribe {
                id,
                room,
                since_seq,
            } if room == self.file => self.resubscribe(socket, id, since_seq).await,
            other => {
                let why = match other {
                    ClientMsg::Subscribe { .. } | ClientMsg::Unsubscribe { .. } => {
                        "this socket is for the file in its URL"
                    }
                    _ => "send ops over REST or /v1/ws; this socket takes ping and subscribe",
                };
                let id = match other {
                    ClientMsg::Subscribe { id, .. } => Some(id),
                    _ => None,
                };
                self.send(socket, &error(id, "bad_request", why, false))
                    .await
            }
        }
    }

    async fn run(
        &mut self,
        socket: &mut WebSocket,
        conn: &Conn,
        replay: Vec<meldframe_host::StoredOp>,
    ) {
        let hello = ServerMsg::Subscribed {
            id: "connect".into(),
            room: self.file.clone(),
        };
        if !self.send(socket, &hello).await || !self.replay(socket, replay).await {
            return;
        }
        // Up one per limited frame, down one per allowed one, as on /v1/ws.
        let mut limited = 0u32;
        loop {
            // Register for a wake-up before looking, so an op accepted in between is not missed.
            let wake = conn.data.notified();
            tokio::pin!(wake);
            wake.as_mut().enable();
            if let Some(at) = self.owed_seen {
                self.heard(at).await;
            }
            if !self.drain(socket).await {
                return;
            }
            tokio::select! {
                () = &mut wake => {}
                () = tokio::time::sleep(self.hub.poll_every) => {}
                () = conn.dropped.notified() => return,
                got = socket.recv() => {
                    let Some(Ok(msg)) = got else { return };
                    if matches!(msg, Message::Close(_)) {
                        return;
                    }
                    // Proof of life first: a frame the budget refuses still shows the client is there.
                    let at = self.hub.now();
                    self.heard(at).await;
                    if self.s.shared.principal_ok(&self.who).is_err() {
                        limited += 1;
                        if limited >= self.s.shared.ws_max_limited {
                            return;
                        }
                        continue;
                    }
                    limited = limited.saturating_sub(1);
                    if let Message::Text(t) = msg
                        && !self.client(socket, t.as_str()).await
                    {
                        return;
                    }
                }
            }
        }
    }
}

/// What a connection holds on the hub and the server, released when it ends for any reason: a close, an
/// error, a panic, or the task being dropped.
struct Held {
    s: Arc<LiveState>,
    hub: Arc<LiveHub>,
    who: Principal,
    file: String,
    id: StreamId,
    conn: Option<Conn>,
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.hub.deregister(conn);
        }
        if let Some(h) = &self.hub.hook {
            h.closed(self.id);
        }
        // Close the stream in the background, in a live slot. With none free (the server is stuck), the
        // silent stream is dropped by the timer after `dead_after_ms`.
        if let (Some(slot), Ok(rt)) = (
            self.s.live_slots.take(&self.who),
            tokio::runtime::Handle::try_current(),
        ) {
            let (src, who, file, id) = (
                self.hub.source.clone(),
                self.who.clone(),
                self.file.clone(),
                self.id,
            );
            drop(rt.spawn_blocking(move || {
                let _slot = slot;
                src.unsubscribe(&who, &file, id);
            }));
        }
    }
}

async fn serve(
    mut socket: WebSocket,
    s: Arc<LiveState>,
    hub: Arc<LiveHub>,
    who: Principal,
    file: String,
    opened: Opened,
) {
    let held = Held {
        conn: hub.register(&who, &file, opened.id),
        s: s.clone(),
        hub: hub.clone(),
        who: who.clone(),
        file: file.clone(),
        id: opened.id,
    };
    let mut session = Session {
        s,
        hub,
        who,
        file,
        id: opened.id,
        marker: None,
        owed_seen: None,
    };
    match &held.conn {
        Some(conn) => session.run(&mut socket, conn, opened.replay).await,
        None => {
            // The timer dropped the stream between the handshake and now: nothing would ever reach it.
            let why = "the stream ended before the socket was ready: connect again";
            let _ = session
                .send(&mut socket, &error(None, "unavailable", why, true))
                .await;
        }
    }
    let _ = tokio::time::timeout(
        session.s.shared.ws_send_timeout,
        socket.send(Message::Close(None)),
    )
    .await;
}
