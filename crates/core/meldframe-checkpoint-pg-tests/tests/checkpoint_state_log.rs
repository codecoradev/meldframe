//! The state route over logs that are hard on the replay (ADR-0023, slice 6): a snapshot below ops that touch a
//! variable, and a log the server cannot work out. The replay starts from the snapshot's nodes and its variables, and
//! a log that does not replay is an error that is not retryable, with a plain code and no detail of the replay.

mod world;
use meldframe_checkpoints::{Kind, NewCheckpoint};
use meldframe_host::{Principal, Snapshot, Store, StoredOp};
use meldframe_http::mcp::Reader;
use meldframe_revert::{LoggedOp, state_at};
use meldframe_server::{Server, SharedServer};
use meldframe_store_pg::{Backing, room_key};
use serde_json::{Value, json};
use std::sync::Arc;
use world::*;

use checkpoint_store::CheckpointStore;

const WS: &str = "ws_1";

fn op(key: &str, command: Value) -> Value {
    json!({"key": key, "command": command})
}

fn logged(ops: Vec<StoredOp>) -> Vec<LoggedOp> {
    let one = |o: StoredOp| LoggedOp {
        seq: o.seq,
        actor: o.actor,
        key: o.key,
        command: o.command,
    };
    ops.into_iter().map(one).collect()
}

async fn write(w: &World, ops: Vec<Value>) {
    for step in ops {
        let (status, body) = rest(w, AGENT, "f", vec![step]).await;
        assert_eq!(status, 200, "{body}");
    }
}

fn row(w: &World, seq: u64) -> u32 {
    let new = NewCheckpoint::by_person(Kind::Manual, seq, None, None, "user:bea").unwrap();
    w.store.create(WS, "f", None, &new, 1).unwrap().checkpoint.n
}

async fn state(w: &World, n: u32) -> (u16, Value) {
    get(
        w,
        Some(PERSON),
        &format!("/v1/files/f/checkpoints/{n}/state"),
    )
    .await
}

fn collection() -> Value {
    json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance",
        "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}})
}

fn variable() -> Value {
    json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
        "values": {"light": "#ffffff", "dark": "#000000"}}})
}

#[tokio::test]
async fn a_snapshot_below_ops_that_touch_a_variable_seeds_the_replay_with_its_variables() {
    let backing = Backing::memory();
    let server = Arc::new(SharedServer::new(Server::assemble_in(
        Vec::new(),
        backing.clone(),
    )));
    let w = start_on(server, Spec::default()).await;
    write(
        &w,
        vec![
            op(
                "k1",
                json!({"type": "addNode", "node": {"id": "fa", "type": "rect", "name": "fa"}}),
            ),
            op("k2", collection()),
            op("k3", variable()),
        ],
    )
    .await;
    // The snapshot holds the nodes and the variables as they are after op 3, as a room keeps them.
    let reader = Principal::new(WS, "user:bea");
    let (seq, variables) = w.server.variables(&reader, "f").unwrap();
    let nodes = w.server.view(&reader, "f").unwrap().nodes;
    assert_eq!(seq, 3);
    let snapshot = Snapshot {
        seq,
        nodes,
        variables,
    };
    backing
        .store()
        .save_snapshot(&room_key(WS, "f"), snapshot)
        .unwrap();
    write(
        &w,
        vec![
            op(
                "k4",
                json!({"type": "updateNode", "id": "fa", "patch": {"fill": {"var": "bg"}}}),
            ),
            op(
                "k5",
                json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
            ),
            op(
                "k6",
                json!({"type": "updateNode", "id": "fa", "patch": {"fill": "#ffffff"}}),
            ),
            op("k7", json!({"type": "removeVariable", "id": "bg"})),
        ],
    )
    .await;
    let log = logged(
        backing
            .store()
            .ops_after(&room_key(WS, "f"), 0, 100)
            .unwrap(),
    );
    assert_eq!(log.len(), 7);
    for seq in 3..=7 {
        w.past.reset();
        let (status, body) = state(&w, row(&w, seq)).await;
        assert_eq!(status, 200, "at seq {seq}: {body}");
        assert_eq!(
            body,
            json!({"seq": seq, "nodes": state_at(&[], &log, seq).unwrap()}),
            "at seq {seq}"
        );
        assert_eq!(
            w.past.afters.lock().unwrap().first().copied(),
            (seq > 3).then_some(3),
            "from the snapshot at seq {seq}"
        );
    }
}

#[tokio::test]
async fn a_log_that_does_not_replay_is_an_error_that_is_not_retryable_and_tells_nothing_of_the_replay()
 {
    let w = start().await;
    write(
        &w,
        vec![
            op(
                "k1",
                json!({"type": "addNode", "node": {"id": "fa", "type": "rect", "name": "fa"}}),
            ),
            op(
                "k2",
                json!({"type": "updateNode", "id": "fa", "patch": {"x": 5}}),
            ),
        ],
    )
    .await;
    for fault in [Fault::Corrupt, Fault::Short] {
        *w.past.fault.lock().unwrap() = Some(fault);
        let (status, body) = state(&w, row(&w, 2)).await;
        let error = &body["error"];
        assert_eq!(
            (status, error["code"].as_str(), error["retryable"].clone()),
            (500, Some("internal"), json!(false)),
            "{body}"
        );
        let said = error["message"].as_str().unwrap().to_lowercase();
        for detail in ["replay", "seq", "op ", "log"] {
            assert!(
                !said.contains(detail),
                "the message gives away `{detail}`: {said}"
            );
        }
    }
    // A log that is fine again answers: nothing was kept of the failures.
    *w.past.fault.lock().unwrap() = None;
    assert_eq!(state(&w, row(&w, 2)).await.0, 200);
}
