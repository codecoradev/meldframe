//! A restore must not ask for more approval than it has to (ADR-0010, ADR-0016): the gate tiers a removal by
//! the tree as it is when the op arrives, so a restore removes leaves first. Only a frame is ever a Delete.

use meldframe_history::FileState;
use meldframe_host::Op;
use meldframe_module::RiskTier;
use meldframe_protocol::{Command, Node};
use meldframe_revert::restore_commands;
use meldframe_rooms::TreeFacts;

fn add(id: &str, kind: &str, parent: Option<&str>) -> Command {
    let node: Node =
        serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent}))
            .unwrap();
    Command::AddNode { node, index: None }
}

fn state(cmds: &[Command]) -> FileState {
    let mut s = FileState::new();
    cmds.iter().for_each(|c| s.apply(c).unwrap());
    s
}

#[test]
fn restoring_to_before_an_agent_added_a_frame_with_content_removes_leaves_first() {
    let before = [
        add("root", "frame", None),
        add("keep", "rect", Some("root")),
    ];
    let agent = [
        add("hero", "frame", Some("root")),
        add("title", "text", Some("hero")),
        add("card", "frame", Some("hero")),
        add("pic", "rect", Some("card")),
    ];
    let target = state(&before);
    let current = state(&[&before[..], &agent[..]].concat());
    let cmds = restore_commands(target.nodes(), current.nodes()).unwrap();

    let mut fs = FileState::from_nodes_unchecked(current.nodes().to_vec());
    let mut removed = 0;
    for (i, c) in cmds.iter().enumerate() {
        if let Command::RemoveNode { id } = c {
            removed += 1;
            let has_children = fs
                .nodes()
                .iter()
                .any(|n| n.parent_id.as_deref() == Some(id));
            assert!(
                !has_children,
                "command {i}: {id} is removed with its content"
            );
            let tier = Op::new("k", c.clone()).tier_with(&TreeFacts::new(fs.nodes()));
            let is_frame = fs.node(id).unwrap().kind == meldframe_protocol::NodeKind::Frame;
            assert_eq!(
                tier == RiskTier::Delete,
                is_frame,
                "only a frame is a Delete: {id}"
            );
        }
        fs.replay_pending(c).unwrap();
    }
    assert_eq!(removed, 4);
    assert_eq!(fs.nodes().len(), target.nodes().len());
}
