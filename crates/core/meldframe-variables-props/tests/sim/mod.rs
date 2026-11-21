//! Replicas and a room, joined by queues. What the room accepts or refuses reaches each replica only when the
//! run delivers it, so several actors edit against one state and the order they meet it in is random. Every seed
//! is one run; a failure names the seed, the last steps and the line that replays it.

pub mod random;

use meldframe_check::Limits;
use meldframe_host::{Gate, MemoryStore, Op, Principal, Refusal, Room, RoomError};
use meldframe_protocol::{Command, Variables};
use meldframe_sync::{EventOutcome, Replica};
use meldframe_variables::check_variables;
use random::{Ids, Rng, contend, rand_command, rand_rename, seed_commands};
use std::collections::{HashMap, VecDeque};

#[allow(clippy::large_enum_variant)]
enum Msg {
    Event(u64, String, String, Command),
    Reject(String),
}

/// What a run did, to show that the checks were not idle.
#[derive(Default, Debug)]
pub struct Stats {
    pub seeds: u64,
    /// Renames the room accepted, and the ones it or a replica refused for a name that was taken.
    pub renames_accepted: u64,
    pub duplicate_name_refusals: u64,
    pub rename_refusals: u64,
    /// Local commands a replica refused, and ops the room refused.
    pub local_refusals: u64,
    pub room_refusals: u64,
    pub undos: u64,
    pub redos: u64,
    /// Renames an undo or a redo sent, each checked not to overwrite a name another actor wrote.
    pub undo_renames: u64,
    /// Ops a replica dropped from its pending queue when another actor's op came in.
    pub dropped: u64,
}

fn is_rename(c: &Command) -> bool {
    matches!(
        c,
        Command::RenameVariable { .. }
            | Command::RenameCollection { .. }
            | Command::RenameMode { .. }
    )
}

fn short(c: &Command) -> String {
    serde_json::to_string(c)
        .unwrap()
        .chars()
        .take(170)
        .collect()
}

/// What a command writes the name of, as keys: `v:id`, `c:id`, `m:collection:mode`. A name is written by a
/// rename and by the command that adds the entity.
fn name_keys(c: &Command) -> Vec<String> {
    match c {
        Command::RenameVariable { id, .. } | Command::RemoveVariable { id } => {
            vec![format!("v:{id}")]
        }
        Command::AddVariable { variable } => vec![format!("v:{}", variable.id)],
        Command::RenameCollection { id, .. } => vec![format!("c:{id}")],
        Command::AddCollection { collection } => {
            let mut k = vec![format!("c:{}", collection.id)];
            k.extend(
                collection
                    .modes
                    .iter()
                    .map(|m| format!("m:{}:{}", collection.id, m.id)),
            );
            k
        }
        Command::RenameMode {
            collection, mode, ..
        } => vec![format!("m:{collection}:{mode}")],
        Command::AddMode {
            collection, mode, ..
        } => vec![format!("m:{collection}:{}", mode.id)],
        _ => vec![],
    }
}

/// The names a command renames (the part of `name_keys` that is a rename).
fn rename_keys(c: &Command) -> Vec<String> {
    if is_rename(c) { name_keys(c) } else { vec![] }
}

/// A replica's state that a refused command must leave as it was.
type Mark = (Variables, usize, usize, usize);

pub struct Sim {
    seed: u64,
    room: Room<MemoryStore>,
    gate: Gate,
    reps: Vec<Replica>,
    inbox: Vec<VecDeque<Msg>>,
    /// For each replica, who wrote the name of each entity last, among the events it has taken in.
    writers: Vec<HashMap<String, String>>,
    ids: Ids,
    trace: Vec<String>,
    limits: Limits,
    pub stats: Stats,
}

