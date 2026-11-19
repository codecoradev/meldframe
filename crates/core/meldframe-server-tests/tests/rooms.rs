//! The server serves one room per file, and every write goes through the gate into the room of its file.

use meldframe_host::{Op, Principal, Refusal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Server, SubmitError};

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn who(workspace: &str) -> Principal {
    Principal::new(workspace, "agent:test")
}

fn add(key: &str, id: &str) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame"})).unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn ids(server: &Server, file: &str) -> Vec<String> {
    ids_in(server, "ws_1", file)
}

fn ids_in(server: &Server, ws: &str, file: &str) -> Vec<String> {
    server
        .with_room(ws, file, |r| {
            r.nodes().iter().map(|n| n.id.clone()).collect()
        })
        .expect("file is open")
}

fn server() -> Server {
    Server::assemble_with(vec![Box::new(Painter)])
}

#[test]
fn ops_in_one_file_do_not_appear_in_the_other() {
    let mut s = server();
    let w = who("ws_1");
    s.submit_tool("paint", "file_a", &w, vec![add("k1", "a1")])
        .unwrap();
    s.submit_tool(
        "paint",
        "file_b",
        &w,
        vec![add("k1", "b1"), add("k2", "b2")],
    )
    .unwrap();
    assert_eq!(ids(&s, "file_a"), ["a1"]);
    assert_eq!(ids(&s, "file_b"), ["b1", "b2"]);
    // Sequence numbers are per file, and the same key in two files is two writes.
    assert_eq!(s.open("ws_1", "file_a").unwrap(), 1);
    assert_eq!(s.open("ws_1", "file_b").unwrap(), 2);
}

#[test]
fn a_write_names_its_file_and_opens_it_on_demand() {
    let mut s = server();
    assert!(s.with_room("ws_1", "file_a", |_| ()).is_none());
    let done = s
        .submit_tool("paint", "file_a", &who("ws_1"), vec![add("k1", "a1")])
        .unwrap();
    assert_eq!((done[0].seq, done[0].written), (1, true));
    assert!(s.with_room("ws_1", "file_a", |_| ()).is_some());
    assert!(s.with_room("ws_1", "file_b", |_| ()).is_none());
}

#[test]
fn a_retried_key_writes_nothing_and_keeps_its_number() {
    let mut s = server();
    let w = who("ws_1");
    s.submit_tool("paint", "f", &w, vec![add("k1", "a1")])
        .unwrap();
    let again = s
        .submit_tool("paint", "f", &w, vec![add("k1", "a1")])
        .unwrap();
    assert_eq!((again[0].seq, again[0].written), (1, false));
    assert_eq!(ids(&s, "f"), ["a1"]);
}

#[test]
fn another_workspace_gets_an_empty_file_of_that_name_never_the_content() {
    let mut s = server();
    s.submit_tool("paint", "f", &who("ws_1"), vec![add("k1", "a1")])
        .unwrap();
    // While open, and after a close (the store is keyed by workspace too): no oracle, no content.
    assert_eq!(s.open("ws_2", "f").unwrap(), 0);
    s.close("ws_1", "f");
    s.close("ws_2", "f");
    assert_eq!(s.open("ws_2", "f").unwrap(), 0);
    assert!(ids_in(&s, "ws_2", "f").is_empty());
    s.submit_tool("paint", "f", &who("ws_2"), vec![add("k1", "x")])
        .unwrap();
    s.close("ws_2", "f");
    assert_eq!(s.open("ws_1", "f").unwrap(), 1);
    assert_eq!(ids_in(&s, "ws_1", "f"), ["a1"]);
    s.open("ws_2", "f").unwrap();
    assert_eq!(ids_in(&s, "ws_2", "f"), ["x"]);
}

