//! Random variable commands, made from what a replica sees. The names come from small alphabets so that
//! two variables of one collection often want the same name, and now and then a name is invalid: the
//! refusals are part of the run.

use meldframe_protocol::{Command, Variables};
use serde_json::{Value, json};

/// Small deterministic generator, so a failing seed can be replayed.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        &s[self.below(s.len())]
    }
}

const VAR_NAMES: [&str; 6] = ["a", "b", "c", "a/b", "a/c", "b/a"];
const COLLECTION_NAMES: [&str; 4] = ["a", "b", "a/b", "c"];
const MODE_NAMES: [&str; 4] = ["x", "y", "z", "light"];
/// Refused by every name check (`addVariable` and the renames alike).
const BAD_NAMES: [&str; 4] = ["Bad Name", "a//b", "-a", "a/"];

fn name(r: &mut Rng, pool: &[&str]) -> String {
    match r.below(12) {
        0 => r.pick(&BAD_NAMES).to_string(),
        _ => r.pick(pool).to_string(),
    }
}

#[derive(Clone, Copy)]
enum Kind {
    RenameVariable,
    RenameCollection,
    RenameMode,
    AddVariable,
    RemoveVariable,
    AddMode,
    RemoveMode,
    SetValue,
    AddCollection,
    RemoveCollection,
}
use Kind::*;

/// The mix of an even seed: every command, renames a third of it.
const MIXED: &[Kind] = &[
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameCollection,
    RenameCollection,
    RenameMode,
    RenameMode,
    RenameMode,
    AddVariable,
    AddVariable,
    RemoveVariable,
    AddMode,
    RemoveMode,
    SetValue,
    SetValue,
    SetValue,
    AddCollection,
    RemoveCollection,
];

/// The mix of an odd seed: mostly renames of variables, so the names of one collection are fought over.
const RENAMES: &[Kind] = &[
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameVariable,
    RenameCollection,
    RenameMode,
    AddVariable,
    RemoveVariable,
    RemoveVariable,
    SetValue,
];

/// Fresh ids for what the run adds: `v1`, `c2`, `m3`; the counter is shared so an id is never made twice.
#[derive(Default)]
pub struct Ids(u32);

impl Ids {
    fn next(&mut self, prefix: &str) -> String {
        self.0 += 1;
        format!("{prefix}{}", self.0)
    }
}

fn cmd(v: Value) -> Command {
    serde_json::from_value(v).expect("a generated command parses")
}

/// An id of what exists, and once in fifteen one that does not.
fn existing(r: &mut Rng, ids: &[&str]) -> String {
    if ids.is_empty() || r.below(15) == 0 {
        return "ghost".to_string();
    }
    r.pick(ids).to_string()
}

/// A colour, or now and then an alias to a variable that is there (which can make a cycle: refused).
fn value(r: &mut Rng, vars: &Variables) -> Value {
    if r.below(6) == 0 && !vars.variables.is_empty() {
        return json!({"var": r.pick(&vars.variables).id});
    }
    json!(format!("#00000{}", r.below(10)))
}

fn mode_ids(vars: &Variables, collection: &str) -> Vec<String> {
    vars.collection(collection)
        .map(|c| c.modes.iter().map(|m| m.id.clone()).collect())
        .unwrap_or_default()
}

/// The collections, with two variables in the first, for the start of every run.
pub fn seed_commands() -> Vec<Command> {
    let mut out = vec![
        cmd(
            json!({"type": "addCollection", "collection": {"id": "c0", "name": "a",
            "modes": [{"id": "m0", "name": "x"}, {"id": "m1", "name": "y"}]}}),
        ),
        cmd(
            json!({"type": "addCollection", "collection": {"id": "c1", "name": "b",
            "modes": [{"id": "m2", "name": "x"}]}}),
        ),
    ];
    for (id, col, name, modes) in [
        ("v0", "c0", "a", &["m0", "m1"][..]),
        ("v1", "c0", "b", &["m0", "m1"][..]),
        ("v2", "c0", "c", &["m0", "m1"][..]),
        ("v3", "c1", "a", &["m2"][..]),
    ] {
        let values: serde_json::Map<String, Value> = modes
            .iter()
            .map(|m| (m.to_string(), json!("#000000")))
            .collect();
        out.push(cmd(json!({"type": "addVariable", "variable": {"id": id,
            "collection": col, "name": name, "type": "color", "values": values}})));
    }
    out
}

