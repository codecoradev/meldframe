//! The hook on the live sockets: an editor announces `capture`, refuses a request, or goes away.

use crate::hub::Hub;
use meldframe_host::Principal;
use meldframe_http_live::ClientHook;
use meldframe_protocol::{ClientMsg, ServerMsg};
use meldframe_stream::StreamId;
use std::sync::Arc;

/// What an editor is told when it is refused as one capable client too many.
fn busy() -> ServerMsg {
    ServerMsg::Error {
        id: None,
        message: "too many editors that can capture are open for this workspace: close one and announce again"
            .into(),
        retryable: true,
        code: Some("busy".into()),
        retry_after_ms: None,
    }
}

#[derive(Debug)]
pub(crate) struct Hook(pub(crate) Arc<Hub>);

impl ClientHook for Hook {
    fn message(
        &self,
        who: &Principal,
        file: &str,
        id: StreamId,
        msg: &ClientMsg,
    ) -> Option<Option<ServerMsg>> {
        match msg {
            ClientMsg::Capabilities { room, capabilities } if room == file => {
                let done = self.0.register(&who.workspace, file, id, capabilities);
                Some(done.err().map(|_| busy()))
            }
            ClientMsg::CaptureFailed {
                request_id,
                code,
                message,
            } => {
                self.0.run(|r| {
                    let done = r.failed(&who.workspace, id, request_id, code, message);
                    done.into_iter().collect()
                });
                Some(None)
            }
            _ => None,
        }
    }

    fn closed(&self, id: StreamId) {
        self.0.run(|r| r.disconnect(id));
    }

    fn tick(&self, now: u64) {
        self.0.run(|r| r.expire(now));
    }
}
