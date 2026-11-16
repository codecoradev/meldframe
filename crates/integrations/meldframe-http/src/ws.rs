//! WebSocket: `GET /v1/ws`; each text frame is `{"id", "tool", "file", "ops"}` and is answered by one
//! frame `{"id", "status", "accepted" | "error"}`, the body of the REST reply plus the status.

use crate::{
    Shared, WireOp,
    reply::Reply,
    rest::{bearer, peer},
};
use axum::{
    body::Body,
    extract::{
        FromRequestParts, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use meldframe_host::Principal;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    id: String,
    tool: String,
    file: String,
    ops: Vec<WireOp>,
}

pub(crate) async fn upgrade(State(s): State<Arc<Shared>>, req: Request<Body>) -> Response {
    let headers: HeaderMap = req.headers().clone();
    let who = match s
        .client_ok(&peer(&req))
        .and_then(|()| s.authenticate(bearer(&headers)))
    {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    let (mut parts, _) = req.into_parts();
    let Ok(ws) = WebSocketUpgrade::from_request_parts(&mut parts, &()).await else {
        return Reply::bad_request("not a WebSocket upgrade").into_response();
    };
    let max = s.max_body;
    ws.max_message_size(max)
        .max_frame_size(max)
        .on_upgrade(move |socket| serve(socket, s, who))
}

fn encode(id: Option<&str>, reply: &Reply) -> Message {
    let mut v = reply.body.clone();
    if let Value::Object(m) = &mut v {
        m.insert("id".into(), id.map_or(Value::Null, Value::from));
        m.insert("status".into(), reply.status.as_u16().into());
    }
    Message::text(v.to_string())
}

/// Send one frame, giving up (and so dropping the connection) when the client does not read.
async fn send(socket: &mut WebSocket, s: &Shared, msg: Message) -> bool {
    matches!(
        tokio::time::timeout(s.ws_send_timeout, socket.send(msg)).await,
        Ok(Ok(()))
    )
}

async fn serve(mut socket: WebSocket, s: Arc<Shared>, who: Principal) {
    // Up one per limited frame or ping, down one per allowed frame; the connection closes at the maximum.
    let mut limited = 0u32;
    // Pings and pongs do not move the idle deadline: only frames that carry work do.
    let mut deadline = tokio::time::Instant::now() + s.ws_idle;
    loop {
        let msg = match tokio::time::timeout_at(deadline, socket.recv()).await {
            Ok(Some(Ok(msg))) => msg,
            Ok(_) => return,
            Err(_) => break,
        };
        let (id, reply) = match msg {
            Message::Close(_) => return,
            Message::Ping(_) | Message::Pong(_) => {
                if s.principal_ok(&who).is_err() {
                    limited += 1;
                    if limited >= s.ws_max_limited {
                        break;
                    }
                }
                continue;
            }
            other => {
                deadline = tokio::time::Instant::now() + s.ws_idle;
                match s.principal_ok(&who) {
                    Err(refused) => {
                        limited += 1;
                        (None, refused)
                    }
                    Ok(()) => {
                        limited = limited.saturating_sub(1);
                        handle(&s, &who, other).await
                    }
                }
            }
        };
        if !send(&mut socket, &s, encode(id.as_deref(), &reply)).await {
            return;
        }
        if limited >= s.ws_max_limited {
            break;
        }
    }
    let _ = send(&mut socket, &s, Message::Close(None)).await;
}

async fn handle(s: &Shared, who: &Principal, msg: Message) -> (Option<String>, Reply) {
    let Message::Text(text) = msg else {
        return (None, Reply::bad_request("send JSON text frames"));
    };
    if text.len() > s.max_body {
        return (None, Reply::too_large(s.max_body));
    }
    match serde_json::from_str::<Frame>(text.as_str()) {
        Err(_) => (
            None,
            Reply::bad_request(
                "a frame is {\"id\", \"tool\", \"file\", \"ops\": [{\"key\", \"command\"}]}",
            ),
        ),
        Ok(f) => {
            let reply = s.submit(who.clone(), f.tool, f.file, f.ops).await;
            (Some(f.id), reply)
        }
    }
}
