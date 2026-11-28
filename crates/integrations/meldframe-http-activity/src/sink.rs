//! The batch sink: the header that `apply_commands` leaves after a call, and the frame that announces it.

use crate::service::Publish;
use crate::wire;
use activity_store::BatchStore;
use meldframe_activity::{BatchCall, BatchSink, Marked, NewBatch};
use meldframe_host::Principal;
use meldframe_http::mcp::Reader;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct Sink {
    pub(crate) store: Arc<dyn BatchStore>,
    pub(crate) reader: Arc<dyn Reader>,
    pub(crate) live: Option<Publish>,
}

impl std::fmt::Debug for Sink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sink").finish_non_exhaustive()
    }
}

/// The server clock in milliseconds.
pub(crate) fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl BatchSink for Sink {
    fn record(
        &self,
        workspace: &str,
        actor: &str,
        call: &BatchCall,
        done: Option<&[(u64, bool)]>,
    ) -> Option<Marked> {
        let principal = Principal {
            workspace: workspace.to_owned(),
            actor: actor.to_owned(),
        };
        let seen: Vec<(u64, bool)> = match done {
            Some(d) => d.to_vec(),
            None => {
                let keys: Vec<String> = (0..call.commands)
                    .map(|i| format!("{}:{i}", call.key))
                    .collect();
                let found = self.reader.written(&principal, &call.file, &keys);
                let found = found.map_err(|e| eprintln!("meldframe-http-activity: {e:?}"));
                found.ok()?.into_iter().map(|s| (s, false)).collect()
            }
        };
        let (found, fresh) = (seen.len(), seen.iter().filter(|s| s.1).count());
        let first = seen.iter().map(|s| s.0).min()?;
        let last = seen.iter().map(|s| s.0).max()?;
        let found = u32::try_from(found).ok()?;
        let batch = NewBatch {
            actor: actor.to_owned(),
            key_prefix: call.key.clone(),
            tool: call.tool.into(),
            commands: u32::try_from(call.commands).ok()?,
            written: u32::try_from(fresh).ok()?,
            // The keys of the call the log holds: the count never passes it, and a replay racing the original it
            // replays (or a retry that fails again) is counted once, whichever reaches the store first.
            known: Some(found),
            atomic: call.atomic,
            first_seq: first,
            last_seq: last,
            at_ms: now_ms(),
            note: call.note.clone(),
        };
        let kept = |e| eprintln!("meldframe-http-activity: no header kept: {e}");
        let r = (self.store.record(workspace, &call.file, &batch))
            .map_err(kept)
            .ok()?;
        if let (true, Some(live)) = (r.changed, &self.live) {
            // After the call's op events (the call has returned from the room). A drop is silent: the list is the truth.
            live(
                &principal,
                &call.file,
                wire::frame(&call.file, &r.header, r.unreviewed),
            );
        }
        Some(Marked {
            n: r.header.n,
            written: found,
            note: r.note.as_str(),
        })
    }
}
