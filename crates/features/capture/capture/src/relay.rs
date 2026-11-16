//! The capture relay state machine.

use std::collections::{BTreeMap, BTreeSet};

use meldframe_protocol::ServerMsg;

use crate::{CaptureError, Token};

const CAPABILITY: &str = "capture";
const MAX_SCALE: f64 = 4.0;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Limits of the relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Largest PNG accepted, in bytes.
    pub max_bytes: usize,
    /// Most pixels the client may render (sent in the request).
    pub max_pixels: u64,
    /// Time a client has to answer, in milliseconds.
    pub timeout_ms: u64,
    /// Most requests pending at once; more is `busy`.
    pub max_pending: usize,
    /// Most requests pending on one client; more is `busy`.
    pub max_pending_per_client: usize,
    /// Most capable clients registered at once; more is `busy`.
    pub max_clients: usize,
}

/// One capture to start.
#[derive(Clone, PartialEq)]
pub struct Begin {
    /// Workspace of the calling principal.
    pub workspace: String,
    /// The file to capture (already resolved through the session binding).
    pub file: String,
    /// The node, or `None` for the whole page.
    pub node_id: Option<String>,
    /// Render scale, above 0 and at most 4.
    pub scale: f64,
    /// Chosen by the caller, unique among the pending requests of the workspace.
    pub request_id: String,
    /// Secret the upload must present; chosen by the caller, unguessable.
    pub upload_token: Token,
    /// The PNG must reflect the file at this sequence number or later.
    pub min_seq: u64,
}

impl std::fmt::Debug for Begin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Begin")
            .field("workspace", &self.workspace)
            .field("file", &self.file)
            .field("request_id", &self.request_id)
            .field("upload_token", &self.upload_token)
            .finish_non_exhaustive()
    }
}

/// A finished request: the PNG, or why there is none.
#[derive(Clone, PartialEq)]
pub struct Done {
    /// The workspace of the request.
    pub workspace: String,
    /// The request that finished.
    pub request_id: String,
    /// The image bytes or the error.
    pub result: Result<Vec<u8>, CaptureError>,
}

impl std::fmt::Debug for Done {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = match &self.result {
            Ok(png) => format!("Ok(<{} bytes>)", png.len()),
            Err(e) => format!("Err({e:?})"),
        };
        write!(
            f,
            "Done {{ request_id: {:?}, result: {r} }}",
            self.request_id
        )
    }
}

#[derive(Debug)]
struct Pending<C> {
    client: C,
    token: Token,
    min_seq: u64,
    deadline: u64,
}

type Key = (String, String);

/// Pending requests and the capture-capable clients, generic over the client (stream) id.
#[derive(Debug)]
pub struct Relay<C> {
    cfg: Config,
    clients: BTreeMap<Key, BTreeSet<C>>,
    load: BTreeMap<C, usize>,
    pending: BTreeMap<Key, Pending<C>>,
}

