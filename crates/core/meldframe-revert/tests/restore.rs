//! Restore: the commands that bring a file back to an earlier state, and the rest of the checkpoint rules.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_history::FileState;
use meldframe_protocol::{Command, Node, NodePatch};
use meldframe_revert::{LoggedOp, RunDetector, restore_commands, state_at};

fn add(id: &str, kind: &str, parent: Option<&str>) -> Command {
    let node: Node =
        serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent}))
            .unwrap();
    Command::AddNode { node, index: None }
}

fn upd(id: &str, f: impl FnOnce(&mut NodePatch)) -> Command {
    let mut patch = NodePatch::default();
    f(&mut patch);
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

fn state(cmds: &[Command]) -> FileState {
    let mut s = FileState::new();
    for c in cmds {
        s.apply(c).unwrap();
    }
    s
}

/// The tree in a canonical form: list order carries no meaning, and `pos` counts as the rank among the
/// siblings (a position another sibling holds is resolved by the apply rule, so the string can differ).
fn canon(nodes: &[Node]) -> String {
    let mut v = nodes.to_vec();
    v.sort_by(|a, b| (&a.parent_id, &a.pos, &a.id).cmp(&(&b.parent_id, &b.pos, &b.id)));
    let mut rank: std::collections::HashMap<Option<String>, usize> = Default::default();
    for n in &mut v {
        let r = rank.entry(n.parent_id.clone()).or_default();
        n.pos = r.to_string();
        *r += 1;
    }
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

/// Apply `cmds` to `current` and check the result is `target`.
fn restores(target: &[Node], current: &[Node]) {
    let cmds = restore_commands(target, current).unwrap();
    let mut s = FileState::from_nodes_unchecked(current.to_vec());
    for c in &cmds {
        s.replay_pending(c).expect("the list applies whole");
    }
    assert_eq!(canon(s.nodes()), canon(target));
}

fn base() -> Vec<Command> {
    vec![
        add("root", "frame", None),
        add("p", "frame", Some("root")),
        add("q", "frame", Some("root")),
        add("a", "rect", Some("p")),
        add("b", "rect", Some("p")),
    ]
}

#[test]
fn an_unchanged_file_needs_no_command() {
    let t = state(&base());
    assert!(restore_commands(t.nodes(), t.nodes()).unwrap().is_empty());
}

#[test]
fn changed_fields_go_back_including_a_field_that_was_unset() {
    let t = state(&base());
    let mut s = FileState::from_nodes_unchecked(t.nodes().to_vec());
    s.apply(&upd("a", |p| {
        p.x = Some(40.0);
        p.fill = Some("#ff0000".into());
        p.pad_t = Some(Some(3.0));
    }))
    .unwrap();
    restores(t.nodes(), s.nodes());
}

#[test]
fn added_nodes_are_removed_and_removed_nodes_come_back_with_their_subtree() {
    let t = state(&base());
    let mut s = FileState::from_nodes_unchecked(t.nodes().to_vec());
    s.apply(&add("extra", "rect", Some("q"))).unwrap();
    s.apply(&Command::RemoveNode { id: "p".into() }).unwrap();
    restores(t.nodes(), s.nodes());
}

#[test]
fn moved_and_reordered_nodes_go_back() {
    let t = state(&base());
    let mut s = FileState::from_nodes_unchecked(t.nodes().to_vec());
    s.apply(&Command::Reparent {
        id: "a".into(),
        parent_id: Some("q".into()),
        pos: None,
    })
    .unwrap();
    s.apply(&Command::Reorder {
        id: "b".into(),
        before: None,
        after: None,
        pos: None,
    })
    .unwrap();
    restores(t.nodes(), s.nodes());
}

#[test]
fn a_node_that_sits_under_a_removed_parent_is_moved_out_first() {
    // The target has `a` under `q` and no `p`; the current file has `a` under `p`.
    let t = state(&[
        add("root", "frame", None),
        add("q", "frame", Some("root")),
        add("a", "rect", Some("q")),
    ]);
    let s = state(&[
        add("root", "frame", None),
        add("q", "frame", Some("root")),
        add("p", "frame", Some("root")),
        add("a", "rect", Some("p")),
    ]);
    restores(t.nodes(), s.nodes());
}

#[test]
fn a_node_whose_kind_changed_under_the_same_id_is_replaced() {
    let t = state(&[add("root", "frame", None), add("a", "rect", Some("root"))]);
    let s = state(&[add("root", "frame", None), add("a", "text", Some("root"))]);
    restores(t.nodes(), s.nodes());
}

#[test]
fn state_at_replays_the_log_up_to_a_number() {
    let start = state(&base());
    let ops = [
        upd("a", |p| p.x = Some(1.0)),
        upd("a", |p| p.x = Some(2.0)),
        upd("a", |p| p.x = Some(3.0)),
    ];
    let log: Vec<LoggedOp> = ops
        .iter()
        .enumerate()
        .map(|(i, c)| LoggedOp {
            seq: 10 + i as u64,
            actor: "agent:x".into(),
            key: format!("k:{i}"),
            command: c.clone(),
        })
        .collect();
    let at = state_at(start.nodes(), &log, 11).unwrap();
    assert_eq!(at.iter().find(|n| n.id == "a").unwrap().x, 2.0);
    let none = state_at(start.nodes(), &log, 9).unwrap();
    assert_eq!(canon(&none), canon(start.nodes()));
}

#[test]
fn a_run_starts_when_an_actor_is_new_or_was_quiet_for_the_gap() {
    let mut d = RunDetector::new(1_000);
    assert!(d.observe("agent:x", 0), "the first op of an actor");
    assert!(!d.observe("agent:x", 500));
    assert!(
        !d.observe("agent:x", 1_400),
        "the gap counts from the last op"
    );
    assert!(d.observe("agent:x", 2_500), "quiet for longer than the gap");
    assert!(d.observe("agent:y", 2_500), "another actor has its own run");
}

/// A small deterministic generator, so a failing seed can be replayed.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

#[test]
fn restore_after_random_edits_brings_back_the_target() {
    let ids = ["root", "p", "q", "a", "b", "c", "d", "e"];
    for seed in 0..300u64 {
        let mut rng = Rng(seed);
        let target = state(&base());
        let mut s = FileState::from_nodes_unchecked(target.nodes().to_vec());
        for step in 0..12 {
            let pick = |r: &mut Rng| ids[r.below(ids.len())];
            let cmd = match rng.below(5) {
                0 => add(&format!("n{step}"), "rect", Some(pick(&mut rng))),
                1 => upd(pick(&mut rng), |p| p.x = Some(step as f64)),
                2 => Command::RemoveNode {
                    id: pick(&mut rng).into(),
                },
                3 => Command::Reparent {
                    id: pick(&mut rng).into(),
                    parent_id: Some(pick(&mut rng).into()),
                    pos: None,
                },
                _ => Command::Reorder {
                    id: pick(&mut rng).into(),
                    before: None,
                    after: None,
                    pos: None,
                },
            };
            let _ = s.replay_pending(&cmd);
        }
        let cmds = restore_commands(target.nodes(), s.nodes())
            .unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        let mut r = FileState::from_nodes_unchecked(s.nodes().to_vec());
        for c in &cmds {
            r.replay_pending(c)
                .unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        }
        assert_eq!(canon(r.nodes()), canon(target.nodes()), "seed {seed}");
    }
}
