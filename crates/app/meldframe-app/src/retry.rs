//! A refusal the server calls retryable (a quota, a dependency still pending): keep the op and send it
//! again with the same key after a backoff, a few times, then give up and roll it back. The count and the
//! clock are `meldframe_app_rules::retry::Retries`.

use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use crate::state::Connection;
use meldframe_app_parts::document::{out_ops, removal_of};
use meldframe_app_rules::retry::Refusal;

impl App {
    /// The server refused op `key`. Returns true when the op was rolled back (the view changed).
    pub(crate) fn refused(
        &mut self,
        now_ms: u64,
        key: String,
        code: String,
        retryable: bool,
        fx: &mut Vec<Effect>,
    ) -> bool {
        if !self.state.pending.contains(&key) {
            return false;
        }
        let live = self.state.connection == Connection::Live;
        match self.retries.on_refused(&key, now_ms, retryable, live) {
            Refusal::Retry { attempt, after_ms } => {
                self.events.push(Event::OpRetry { key, attempt });
                fx.push(Effect::WakeAfter { after_ms });
                return false;
            }
            Refusal::Later => return false,
            Refusal::Reject => {}
        }
        let node = removal_of(self.replica.unconfirmed(), &key);
        if !self.replica.on_reject(&key) {
            return false;
        }
        self.events.push(Event::OpRejected {
            key,
            code,
            node,
            retryable,
        });
        true
    }

    /// Send the ops whose retry is due.
    pub(crate) fn send_due_retries(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        if !self.retries.has_due() || self.state.connection != Connection::Live {
            return;
        }
        let due = self.retries.take_due(now_ms);
        if due.is_empty() {
            return;
        }
        let ops: Vec<_> = self
            .replica
            .unconfirmed()
            .into_iter()
            .filter(|o| due.contains(&o.key))
            .collect();
        if !ops.is_empty() {
            fx.push(Effect::SendOps { ops: out_ops(ops) });
        }
    }

    /// Forget the retry state of ops that are no longer pending.
    pub(crate) fn prune_retries(&mut self) {
        self.retries.prune(&self.state.pending);
    }
}
