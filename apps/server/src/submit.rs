//! Submitting ops on behalf of an agent tool: the tool check, the gate, the room and the live streams, in that order.

use crate::Server;
use crate::clock::now;
use crate::error::SubmitError;
use crate::limits;
use crate::tools::{check_tool, known_auto_tool};
use meldframe_host::{Accepted, Op, Principal, Room, Store, StoredOp};
use meldframe_rooms::TreeFacts;
use meldframe_store_pg::{AnyStore, room_key};

impl Server {
    /// Submit ops on behalf of an agent tool to `file` of the principal's workspace. The registry's tier
    /// for the tool gives its policy and is an upper bound on the tier of every op: a Read tool cannot
    /// change data and a Change tool cannot delete. A tool the registry does not know is `Deny` (fail
    /// closed), and `Ask` counts as forbidden until approval exists, as in the gate. The gate then applies
    /// its own tier policy to each op and is the only source of a permit. The tree of the file's room, as it
    /// is now, tells the gate which deletes are small (ADR-0010): a small delete is a Change, so a Change
    /// tool may submit it. Each admitted op is then accepted by the room of `file`, in order, and recorded
    /// in the activity log. Returns the accepted ops (`written: false` for a retried key).
    ///
    /// An unknown or denied tool is refused before any room is opened, and a room this call opened is
    /// closed again when the call fails, so refused calls cannot fill the registry; idle rooms are closed
    /// first. An op whose key was already applied with the same command is a duplicate: it is answered with
    /// its first sequence number and skips the tier decision (the tree it was decided on may have changed,
    /// as after a delete) and the gate, since nothing is written.
    ///
    /// Known limit: the gate counts quota when it admits a batch, before the room accepts. A batch that
    /// stops at a failing op leaves the later ops charged and unwritten, and a retry is charged again (rate,
    /// storage and total alike); refunding needs a host change. The quotas are checked after the file is open.
    pub fn submit_tool(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, SubmitError> {
        let done = self.submit_tool_unlogged(tool, file, who, ops);
        self.record_call(tool, file, who, done.as_ref().err());
        done
    }

    fn submit_tool_unlogged(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, SubmitError> {
        limits::valid_names(&who.workspace, file)?;
        let at = now();
        for key in self.rooms.close_idle(at, self.idle_after) {
            self.open_keys.remove(&key);
        }
        known_auto_tool(&self.registry, tool, &ops)?;
        limits::precheck(self.registry.agent_tool(tool).map(|t| t.tier), &ops)?;
        let key = room_key(&who.workspace, file);
        let opened = self.open_room(&who.workspace, &key, at)?;
        self.open_keys.insert(key.clone());
        let done = self.write(tool, file, &key, who, ops, at);
        if done.is_err() && !opened.already_open {
            self.rooms.close(&key);
            self.open_keys.remove(&key);
        }
        done
    }

    /// The checks of `submit_tool` that need no room: names, batch size, a known tool with an `Auto` policy,
    /// and a Read tool with an op that is a write whatever the tree. A caller that opens the room itself
    /// first (an atomic batch) runs this before opening it.
    pub fn precheck(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: &[Op],
    ) -> Result<(), SubmitError> {
        limits::valid_names(&who.workspace, file)?;
        known_auto_tool(&self.registry, tool, ops)?;
        limits::precheck(self.registry.agent_tool(tool).map(|t| t.tier), ops)?;
        Ok(())
    }

    fn write(
        &mut self,
        tool: &str,
        file: &str,
        key: &str,
        who: &Principal,
        ops: Vec<Op>,
        at: u64,
    ) -> Result<Vec<Accepted>, SubmitError> {
        let Server {
            registry,
            gate,
            approvals,
            rooms,
            live,
            ..
        } = self;
        approvals.prune(at);
        // An approval covers one op: only a call of one op as sent may ask (a larger call is refused whole).
        let single = ops.len() == 1;
        let mut done: Vec<Option<Accepted>> = ops
            .iter()
            .map(|op| {
                rooms
                    .with_room(key, |room| duplicate(room, &who.actor, op))
                    .flatten()
            })
            .collect();
        let fresh: Vec<Op> = ops
            .into_iter()
            .zip(&done)
            .filter(|(_, d)| d.is_none())
            .map(|(op, _)| op)
            .collect();
        if fresh.is_empty() && !done.is_empty() {
            return Ok(done.into_iter().flatten().collect());
        }
        let mut sent = fresh.clone().into_iter();
        let report = rooms
            .with_room(key, |room| {
                let facts = TreeFacts::new(room.nodes());
                // A delete of a node that is not in the file asks nobody: it keeps the refusal it had.
                let ask = single
                    && match fresh.as_slice() {
                        [
                            Op {
                                command: meldframe_protocol::Command::RemoveNode { id },
                                ..
                            },
                        ] => room.nodes().iter().any(|n| n.id == *id),
                        other => other.len() == 1,
                    };
                check_tool(registry, tool, &fresh, &facts, ask)?;
                if let (true, [op]) = (ask, fresh.as_slice()) {
                    approvals
                        .submit(gate, who, file, op.clone(), &facts, at)
                        .map_err(SubmitError::from)
                } else {
                    Ok(gate.submit_with(who, fresh, &facts)?)
                }
            })
            .expect("the room was just opened")?;
        let mut written = report.admitted.into_iter();
        // The nodes the batch wrote and its newest number, for one touch hint.
        let (mut touched, mut newest) = (Vec::new(), 0);
        for slot in done.iter_mut().filter(|d| d.is_none()) {
            let admission = written.next().expect("one admission per op");
            let op = sent.next().expect("one op per admission");
            let accepted = rooms.accept(key, admission, at)?.accepted;
            if accepted.written {
                touched.extend(meldframe_batch::touches(&op.command));
                newest = accepted.seq;
                let stored = StoredOp {
                    seq: accepted.seq,
                    actor: who.actor.clone(),
                    key: op.key,
                    command: op.command,
                };
                // A gap cannot happen (one writer, one numbering); if it did, the hub resyncs its streams
                // itself, so the write is never failed here.
                let _ = live.publish(key, accepted.seq, stored);
            }
            *slot = Some(accepted);
        }
        live.touch(key, &who.actor, touched, newest, at);
        Ok(done.into_iter().flatten().collect())
    }
}

/// `Some` when `op`'s key is already in the room's log for `actor` with the same command.
fn duplicate(room: &Room<AnyStore>, actor: &str, op: &Op) -> Option<Accepted> {
    let store = room.store();
    let seq = store.find(room.id(), actor, &op.key).ok()??;
    let first = store.ops_after(room.id(), seq - 1, 1).ok()?.pop()?;
    let same =
        serde_json::to_value(&first.command).ok()? == serde_json::to_value(&op.command).ok()?;
    (first.seq == seq && same).then_some(Accepted {
        seq,
        written: false,
    })
}
