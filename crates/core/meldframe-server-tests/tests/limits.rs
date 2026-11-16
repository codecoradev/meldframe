//! The server bounds what one workspace can hold open, validates workspace and file names in one place, and
//! refuses calls that cannot succeed before it opens a room for them.

use meldframe_host::{Op, Principal, Refusal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_rooms::RegistryError;
use meldframe_server::{MAX_BATCH_OPS, Server, SubmitError};

struct Tools;
impl Module for Tools {
    fn name(&self) -> &'static str {
        "tools"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
        r.agent_tool("peek", "Look at a node.", RiskTier::Read);
    }
}

fn who(ws: &str) -> Principal {
    Principal::new(ws, "agent:test")
}

fn add(n: usize) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": format!("n{n}"), "type": "frame"}))
        .unwrap();
    Op::new(format!("k{n}"), Command::AddNode { node, index: None })
}

fn server(per_workspace: usize) -> Server {
    Server::assemble_with(vec![Box::new(Tools)]).with_workspace_rooms(per_workspace)
}

fn paint(s: &mut Server, ws: &str, file: &str) -> Result<(), SubmitError> {
    s.submit_tool("paint", file, &who(ws), vec![add(1)])
        .map(|_| ())
}

#[test]
fn one_workspace_cannot_fill_the_registry_for_the_others() {
    let mut s = server(2);
    paint(&mut s, "ws_1", "a").unwrap();
    paint(&mut s, "ws_1", "b").unwrap();
    let err = paint(&mut s, "ws_1", "c").unwrap_err();
    assert_eq!(
        err,
        SubmitError::Rooms(RegistryError::WorkspaceFull { limit: 2 })
    );
    assert_eq!(
        s.open("ws_1", "c").unwrap_err(),
        RegistryError::WorkspaceFull { limit: 2 }
    );
    // An open file still answers, another workspace is unaffected, and closing frees a slot.
    paint(&mut s, "ws_1", "a").unwrap();
    paint(&mut s, "ws_2", "a").unwrap();
    s.close("ws_1", "a");
    paint(&mut s, "ws_1", "c").unwrap();
}

#[test]
fn a_bad_workspace_or_file_name_is_refused_before_anything_opens() {
    let mut s = server(1);
    let long = "x".repeat(129);
    let bad = ["", "a\0b", "a\nb", "a\u{7f}b", long.as_str()];
    for name in bad {
        for (ws, file) in [("ws_1", name), (name, "f")] {
            assert!(
                matches!(s.open(ws, file), Err(RegistryError::InvalidName { .. })),
                "open {ws:?} {file:?}"
            );
            let err = s
                .submit_tool("paint", file, &who(ws), vec![add(1)])
                .unwrap_err();
            assert!(
                matches!(err, SubmitError::Rooms(RegistryError::InvalidName { .. })),
                "submit {ws:?} {file:?}"
            );
        }
    }
    assert!(s.with_room("ws_1", "", |_| ()).is_none());
    // Nothing was opened: the one-room limit of the workspace is still free.
    paint(&mut s, "ws_1", "ok").unwrap();
    // The longest name still passes.
    paint(&mut s, "ws_2", &"y".repeat(128)).unwrap();
}

#[test]
fn a_read_tool_with_a_write_op_is_refused_without_opening_the_room() {
    let mut s = server(1);
    paint(&mut s, "ws_1", "held").unwrap();
    // The workspace is at its limit: opening "other" would fail with WorkspaceFull, so a refusal here
    // proves the room was never opened.
    let err = s
        .submit_tool("peek", "other", &who("ws_1"), vec![add(1)])
        .unwrap_err();
    assert!(
        matches!(err, SubmitError::Refused(Refusal::Forbidden { .. })),
        "{err:?}"
    );
    assert_eq!(err.refusal().map(Refusal::code), Some("forbidden"));
}

#[test]
fn an_oversize_batch_is_refused_before_the_room_opens_or_a_key_is_looked_up() {
    let mut s = server(1);
    paint(&mut s, "ws_1", "held").unwrap();
    let many: Vec<Op> = (0..=MAX_BATCH_OPS).map(add).collect();
    let err = s
        .submit_tool("paint", "other", &who("ws_1"), many)
        .unwrap_err();
    assert!(
        matches!(err, SubmitError::Refused(Refusal::Invalid { .. })),
        "{err:?}"
    );
    // A batch of exactly the cap is taken.
    let full: Vec<Op> = (0..MAX_BATCH_OPS).map(add).collect();
    s.submit_tool("paint", "held", &who("ws_1"), full).unwrap();
}

#[test]
fn names_are_held_to_the_strict_rule_for_workspace_and_file_alike() {
    let mut s = server(8);
    let bad = [
        "a\u{202E}b",
        "a\u{200B}b",
        "a\u{2028}b",
        "a\u{2029}b",
        "a\u{FEFF}b",
        "a\u{2066}b",
        "a\u{85}b",
        "   ",
        "\t",
    ];
    for name in bad {
        assert!(
            matches!(s.open("ws_1", name), Err(RegistryError::InvalidName { .. })),
            "file {name:?}"
        );
        assert!(
            matches!(s.open(name, "f"), Err(RegistryError::InvalidName { .. })),
            "workspace {name:?}"
        );
    }
    for file in [".", ".."] {
        assert!(matches!(
            s.open("ws_1", file),
            Err(RegistryError::InvalidName { .. })
        ));
    }
    // A dot is a fine workspace name, and ordinary Unicode is fine.
    s.open(".", "f").unwrap();
    s.open("ws_é", "fichier ü").unwrap();
}

#[test]
fn idle_rooms_free_their_slots_when_a_read_only_workspace_opens_more() {
    let mut s = server(2).with_idle_after(30);
    s.open("ws_1", "a").unwrap();
    s.open("ws_1", "b").unwrap();
    assert!(s.open("ws_1", "c").is_err(), "nothing is idle yet");
    std::thread::sleep(std::time::Duration::from_millis(80));
    s.open("ws_1", "c").unwrap();
    assert!(s.with_room("ws_1", "a", |_| ()).is_none());
}

#[test]
fn tick_closes_idle_rooms() {
    let mut s = server(8).with_idle_after(30);
    s.open("ws_1", "a").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(80));
    s.tick(0);
    assert!(s.with_room("ws_1", "a", |_| ()).is_none());
}
