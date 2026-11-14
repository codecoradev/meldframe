//! The server: one total order, idempotent per `(actor, key)`, with the dependency rule.

use meldframe_core::{Limits, apply_command};
use meldframe_protocol::Node;
use meldframe_sync::Outgoing;
use std::collections::HashMap;

pub enum Verdict {
    /// Seen before: answered without a new event.
    Duplicate,
    Rejected,
    Accepted {
        seq: u64,
    },
}

#[derive(Default)]
pub struct Counts {
    pub rejects: u64,
    pub dep_rejects: u64,
}

pub struct Server {
    pub nodes: Vec<Node>,
    pub seq: u64,
    pub counts: Counts,
    idem: HashMap<(String, String), u64>,
}

impl Server {
    pub fn new(nodes: Vec<Node>, _seed: u64) -> Self {
        Server {
            nodes,
            seq: 0,
            counts: Counts::default(),
            idem: HashMap::new(),
        }
    }

    pub fn submit(&mut self, actor: &str, o: &Outgoing) -> Verdict {
        let key = (actor.to_string(), o.key.clone());
        if self.idem.contains_key(&key) {
            return Verdict::Duplicate;
        }
        // The dependency rule: every dependency must have been applied.
        if o.deps
            .iter()
            .any(|d| !self.idem.contains_key(&(actor.to_string(), d.clone())))
        {
            self.counts.rejects += 1;
            self.counts.dep_rejects += 1;
            return Verdict::Rejected;
        }
        if apply_command(&mut self.nodes, &o.command, &Limits::default()).is_err() {
            self.counts.rejects += 1;
            return Verdict::Rejected;
        }
        self.seq += 1;
        self.idem.insert(key, self.seq);
        Verdict::Accepted { seq: self.seq }
    }
}