/// One random command as the replica that holds `vars` would send it.
pub fn rand_command(vars: &Variables, ids: &mut Ids, r: &mut Rng, renames: bool) -> Command {
    let kind = *r.pick(if renames { RENAMES } else { MIXED });
    let vs: Vec<&str> = vars.variables.iter().map(|v| v.id.as_str()).collect();
    let cs: Vec<&str> = vars.collections.iter().map(|c| c.id.as_str()).collect();
    let col = existing(r, &cs);
    let modes = mode_ids(vars, &col);
    let mode_refs: Vec<&str> = modes.iter().map(String::as_str).collect();
    match kind {
        RenameVariable => cmd(
            json!({"type": "renameVariable", "id": existing(r, &vs), "name": name(r, &VAR_NAMES)}),
        ),
        RenameCollection => {
            cmd(json!({"type": "renameCollection", "id": col, "name": name(r, &COLLECTION_NAMES)}))
        }
        RenameMode => cmd(json!({"type": "renameMode", "collection": col,
            "mode": existing(r, &mode_refs), "name": r.pick(&MODE_NAMES)})),
        AddVariable => {
            let values: serde_json::Map<String, Value> =
                modes.iter().map(|m| (m.clone(), value(r, vars))).collect();
            cmd(
                json!({"type": "addVariable", "variable": {"id": ids.next("v"), "collection": col,
                "name": name(r, &VAR_NAMES), "type": "color", "values": values}}),
            )
        }
        RemoveVariable => cmd(json!({"type": "removeVariable", "id": existing(r, &vs)})),
        AddMode => {
            let index = [None, Some(1), Some(r.below(modes.len() + 2))][r.below(3)];
            let copy_from = (r.below(2) == 0).then(|| existing(r, &mode_refs));
            cmd(json!({"type": "addMode", "collection": col,
                "mode": {"id": ids.next("m"), "name": r.pick(&MODE_NAMES)},
                "index": index, "copyFrom": copy_from}))
        }
        RemoveMode => cmd(json!({"type": "removeMode", "collection": col,
            "mode": existing(r, &mode_refs)})),
        SetValue => {
            let id = existing(r, &vs);
            let own = vars
                .variable(&id)
                .map(|v| mode_ids(vars, &v.collection))
                .unwrap_or_default();
            let own: Vec<&str> = own.iter().map(String::as_str).collect();
            cmd(json!({"type": "setVariableValue", "id": id,
                "mode": existing(r, &own), "value": value(r, vars)}))
        }
        AddCollection => {
            let modes: Vec<Value> = (0..1 + r.below(2))
                .map(|_| json!({"id": ids.next("m"), "name": r.pick(&MODE_NAMES)}))
                .collect();
            cmd(
                json!({"type": "addCollection", "collection": {"id": ids.next("c"),
                "name": name(r, &COLLECTION_NAMES), "modes": modes}}),
            )
        }
        RemoveCollection => cmd(json!({"type": "removeCollection", "id": col})),
    }
}

/// A rename of a variable, a collection or a mode that exists, to a name that is valid.
pub fn rand_rename(vars: &Variables, r: &mut Rng) -> Option<Command> {
    let id = |r: &mut Rng, ids: Vec<String>| (!ids.is_empty()).then(|| r.pick(&ids).clone());
    match r.below(3) {
        0 => {
            let v = id(r, vars.variables.iter().map(|v| v.id.clone()).collect())?;
            Some(cmd(
                json!({"type": "renameVariable", "id": v, "name": r.pick(&VAR_NAMES)}),
            ))
        }
        1 => {
            let c = id(r, vars.collections.iter().map(|c| c.id.clone()).collect())?;
            Some(cmd(
                json!({"type": "renameCollection", "id": c, "name": r.pick(&COLLECTION_NAMES)}),
            ))
        }
        _ => {
            let c = vars.collections.get(r.below(vars.collections.len()))?;
            let m = r.pick(&c.modes);
            Some(cmd(
                json!({"type": "renameMode", "collection": c.id, "mode": m.id,
                "name": r.pick(&MODE_NAMES)}),
            ))
        }
    }
}

/// What a second actor does to the entity that `c` renamed: renames it again, or (a variable) removes it.
pub fn contend(c: &Command, r: &mut Rng) -> Command {
    match c {
        Command::RenameVariable { id, .. } if r.below(5) == 0 => {
            cmd(json!({"type": "removeVariable", "id": id}))
        }
        Command::RenameVariable { id, .. } => {
            cmd(json!({"type": "renameVariable", "id": id, "name": r.pick(&VAR_NAMES)}))
        }
        Command::RenameCollection { id, .. } => {
            cmd(json!({"type": "renameCollection", "id": id, "name": r.pick(&COLLECTION_NAMES)}))
        }
        Command::RenameMode {
            collection, mode, ..
        } => cmd(json!({"type": "renameMode",
            "collection": collection, "mode": mode, "name": r.pick(&MODE_NAMES)})),
        _ => unreachable!("contend takes a rename"),
    }
}
