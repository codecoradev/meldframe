//! E5.2 and E5.3 with a server that refuses a frame removal (the Delete tier): the frame stays, empty, and the
//! person is told which removal was refused.

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, Event, Frame};
use meldframe_protocol::Command;

fn add(id: &str, kind: &str, parent: &str) -> Command {
    Command::AddNode {
        node: node(id, kind, Some(parent)),
        index: None,
    }
}

#[test]
fn a_restore_across_an_added_frame_leaves_it_empty_and_names_the_refused_removal() {
    let mut l = Loopback::live();
    l.send(Action::CreateCheckpoint {
        label: "before".into(),
    });
    let id = l.app.state().checkpoints.last().unwrap().id.clone();
    l.remote("agent:x", "b1:0", 0, add("hero", "frame", "root"));
    l.remote("agent:x", "b1:1", 0, add("pic", "rect", "hero"));
    let effects = l.app.dispatch(Action::RestoreCheckpoint { id });
    let ops: Vec<_> = effects
        .iter()
        .flat_map(|e| match e {
            meldframe_app::Effect::SendOps { ops } => ops.clone(),
            _ => vec![],
        })
        .collect();
    assert_eq!(ops.len(), 2, "remove pic, then remove hero");
    assert!(matches!(&ops[1].command, Command::RemoveNode { id } if id == "hero"));
    // The server confirms the leaf and refuses the frame, as the Delete tier does.
    let seq = l.seq + 1;
    l.app.dispatch(Action::Frame {
        now_ms: 0,
        frame: Frame::Event {
            seq,
            actor: "user:me".into(),
            key: Some(ops[0].key.clone()),
            command: ops[0].command.clone(),
        },
    });
    l.app.dispatch(Action::Frame {
        now_ms: 0,
        frame: Frame::Rejected {
            key: ops[1].key.clone(),
            code: "approval_required".into(),
            retryable: false,
        },
    });
    l.events.extend(l.app.take_events());
    let nodes = l.app.nodes();
    assert!(nodes.iter().any(|n| n.id == "hero"), "the frame stays");
    assert!(!nodes.iter().any(|n| n.id == "pic"), "its content is gone");
    assert!(l.app.state().pending.is_empty());
    assert!(
        l.events
            .iter()
            .any(|e| matches!(e, Event::Restored { requested: 2, .. }))
    );
    let rejected: Vec<_> = l
        .events
        .iter()
        .filter_map(|e| match e {
            Event::OpRejected { node, code, .. } => Some((node.clone(), code.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        rejected,
        vec![(Some("hero".to_string()), "approval_required".to_string())],
        "the person is told which removal was refused"
    );
}