#[test]
fn workspace_and_file_names_cannot_be_made_to_collide() {
    let mut s = server();
    s.submit_tool("paint", "c", &who("a/b"), vec![add("k1", "one")])
        .unwrap();
    s.submit_tool("paint", "b/c", &who("a"), vec![add("k1", "two")])
        .unwrap();
    assert_eq!(ids_in(&s, "a/b", "c"), ["one"]);
    assert_eq!(ids_in(&s, "a", "b/c"), ["two"]);
    // The boundary cannot move: ("ab", "c") is not ("a", "bc"), and multibyte names count in bytes.
    s.submit_tool("paint", "c", &who("ab"), vec![add("k1", "x")])
        .unwrap();
    s.submit_tool("paint", "bc", &who("a"), vec![add("k1", "y")])
        .unwrap();
    s.submit_tool("paint", "é", &who("wé"), vec![add("k1", "m1")])
        .unwrap();
    s.submit_tool("paint", "é", &who("w"), vec![add("k1", "m2")])
        .unwrap();
    s.submit_tool("paint", "wé", &who("é"), vec![add("k1", "m3")])
        .unwrap();
    assert_eq!(ids_in(&s, "ab", "c"), ["x"]);
    assert_eq!(ids_in(&s, "a", "bc"), ["y"]);
    assert_eq!(ids_in(&s, "wé", "é"), ["m1"]);
    assert_eq!(ids_in(&s, "w", "é"), ["m2"]);
    assert_eq!(ids_in(&s, "é", "wé"), ["m3"]);
}

#[test]
fn a_failing_call_does_not_close_a_room_that_was_already_open() {
    let mut s = server();
    let w = who("ws_1");
    s.submit_tool("paint", "f", &w, vec![add("k1", "a1")])
        .unwrap();
    s.open("ws_1", "f").unwrap();
    let rm = Op::new("k2", Command::RemoveNode { id: "a1".into() });
    assert!(s.submit_tool("paint", "f", &w, vec![rm]).is_err());
    assert_eq!(ids(&s, "f"), ["a1"]);
}

#[test]
fn a_batch_that_fails_part_way_keeps_its_earlier_ops_and_a_retry_is_safe() {
    let mut s = server();
    let w = who("ws_1");
    let batch = || vec![add("k1", "a1"), add("k2", "a1")];
    let err = s.submit_tool("paint", "f", &w, batch()).unwrap_err();
    assert!(matches!(err, SubmitError::Rooms(_)), "{err:?}");
    assert!(err.refusal().is_some());
    assert_eq!(s.open("ws_1", "f").unwrap(), 1);
    assert_eq!(ids(&s, "f"), ["a1"]);
    let again = s.submit_tool("paint", "f", &w, batch()).unwrap_err();
    assert!(again.refusal().is_some(), "{again:?}");
    assert_eq!(s.open("ws_1", "f").unwrap(), 1);
    // Sent alone, the first op is a duplicate of the earlier write.
    let first = s
        .submit_tool("paint", "f", &w, vec![add("k1", "a1")])
        .unwrap();
    assert_eq!((first[0].seq, first[0].written), (1, false));
}

#[test]
fn a_duplicate_is_per_actor_and_per_workspace() {
    let mut s = server();
    let a = Principal::new("ws_1", "agent:a");
    let b = Principal::new("ws_1", "agent:b");
    s.submit_tool("paint", "f", &a, vec![add("k1", "n1")])
        .unwrap();
    let other_actor = s
        .submit_tool("paint", "f", &b, vec![add("k1", "n2")])
        .unwrap();
    assert_eq!((other_actor[0].seq, other_actor[0].written), (2, true));
    let other_ws = s
        .submit_tool(
            "paint",
            "f",
            &Principal::new("ws_2", "agent:a"),
            vec![add("k1", "n1")],
        )
        .unwrap();
    assert_eq!((other_ws[0].seq, other_ws[0].written), (1, true));
}

fn read_tool_server() -> Server {
    struct Looker;
    impl Module for Looker {
        fn name(&self) -> &'static str {
            "looker"
        }
        fn register(&self, r: &mut Registrar) {
            r.agent_tool("look", "Look.", RiskTier::Read);
        }
    }
    Server::assemble_with(vec![Box::new(Painter), Box::new(Looker)])
}

#[test]
fn a_refused_call_opens_no_room_and_cannot_fill_the_registry() {
    let mut s = read_tool_server();
    let w = who("ws_1");
    for i in 0..300 {
        let f = format!("f{i}");
        assert!(s.submit_tool("nope", &f, &w, vec![add("k", "a")]).is_err());
        assert!(s.submit_tool("look", &f, &w, vec![add("k", "a")]).is_err());
        assert!(s.submit_tool("paint", &f, &w, vec![]).is_err());
        assert!(s.with_room("ws_1", &f, |_| ()).is_none(), "{f}");
    }
    s.submit_tool("paint", "real", &w, vec![add("k1", "a1")])
        .unwrap();
}

