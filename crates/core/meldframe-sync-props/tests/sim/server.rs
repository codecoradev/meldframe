//! The server: one total order, idempotent per `(actor, key)`, with the dependency rule, and checks
//! on every undo or redo op it accepts.
//!
//! - I4: an undo or redo must not change a field whose last writer is another actor whose write the
//!   op's author had already seen (`write seq <= seen_seq`). A concurrent write it had not seen is
//!   counted, not an error. A `Reparent` that changes the parent is not checked for `pos`: `pos` has
//!   no meaning across parents.
//! - I8: a value an undo or redo writes into a field with `UpdateNode` was once the value of that
//!   field on the server (or the initial value). A value that never existed means the inverse was
//!   worked out from a premise the server refused.

use meldframe_core::{Limits, apply_command};
use meldframe_protocol::{Command, Node};
use meldframe_sync::{OpKind, Outgoing};
use serde_json::Value;
use std::collections::HashMap;

pub type Field = (String, String);

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
    pub ur_accepted: u64,
    pub i4_checked: u64,
    pub i4_concurrent: u64,
    pub i8_checked: u64,
}

pub struct Server {
    pub nodes: Vec<Node>,
    pub seq: u64,
    pub counts: Counts,
    seed: u64,
    idem: HashMap<(String, String), u64>,
    last: HashMap<Field, (u64, String)>,
    values: HashMap<Field, Vec<Value>>,
}

fn json(n: &Node) -> Value {
    serde_json::to_value(n).unwrap()
}

fn find(nodes: &[Node], id: &str) -> Option<Value> {
    nodes.iter().find(|n| n.id == id).map(json)
}

fn patch_keys(c: &Command) -> Vec<String> {
    match c {
        Command::UpdateNode { patch, .. } => match serde_json::to_value(patch).unwrap() {
            Value::Object(m) => m.keys().cloned().collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// The fields an op writes, for I4 (edits of a node's fields and its place among siblings).
fn writes(c: &Command) -> Vec<Field> {
    let f = |id: &str, k: &str| (id.to_string(), k.to_string());
    match c {
        Command::UpdateNode { id, .. } => patch_keys(c).iter().map(|k| f(id, k)).collect(),
        Command::Reparent { id, .. } => vec![f(id, "parentId"), f(id, "pos")],
        Command::Reorder { id, .. } => vec![f(id, "pos")],
        Command::AddNode { node, .. } => json(node)
            .as_object()
            .unwrap()
            .keys()
            .map(|k| f(&node.id, k))
            .collect(),
        Command::RemoveNode { .. } => vec![],
    }
}

impl Server {
    pub fn new(nodes: Vec<Node>, seed: u64) -> Self {
        let mut s = Server {
            nodes,
            seq: 0,
            counts: Counts::default(),
            seed,
            idem: HashMap::new(),
            last: HashMap::new(),
            values: HashMap::new(),
        };
        let all: Vec<Node> = s.nodes.clone();
        all.iter().for_each(|n| s.remember(n));
        s
    }

    fn remember(&mut self, n: &Node) {
        for (k, v) in json(n).as_object().unwrap() {
            let e = self.values.entry((n.id.clone(), k.clone())).or_default();
            if !e.contains(v) {
                e.push(v.clone());
            }
        }
    }

    /// I8: every value an undo or redo writes with `UpdateNode` was a value of that field before.
    fn check_i8(&mut self, actor: &str, o: &Outgoing) {
        let Command::UpdateNode { id, patch } = &o.command else {
            return;
        };
        if !self.nodes.iter().any(|n| n.id == *id) {
            return;
        }
        for (k, v) in serde_json::to_value(patch).unwrap().as_object().unwrap() {
            if k == "image" {
                continue;
            }
            self.counts.i8_checked += 1;
            let known = self.values.get(&(id.clone(), k.clone()));
            assert!(
                known.is_some_and(|vs| vs.contains(v)),
                "seed {}: I8: {:?} of {actor} writes {id}.{k} = {v}, which was never its value on the server",
                self.seed,
                o.kind
            );
        }
    }

    /// I4 for an accepted undo or redo; `before` is the tree before the op.
    fn check_i4(&mut self, actor: &str, o: &Outgoing, before: &[Node]) {
        let parent_changed = matches!(&o.command, Command::Reparent { id, .. }
            if find(before, id).map(|v| v["parentId"].clone()) != find(&self.nodes, id).map(|v| v["parentId"].clone()));
        for w in writes(&o.command) {
            if parent_changed && w.1 == "pos" {
                continue;
            }
            let same = find(before, &w.0).and_then(|v| v.get(&w.1).cloned())
                == find(&self.nodes, &w.0).and_then(|v| v.get(&w.1).cloned());
            self.counts.i4_checked += 1;
            if let Some((wseq, wactor)) = self.last.get(&w)
                && wactor != actor
                && !same
            {
                assert!(
                    *wseq > o.seen_seq,
                    "seed {}: I4: {:?} of {actor} (seen {}) overwrites {:?} written by {wactor} at seq {wseq}",
                    self.seed,
                    o.command,
                    o.seen_seq,
                    w
                );
                self.counts.i4_concurrent += 1;
            }
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
        let undoing = o.kind != OpKind::Edit;
        if undoing {
            self.check_i8(actor, o);
        }
        let before = self.nodes.clone();
        if apply_command(&mut self.nodes, &o.command, &Limits::default()).is_err() {
            self.counts.rejects += 1;
            return Verdict::Rejected;
        }
        self.seq += 1;
        self.idem.insert(key, self.seq);
        if undoing {
            self.counts.ur_accepted += 1;
            if !matches!(
                o.command,
                Command::AddNode { .. } | Command::RemoveNode { .. }
            ) {
                self.check_i4(actor, o, &before);
            }
        }
        for n in self.nodes.clone().iter() {
            let id = match &o.command {
                Command::UpdateNode { id, .. }
                | Command::Reparent { id, .. }
                | Command::Reorder { id, .. } => id.as_str(),
                Command::AddNode { node, .. } => node.id.as_str(),
                Command::RemoveNode { .. } => "",
            };
            if n.id == id {
                self.remember(n);
            }
        }
        for w in writes(&o.command) {
            self.last.insert(w, (self.seq, actor.to_string()));
        }
        if matches!(o.command, Command::RemoveNode { .. }) {
            let alive: std::collections::HashSet<&str> =
                self.nodes.iter().map(|n| n.id.as_str()).collect();
            self.last.retain(|(id, _), _| alive.contains(id.as_str()));
        }
        Verdict::Accepted { seq: self.seq }
    }
}