impl<C: Copy + Ord> Relay<C> {
    /// An empty relay.
    pub fn new(cfg: Config) -> Self {
        Relay {
            cfg,
            clients: BTreeMap::new(),
            load: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// Requests waiting for an answer.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// The largest upload accepted; the HTTP layer enforces it while streaming the body.
    pub fn max_bytes(&self) -> usize {
        self.cfg.max_bytes
    }

    /// A client advertised its capabilities for a file of a workspace; it is capable when the list names
    /// `capture`. Advertising again replaces the earlier list. A new client over `max_clients` is `busy`.
    pub fn set_capabilities(
        &mut self,
        workspace: &str,
        file: &str,
        client: C,
        caps: &[String],
    ) -> Result<(), CaptureError> {
        let key = (workspace.to_string(), file.to_string());
        if caps.iter().any(|c| c == CAPABILITY) {
            let known = self.clients.get(&key).is_some_and(|s| s.contains(&client));
            let total: usize = self.clients.values().map(BTreeSet::len).sum();
            if !known && total >= self.cfg.max_clients {
                return Err(CaptureError::Busy);
            }
            self.clients.entry(key).or_default().insert(client);
        } else if let Some(set) = self.clients.get_mut(&key) {
            set.remove(&client);
            if set.is_empty() {
                self.clients.remove(&key);
            }
        }
        Ok(())
    }

    /// Start a capture: pick the capable client with the fewest pending requests and return it with the
    /// ephemeral frame to send it. `busy` when the relay or that client is at its limit.
    pub fn begin(&mut self, now: u64, b: Begin) -> Result<(C, ServerMsg), CaptureError> {
        if !(b.scale.is_finite() && b.scale > 0.0 && b.scale <= MAX_SCALE) {
            return Err(CaptureError::Invalid(format!(
                "scale must be above 0 and at most {MAX_SCALE}"
            )));
        }
        let key = (b.workspace.clone(), b.request_id.clone());
        if self.pending.contains_key(&key) {
            return Err(CaptureError::Invalid(
                "request id is already pending".into(),
            ));
        }
        let load = |c: &C| self.load.get(c).copied().unwrap_or(0);
        let client = self
            .clients
            .get(&(b.workspace.clone(), b.file.clone()))
            .and_then(|set| set.iter().min_by_key(|c| (load(c), **c)))
            .copied()
            .ok_or(CaptureError::NoCapableClient)?;
        if self.pending.len() >= self.cfg.max_pending
            || load(&client) >= self.cfg.max_pending_per_client
        {
            return Err(CaptureError::Busy);
        }
        let msg = ServerMsg::CaptureRequest {
            request_id: b.request_id,
            room: b.file,
            node_id: b.node_id,
            scale: b.scale,
            max_pixels: self.cfg.max_pixels,
            min_seq: b.min_seq,
            upload_token: b.upload_token.as_str().to_string(),
            timeout_ms: self.cfg.timeout_ms,
        };
        *self.load.entry(client).or_default() += 1;
        self.pending.insert(
            key,
            Pending {
                client,
                token: b.upload_token,
                min_seq: b.min_seq,
                deadline: now.saturating_add(self.cfg.timeout_ms),
            },
        );
        Ok((client, msg))
    }

    /// A PNG arrived over HTTP for `request_id`, from a caller of `workspace` holding `token`, rendered at
    /// sequence number `seq`. Unknown request, wrong token or wrong workspace: ignored (`None`). The length
    /// is checked before the bytes are copied.
    pub fn upload(
        &mut self,
        workspace: &str,
        request_id: &str,
        token: &str,
        seq: u64,
        png: &[u8],
    ) -> Option<Done> {
        let key = (workspace.to_string(), request_id.to_string());
        let p = self.pending.get(&key)?;
        if !p.token.matches(token) {
            return None;
        }
        let min_seq = p.min_seq;
        let result = if png.len() > self.cfg.max_bytes {
            Err(CaptureError::TooLarge)
        } else {
            self.check_png(png, seq, min_seq).map(|()| png.to_vec())
        };
        self.finish(&key, result)
    }

    fn check_png(&self, png: &[u8], seq: u64, min_seq: u64) -> Result<(), CaptureError> {
        let bad = |m: &str| CaptureError::RenderFailed(m.to_string());
        if png.len() < 33 || !png.starts_with(PNG_MAGIC) || &png[12..16] != b"IHDR" {
            return Err(bad("the upload is not a PNG"));
        }
        let dim = |at: usize| u64::from(u32::from_be_bytes(png[at..at + 4].try_into().unwrap()));
        let (w, h) = (dim(16), dim(20));
        if w == 0 || h == 0 {
            return Err(bad("the PNG has a zero dimension"));
        }
        if w * h > self.cfg.max_pixels {
            return Err(CaptureError::TooLarge);
        }
        if seq < min_seq {
            return Err(CaptureError::StaleClient);
        }
        Ok(())
    }

    /// The client refused (`ClientMsg::CaptureFailed`). Only the client the request went to is heard.
    pub fn failed(
        &mut self,
        workspace: &str,
        client: C,
        request_id: &str,
        code: &str,
        message: &str,
    ) -> Option<Done> {
        let key = (workspace.to_string(), request_id.to_string());
        if self.pending.get(&key)?.client != client {
            return None;
        }
        self.finish(&key, Err(CaptureError::from_client(code, message)))
    }

    /// Fail every request whose deadline is at or before `now`.
    pub fn expire(&mut self, now: u64) -> Vec<Done> {
        let due: Vec<Key> = self
            .pending
            .iter()
            .filter(|(_, p)| p.deadline <= now)
            .map(|(k, _)| k.clone())
            .collect();
        due.iter()
            .filter_map(|k| self.finish(k, Err(CaptureError::Timeout)))
            .collect()
    }

    /// A stream closed: it is no longer capable, and the requests sent to it fail.
    pub fn disconnect(&mut self, client: C) -> Vec<Done> {
        self.clients.retain(|_, set| {
            set.remove(&client);
            !set.is_empty()
        });
        let lost: Vec<Key> = self
            .pending
            .iter()
            .filter(|(_, p)| p.client == client)
            .map(|(k, _)| k.clone())
            .collect();
        lost.iter()
            .filter_map(|k| self.finish(k, Err(CaptureError::ClientDisconnected)))
            .collect()
    }

    fn finish(&mut self, key: &Key, result: Result<Vec<u8>, CaptureError>) -> Option<Done> {
        let p = self.pending.remove(key)?;
        if let Some(n) = self.load.get_mut(&p.client) {
            *n -= 1;
            if *n == 0 {
                self.load.remove(&p.client);
            }
        }
        Some(Done {
            workspace: key.0.clone(),
            request_id: key.1.clone(),
            result,
        })
    }
}