#[test]
fn an_idle_room_is_closed_by_the_next_write() {
    let mut s = server().with_idle_after(0);
    let w = who("ws_1");
    s.submit_tool("paint", "a", &w, vec![add("k1", "a1")])
        .unwrap();
    s.submit_tool("paint", "b", &w, vec![add("k1", "b1")])
        .unwrap();
    assert!(s.with_room("ws_1", "a", |_| ()).is_none());
    // It comes back from its store.
    assert_eq!(s.open("ws_1", "a").unwrap(), 1);
}

#[test]
fn retrying_a_small_delete_is_a_duplicate_not_a_refusal() {
    let mut s = server();
    let w = who("ws_1");
    let rect =
        serde_json::from_value(serde_json::json!({"id": "r", "type": "rect", "parentId": null}))
            .unwrap();
    s.submit_tool(
        "paint",
        "f",
        &w,
        vec![Op::new(
            "k1",
            Command::AddNode {
                node: rect,
                index: None,
            },
        )],
    )
    .unwrap();
    let rm = || Op::new("k2", Command::RemoveNode { id: "r".into() });
    let first = s.submit_tool("paint", "f", &w, vec![rm()]).unwrap();
    assert_eq!((first[0].seq, first[0].written), (2, true));
    let again = s.submit_tool("paint", "f", &w, vec![rm()]).unwrap();
    assert_eq!((again[0].seq, again[0].written), (2, false));
    // A retry mixed with a new op keeps the order of the batch.
    let mixed = s
        .submit_tool("paint", "f", &w, vec![rm(), add("k3", "n3")])
        .unwrap();
    assert_eq!(
        mixed.iter().map(|a| (a.seq, a.written)).collect::<Vec<_>>(),
        [(2, false), (3, true)]
    );
    // The same key with another command is still refused.
    let other = add("k2", "other");
    let err = s.submit_tool("paint", "f", &w, vec![other]).unwrap_err();
    assert_eq!(err.refusal().map(Refusal::code), Some("invalid"), "{err:?}");
}

#[test]
fn gate_and_room_refusals_have_one_shape() {
    let mut s = server();
    let w = who("ws_1");
    let gate = s
        .submit_tool("nope", "f", &w, vec![add("k1", "a1")])
        .unwrap_err();
    assert_eq!(gate.refusal().map(Refusal::code), Some("forbidden"));
    s.submit_tool("paint", "f", &w, vec![add("k1", "a1")])
        .unwrap();
    let room = s
        .submit_tool("paint", "f", &w, vec![add("k1", "different")])
        .unwrap_err();
    assert!(matches!(room, SubmitError::Rooms(_)));
    assert_eq!(room.refusal().map(Refusal::code), Some("invalid"));
}

#[test]
fn a_refused_batch_writes_nothing_to_its_file() {
    let mut s = server();
    let rm = Op::new("k2", Command::RemoveNode { id: "a1".into() });
    let err = s
        .submit_tool("paint", "f", &who("ws_1"), vec![add("k1", "a1"), rm])
        .unwrap_err();
    // A frame delete is above a Change tool: the whole batch is refused before the room sees it.
    assert!(
        matches!(err, SubmitError::Refused(Refusal::Forbidden { .. })),
        "{err:?}"
    );
    // The failed call left no room open, and nothing in the store.
    assert!(s.with_room("ws_1", "f", |_| ()).is_none());
    assert_eq!(s.open("ws_1", "f").unwrap(), 0);
}

#[test]
fn a_closed_file_comes_back_from_its_store_with_its_content() {
    let mut s = server();
    let w = who("ws_1");
    s.submit_tool("paint", "f", &w, vec![add("k1", "a1"), add("k2", "a2")])
        .unwrap();
    assert!(s.close("ws_1", "f"));
    assert!(s.with_room("ws_1", "f", |_| ()).is_none());
    assert_eq!(s.open("ws_1", "f").unwrap(), 2);
    assert_eq!(ids(&s, "f"), ["a1", "a2"]);
}

#[test]
fn close_all_closes_a_room_that_only_a_subscription_opened() {
    let mut s = server();
    s.subscribe(&who("ws_1"), "f", 0, 0).unwrap();
    assert!(s.with_room("ws_1", "f", |_| ()).is_some());
    s.close_all();
    assert!(s.with_room("ws_1", "f", |_| ()).is_none());
}