impl Sim {
    /// `n` replicas and the room, after a seed actor made two collections and four variables and every
    /// replica got them: the histories start empty.
    pub fn new(seed: u64, n: usize) -> Sim {
        let mut s = Sim {
            seed,
            room: Room::open("ws_1", "file_1", MemoryStore::new()).unwrap(),
            gate: Gate::new(),
            reps: (0..n)
                .map(|i| Replica::new(&format!("user:u{i}"), &format!("u{i}"), vec![], 0).unwrap())
                .collect(),
            inbox: (0..n).map(|_| VecDeque::new()).collect(),
            writers: (0..n).map(|_| HashMap::new()).collect(),
            ids: Ids::default(),
            trace: Vec::new(),
            limits: Limits::default(),
            stats: Stats::default(),
        };
        for (i, c) in seed_commands().iter().enumerate() {
            if let Err(r) = s.submit(None, "user:seed", &format!("s{i}"), c, &[]) {
                s.fail(&format!(
                    "the seed command {} was refused: {}",
                    short(c),
                    r.code()
                ));
            }
        }
        for i in 0..n {
            s.deliver(i, usize::MAX);
        }
        s
    }

    fn fail(&self, msg: &str) -> ! {
        let from = self.trace.len().saturating_sub(
            std::env::var("MELDFRAME_TRACE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        );
        panic!(
            "seed {seed}: {msg}\nreplay: MELDFRAME_SEED={seed} cargo test -p meldframe-variables-props\nlast steps:\n{}",
            self.trace[from..].join("\n"),
            seed = self.seed,
        )
    }

    pub fn vars(&self, i: usize) -> &Variables {
        self.reps[i].vars()
    }

    /// No undo or redo overwrites a name another actor wrote after the author's step and the author has seen: a
    /// rename that an undo or a redo of replica `i` queued names an entity whose name the replica itself wrote last.
    /// `pending` is what the replica had queued before the step; those ops sit on top of the confirmed events.
    fn check_names(&mut self, i: usize, pending: &[meldframe_sync::Outgoing]) {
        let me = self.reps[i].actor().to_string();
        let now = self.reps[i].unconfirmed();
        for o in &now[pending.len()..] {
            for k in rename_keys(&o.command) {
                self.stats.undo_renames += 1;
                let mine = pending.iter().any(|p| name_keys(&p.command).contains(&k));
                let last = self.writers[i].get(&k);
                if !mine && last != Some(&me) {
                    self.fail(&format!(
                        "replica {i}: {} {} overwrites a name that {} wrote and the replica had seen",
                        format!("{:?}", o.kind).to_lowercase(),
                        short(&o.command),
                        last.map_or("nobody", String::as_str),
                    ));
                }
            }
        }
    }

    pub fn undo_depth(&self, i: usize) -> usize {
        self.reps[i].undo_depth()
    }

    /// Replica `i`'s variables with the collections and the variables in id order: an undo that adds a
    /// variable or a collection again puts it last, as `addVariable` does, so the list order is not compared.
    pub fn canon(&self, i: usize) -> Variables {
        let mut v = self.vars(i).clone();
        v.collections.sort_by(|a, b| a.id.cmp(&b.id));
        v.variables.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Fail unless replica `i` holds `want` (from `canon`), naming what differs.
    pub fn expect_vars(&self, i: usize, want: &Variables, what: &str) {
        let got = self.canon(i);
        if got == *want {
            return;
        }
        let mut diff = Vec::new();
        for (a, b) in want.collections.iter().zip(&got.collections) {
            if a != b {
                diff.push(format!(
                    "collection want {} got {}",
                    serde_json::to_string(a).unwrap(),
                    serde_json::to_string(b).unwrap()
                ));
            }
        }
        for (a, b) in want.variables.iter().zip(&got.variables) {
            if a != b {
                diff.push(format!(
                    "variable want {} got {}",
                    serde_json::to_string(a).unwrap(),
                    serde_json::to_string(b).unwrap()
                ));
            }
        }
        if want.collections.len() != got.collections.len()
            || want.variables.len() != got.variables.len()
        {
            diff.push("another number of collections or variables".to_string());
        }
        self.fail(&format!(
            "replica {i}: {what} did not give the variables back as they were:\n{}",
            diff.join("\n")
        ));
    }

    fn mark(&self, i: usize) -> Mark {
        let r = &self.reps[i];
        (
            r.vars().clone(),
            r.pending_len(),
            r.undo_depth(),
            r.redo_depth(),
        )
    }

    /// A command the replica refused changed nothing: not the variables, the queue or the history.
    fn refused_locally(&mut self, i: usize, before: &Mark, what: &str, code: &str) {
        self.stats.local_refusals += 1;
        if self.mark(i) != *before {
            self.fail(&format!(
                "replica {i}: a refused {what} changed the replica ({code})"
            ));
        }
    }

    /// Put one op through the gate and the room. An accepted op goes to every inbox; a refusal goes to the
    /// inbox of `who`, the replica that sent it, and the room is as it was.
    fn submit(
        &mut self,
        who: Option<usize>,
        actor: &str,
        key: &str,
        c: &Command,
        deps: &[String],
    ) -> Result<u64, Refusal> {
        let op = Op::new(key, c.clone()).with_deps(deps.to_vec());
        let admission = self
            .gate
            .submit(&Principal::new("ws_1", actor), vec![op])
            .unwrap()
            .admitted
            .remove(0);
        let before = (self.room.vars().clone(), self.room.seq());
        match self.room.accept(admission) {
            Ok(a) => {
                self.stats.renames_accepted += is_rename(c) as u64;
                for q in &mut self.inbox {
                    q.push_back(Msg::Event(a.seq, actor.into(), key.into(), c.clone()));
                }
                Ok(a.seq)
            }
            Err(RoomError::Refused(r)) => {
                self.stats.room_refusals += 1;
                if (self.room.vars().clone(), self.room.seq()) != before {
                    self.fail(&format!(
                        "the room changed on a refused op {} ({})",
                        short(c),
                        r.code()
                    ));
                }
                if is_rename(c) {
                    self.stats.rename_refusals += 1;
                    self.stats.duplicate_name_refusals += (r.code() == "E_DUPLICATE_NAME") as u64;
                }
                if let Some(w) = who {
                    self.inbox[w].push_back(Msg::Reject(key.into()));
                }
                Err(r)
            }
            Err(e) => self.fail(&format!("the room failed on {}: {e}", short(c))),
        }
    }

    /// Replica `i` makes a random edit of its own. Whether the replica took it (a refused edit changed nothing).
    pub fn edit(&mut self, i: usize, r: &mut Rng, renames: bool) -> bool {
        let c = rand_command(self.reps[i].vars(), &mut self.ids, r, renames);
        self.edit_with(i, &c)
    }

    /// A race for a name: replica `i` renames something and sends it, replica `j` takes it in and renames it again
    /// (or removes it) and sends that, `i` takes it in and undoes its rename; now and then another contender and a
    /// redo follow. An undo and a redo must not write the name back over what `j` wrote.
    pub fn race(&mut self, i: usize, j: usize, r: &mut Rng) {
        let Some(c) = rand_rename(self.reps[i].vars(), r) else {
            return;
        };
        if !self.edit_with(i, &c) {
            return;
        }
        self.send(i);
        self.deliver(j, usize::MAX);
        let again = contend(&c, r);
        self.edit_with(j, &again);
        self.send(j);
        self.deliver(i, usize::MAX);
        self.undo(i);
        if r.below(2) == 0 {
            self.deliver(j, usize::MAX);
            let again = contend(&c, r);
            self.edit_with(j, &again);
            self.send(j);
            self.deliver(i, usize::MAX);
            self.redo(i);
        }
    }

    pub fn edit_with(&mut self, i: usize, c: &Command) -> bool {
        let c = c.clone();
        let before = self.mark(i);
        match self.reps[i].apply(&c) {
            Ok(()) => {
                self.trace.push(format!("r{i} edit {}", short(&c)));
                true
            }
            Err(e) => {
                self.trace
                    .push(format!("r{i} edit {} refused {}", short(&c), e.code()));
                if is_rename(&c) {
                    self.stats.rename_refusals += 1;
                    self.stats.duplicate_name_refusals += (e.code() == "E_DUPLICATE_NAME") as u64;
                }
                self.refused_locally(i, &before, "edit", e.code());
                false
            }
        }
    }

    /// Whether there was a step to undo.
    pub fn undo(&mut self, i: usize) -> bool {
        let before = self.mark(i);
        let pending = self.reps[i].unconfirmed();
        let res = self.reps[i].undo();
        match res {
            Ok(done) => {
                self.trace.push(format!("r{i} undo {done}"));
                self.check_names(i, &pending);
                self.stats.undos += done as u64;
                done
            }
            Err(e) => {
                self.trace.push(format!("r{i} undo refused {}", e.code()));
                self.refused_locally(i, &before, "undo", e.code());
                false
            }
        }
    }

    /// Whether there was a step to redo.
    pub fn redo(&mut self, i: usize) -> bool {
        let before = self.mark(i);
        let pending = self.reps[i].unconfirmed();
        let res = self.reps[i].redo();
        match res {
            Ok(done) => {
                self.trace.push(format!("r{i} redo {done}"));
                self.check_names(i, &pending);
                self.stats.redos += done as u64;
                done
            }
            Err(e) => {
                self.trace.push(format!("r{i} redo refused {}", e.code()));
                self.refused_locally(i, &before, "redo", e.code());
                false
            }
        }
    }

    /// Send the outbox of replica `i` to the room, in order.
    pub fn send(&mut self, i: usize) {
        let actor = self.reps[i].actor().to_string();
        for o in self.reps[i].take_outbox() {
            let res = self.submit(Some(i), &actor, &o.key, &o.command, &o.deps);
            let how = match res {
                Ok(seq) => format!("seq {seq}"),
                Err(r) => format!("refused {}", r.code()),
            };
            self.trace
                .push(format!("r{i} send {} {how}", short(&o.command)));
        }
    }

    /// Deliver at most `max` of the messages replica `i` has waiting, in order.
    pub fn deliver(&mut self, i: usize, max: usize) {
        for _ in 0..max {
            let Some(m) = self.inbox[i].pop_front() else {
                return;
            };
            match m {
                Msg::Event(seq, actor, key, c) => {
                    match self.reps[i].on_event_keyed(seq, &actor, Some(&key), &c) {
                        Ok(EventOutcome::Rebased { dropped }) => {
                            self.stats.dropped += dropped as u64
                        }
                        Ok(_) => {}
                        Err(e) => {
                            self.fail(&format!("replica {i} could not follow event {seq}: {e:?}"))
                        }
                    }
                    for k in name_keys(&c) {
                        self.writers[i].insert(k, actor.clone());
                    }
                    self.trace
                        .push(format!("r{i} event {seq} {actor} {}", short(&c)));
                }
                Msg::Reject(key) => {
                    self.reps[i].on_reject(&key);
                    self.trace.push(format!("r{i} reject {key}"));
                }
            }
        }
    }

    /// The invariants, after every step: each replica is consistent and holds valid variables, the room
    /// holds valid variables, and a replica that has everything the room did and nothing pending equals it.
    pub fn check(&self) {
        let limits = self.limits.var_limits();
        if let Err((id, e)) = check_variables(self.room.vars(), &limits) {
            self.fail(&format!("the room holds invalid variables: {id}: {e}"));
        }
        for (i, r) in self.reps.iter().enumerate() {
            if let Err(e) = r.check_consistency() {
                self.fail(&format!("replica {i}: {e}"));
            }
            if let Err((id, e)) = check_variables(r.vars(), &limits) {
                self.fail(&format!("replica {i} holds invalid variables: {id}: {e}"));
            }
            if r.pending_len() == 0 && self.inbox[i].is_empty() && r.vars() != self.room.vars() {
                self.fail(&format!("replica {i} is quiet and differs from the room"));
            }
        }
    }

    /// Send and deliver everything until nothing is left, then every replica equals the room.
    pub fn settle(&mut self) {
        for _ in 0..1000 {
            let n = self.reps.len();
            for i in 0..n {
                self.send(i);
            }
            for i in 0..n {
                self.deliver(i, usize::MAX);
            }
            if self.inbox.iter().all(VecDeque::is_empty)
                && self.reps.iter().all(|r| r.pending_len() == 0)
            {
                self.check();
                for (i, r) in self.reps.iter().enumerate() {
                    if r.vars() != self.room.vars() {
                        self.fail(&format!("replica {i} differs from the room at the end"));
                    }
                }
                return;
            }
        }
        self.fail("the run did not settle");
    }
}
