//! The state the three parts share: the relay, the callers waiting on it and the session bindings.

use capture::{Begin, Binding, CaptureError, Config, Done, Relay, Token};
use meldframe_host::Principal;
use meldframe_http::mcp::Reader;
use meldframe_http::{Clock, Shared, Slots};
use meldframe_http_live::LiveSource;
use meldframe_protocol::ServerMsg;
use meldframe_stream::StreamId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::oneshot;

/// Most sessions with a bound file kept; a new caller over it gets `busy`.
const MAX_BINDINGS: usize = 4096;
/// How long past the deadline a call waits for the timer before it expires the request itself.
const GRACE: Duration = Duration::from_millis(250);

pub(crate) type Answer = Result<Vec<u8>, CaptureError>;
type Key = (String, String);

struct State {
    relay: Relay<StreamId>,
    waiters: HashMap<Key, oneshot::Sender<Answer>>,
    bindings: HashMap<Key, Binding>,
}

pub(crate) struct Hub {
    state: Mutex<State>,
    pub(crate) shared: Arc<Shared>,
    pub(crate) source: Arc<dyn LiveSource>,
    pub(crate) reader: Arc<dyn Reader>,
    clock: Clock,
    timeout: Duration,
    pub(crate) uploads: Slots,
    pub(crate) policy: Option<meldframe_module::Policy>,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub").finish_non_exhaustive()
    }
}

/// `n` random bytes as lower-case hex.
pub(crate) fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("the system has randomness");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Hub {
    pub(crate) fn new(
        shared: Arc<Shared>,
        source: Arc<dyn LiveSource>,
        reader: Arc<dyn Reader>,
        clock: Clock,
        cfg: Config,
        options: crate::Options,
    ) -> Hub {
        Hub {
            state: Mutex::new(State {
                relay: Relay::new(cfg),
                waiters: HashMap::new(),
                bindings: HashMap::new(),
            }),
            shared,
            source,
            reader,
            clock,
            timeout: Duration::from_millis(cfg.timeout_ms),
            uploads: Slots::new(options.uploads, options.uploads_per_principal),
            policy: options.policy,
        }
    }

    pub(crate) fn now(&self) -> u64 {
        (self.clock)()
    }

    fn st(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Run `f` on the relay and hand every request it finished to its caller; returns them too.
    pub(crate) fn run(&self, f: impl FnOnce(&mut Relay<StreamId>) -> Vec<Done>) -> Vec<Done> {
        let mut st = self.st();
        let done = f(&mut st.relay);
        for d in &done {
            if let Some(tx) = st
                .waiters
                .remove(&(d.workspace.clone(), d.request_id.clone()))
            {
                let _ = tx.send(d.result.clone());
            }
        }
        done
    }

    /// An editor announced its capabilities for `file`; `busy` when its workspace has too many.
    pub(crate) fn register(
        &self,
        workspace: &str,
        file: &str,
        id: StreamId,
        caps: &[String],
    ) -> Result<(), CaptureError> {
        self.st().relay.set_capabilities(workspace, file, id, caps)
    }

    /// Read the relay without finishing anything.
    pub(crate) fn with_relay<T>(&self, f: impl FnOnce(&Relay<StreamId>) -> T) -> T {
        f(&self.st().relay)
    }

    /// The file this caller captures: the bound one, or the one named when nothing is bound yet.
    pub(crate) fn file_of(
        &self,
        who: &Principal,
        named: Option<&str>,
    ) -> Result<String, CaptureError> {
        let key = (who.workspace.clone(), who.actor.clone());
        match self.st().bindings.get(&key) {
            Some(b) => b.resolve(named).map(str::to_owned),
            None => named.map(str::to_owned).ok_or(CaptureError::NoFileSelected),
        }
    }

    /// Bind the caller to `file`, once an editor was found for it. A full table binds nothing.
    pub(crate) fn bind(&self, who: &Principal, file: &str) {
        let key = (who.workspace.clone(), who.actor.clone());
        let mut st = self.st();
        if st.bindings.contains_key(&key) || st.bindings.len() < MAX_BINDINGS {
            let _ = st.bindings.entry(key).or_default().bind(file);
        }
    }

    /// Fail one request and nothing else.
    pub(crate) fn abort(&self, workspace: &str, request_id: &str, why: CaptureError) {
        self.run(|r| r.abort(workspace, request_id, why).into_iter().collect());
    }

    /// Start a capture: the editor to send to, the frame to send and where the answer will arrive.
    pub(crate) fn begin(
        &self,
        who: &Principal,
        file: &str,
        node_id: Option<String>,
        scale: f64,
        min_seq: u64,
    ) -> Result<(StreamId, ServerMsg, String, oneshot::Receiver<Answer>), CaptureError> {
        let request_id = format!("cap_{}", random_hex(8));
        let token = Token::new(&random_hex(16))?;
        let mut st = self.st();
        let (client, msg) = st.relay.begin(
            self.now(),
            Begin {
                workspace: who.workspace.clone(),
                actor: who.actor.clone(),
                file: file.to_owned(),
                node_id,
                scale,
                request_id: request_id.clone(),
                upload_token: token,
                min_seq,
            },
        )?;
        let (tx, rx) = oneshot::channel();
        st.waiters
            .insert((who.workspace.clone(), request_id.clone()), tx);
        Ok((client, msg, request_id, rx))
    }

    /// Wait for the answer. The live timer expires requests; if it does not run, the call does it itself.
    pub(crate) async fn wait(&self, mut rx: oneshot::Receiver<Answer>) -> Answer {
        match tokio::time::timeout(self.timeout + GRACE, &mut rx).await {
            Ok(Ok(answer)) => answer,
            Ok(Err(_)) => Err(CaptureError::Timeout),
            Err(_) => {
                let now = self.now();
                self.run(|r| r.expire(now));
                rx.try_recv().unwrap_or(Err(CaptureError::Timeout))
            }
        }
    }
}

/// Aborts its request when dropped: a caller that goes away (the MCP call is cancelled) frees the editor's
/// place at once. A request that already finished is not pending, so the abort does nothing.
pub(crate) struct Cancel<'a> {
    pub(crate) hub: &'a Hub,
    pub(crate) workspace: String,
    pub(crate) request_id: String,
}

impl Drop for Cancel<'_> {
    fn drop(&mut self) {
        self.hub
            .abort(&self.workspace, &self.request_id, CaptureError::Timeout);
    }
}
