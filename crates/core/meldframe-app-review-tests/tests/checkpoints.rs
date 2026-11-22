//! E5.3, ADR-0023: the checkpoints of the file as the server keeps them. The list, a frame, a gap, the second check of
//! what the server sends, the bounds of the list and the wire shape.

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{
    Action, Checkpoint, CheckpointKind as Kind, CheckpointRequest, CheckpointRow,
    CheckpointsStatus, Effect, Frame,
};

fn row(n: u32, kind: Kind) -> CheckpointRow {
    let by = (kind != Kind::RunStart).then(|| "user:me".to_string());
    let about = (kind == Kind::RunStart).then(|| "agent:x".to_string());
    CheckpointRow {
        n,
        seq: u64::from(n),
        kind,
        label: None,
        about,
        by,
        at_ms: 1,
    }
}

fn fetches(fx: &[Effect]) -> usize {
    fx.iter()
        .filter(|e| matches!(e, Effect::FetchCheckpoints {}))
        .count()
}

fn frame(l: &mut Loopback, checkpoint: CheckpointRow) -> Vec<Effect> {
    l.send(Action::Frame {
        now_ms: 0,
        frame: Frame::Checkpoint { checkpoint },
    })
}

fn ids(l: &Loopback) -> Vec<String> {
    l.app
        .state()
        .checkpoints
        .iter()
        .map(|c| c.id.clone())
        .collect()
}

fn loaded(l: &mut Loopback, rows: Vec<CheckpointRow>, can_create: bool) {
    l.send(Action::CheckpointsLoaded {
        checkpoints: rows,
        can_create,
    });
}

#[test]
fn the_list_is_what_the_server_said_newest_first_with_the_text_of_n_as_the_id() {
    let mut l = Loopback::live();
    assert!(
        !l.app.state().can_create_checkpoint,
        "nothing is allowed before the list is read"
    );
    assert_eq!(l.app.state().checkpoints_status, CheckpointsStatus::Unread);
    let mut manual = row(2, Kind::Manual);
    manual.label = Some("  Before the logo ".into());
    loaded(
        &mut l,
        vec![row(1, Kind::RunStart), manual, row(3, Kind::BeforeRestore)],
        true,
    );
    assert_eq!(ids(&l), ["3", "2", "1"]);
    let state = l.app.state();
    assert!(state.can_create_checkpoint);
    assert_eq!(state.checkpoints_status, CheckpointsStatus::Loaded);
    let two = &state.checkpoints[1];
    assert_eq!(
        (two.n, two.label.as_deref(), two.by.as_deref()),
        (2, Some("Before the logo"), Some("user:me"))
    );
    assert!(
        l.events
            .iter()
            .any(|e| matches!(e, meldframe_app::Event::CheckpointsChanged { count: 3 }))
    );
    // The same list again says nothing new; a shorter one replaces it.
    let before = l.events.len();
    loaded(
        &mut l,
        vec![row(3, Kind::BeforeRestore), row(2, Kind::Manual)],
        false,
    );
    assert_eq!(
        (ids(&l), l.app.state().can_create_checkpoint),
        (vec!["3".to_string(), "2".to_string()], false)
    );
    assert_eq!(l.events.len(), before + 1);
}

#[test]
fn a_row_the_server_never_makes_is_left_out_and_a_label_not_fit_to_show_is_left_off() {
    let mut l = Loopback::live();
    let with = |f: &dyn Fn(&mut CheckpointRow)| {
        let mut r = row(7, Kind::Manual);
        f(&mut r);
        r
    };
    let rows = vec![
        with(&|r| r.n = 0),
        with(&|r| r.by = None),
        with(&|r| r.about = Some("agent:x".into())),
        with(&|r| r.by = Some("a\u{202e}b".into())),
        with(&|r| r.kind = Kind::RunStart),
        row(1, Kind::Manual),
        row(1, Kind::BeforeRevert),
    ];
    let mut bad_label = with(&|r| r.label = Some("pay\u{202e}now".into()));
    bad_label.n = 9;
    let mut restore_label = row(8, Kind::BeforeRestore);
    restore_label.label = Some("a label".into());
    loaded(
        &mut l,
        [rows, vec![bad_label, restore_label]].concat(),
        true,
    );
    // Row 1 once (the first of the two with that number), and row 9 without its label.
    assert_eq!(ids(&l), ["9", "1"]);
    assert_eq!(l.app.state().checkpoints[0].label, None);
}

#[test]
fn a_frame_in_order_joins_the_list_and_a_gap_reads_the_list_again() {
    let mut l = Loopback::live();
    loaded(
        &mut l,
        vec![row(1, Kind::RunStart), row(2, Kind::Manual)],
        true,
    );
    assert_eq!(
        fetches(&frame(&mut l, row(3, Kind::Manual))),
        0,
        "n 3 follows 2"
    );
    assert_eq!(ids(&l), ["3", "2", "1"]);
    assert_eq!(
        fetches(&frame(&mut l, row(3, Kind::Manual))),
        0,
        "a row held is the same row"
    );
    let fx = frame(&mut l, row(5, Kind::BeforeRevert));
    assert_eq!(fetches(&fx), 1, "n 4 is missing");
    assert_eq!(ids(&l), ["5", "3", "2", "1"], "the row is shown meanwhile");
    // One read at a time: a second gap while it is out asks for nothing.
    assert_eq!(fetches(&frame(&mut l, row(9, Kind::Manual))), 0);
    loaded(
        &mut l,
        (1..=9).map(|n| row(n, Kind::Manual)).collect(),
        true,
    );
    assert_eq!(
        fetches(&frame(&mut l, row(12, Kind::Manual))),
        1,
        "and again once it was answered"
    );
}

#[test]
fn a_row_that_arrived_while_a_read_was_out_is_not_lost_when_the_older_answer_comes() {
    let mut l = Loopback::live();
    loaded(
        &mut l,
        vec![row(1, Kind::RunStart), row(2, Kind::Manual)],
        true,
    );
    // A read is out (a gap asked for it); a frame and then the answer of the read, made before that row existed.
    assert_eq!(fetches(&frame(&mut l, row(4, Kind::Manual))), 1);
    frame(&mut l, row(5, Kind::BeforeRevert));
    loaded(
        &mut l,
        vec![
            row(1, Kind::RunStart),
            row(2, Kind::Manual),
            row(3, Kind::Manual),
        ],
        true,
    );
    assert_eq!(
        ids(&l),
        ["5", "4", "3", "2", "1"],
        "what is newer than the answer stays"
    );
    // A row the answer holds is the answer's, and a row rolled out of the answer is not kept for being old.
    loaded(
        &mut l,
        vec![
            row(3, Kind::Manual),
            row(4, Kind::Manual),
            row(5, Kind::BeforeRevert),
        ],
        true,
    );
    assert_eq!(ids(&l), ["5", "4", "3"]);
}

#[test]
fn a_frame_while_the_connection_is_down_asks_for_nothing_and_a_subscribe_reads_the_list() {
    let mut l = Loopback::live();
    l.app.dispatch(Action::Disconnected {});
    assert_eq!(fetches(&frame(&mut l, row(4, Kind::Manual))), 0);
    let fx = l.app.dispatch(Action::Connected { now_ms: 5 });
    assert_eq!(fetches(&fx), 1, "after a subscribe");
    let fx = l.app.dispatch(Action::Frame {
        now_ms: 6,
        frame: Frame::Resync {
            after_seq: 0,
            latest_seq: 0,
        },
    });
    assert_eq!(fetches(&fx), 1, "after a resync, even with a read out");
    let fx = l.app.dispatch(Action::OpenActivity {});
    assert_eq!(fetches(&fx), 0, "a read is out already");
    loaded(&mut l, vec![], true);
    assert_eq!(
        fetches(&l.app.dispatch(Action::OpenActivity {})),
        1,
        "opening the Activity tab"
    );
}

#[test]
fn a_new_run_start_rolls_the_oldest_of_its_share_out_as_the_server_does() {
    let mut l = Loopback::live();
    let runs = |l: &Loopback| {
        l.app
            .state()
            .checkpoints
            .iter()
            .filter(|c| c.kind == Kind::RunStart)
            .count()
    };
    let mut rows: Vec<CheckpointRow> = (1..=25).map(|n| row(n, Kind::RunStart)).collect();
    for r in rows.iter_mut() {
        r.about = Some(format!("agent:a{}", r.n));
    }
    rows.push(row(26, Kind::Manual));
    loaded(&mut l, rows, true);
    assert_eq!(runs(&l), 25);
    let mut next = row(27, Kind::RunStart);
    next.about = Some("agent:new".into());
    frame(&mut l, next);
    assert_eq!(runs(&l), 25, "the share is full: the oldest went");
    assert!(!ids(&l).contains(&"1".to_string()) && ids(&l).contains(&"26".to_string()));
    // An agent's eleventh row rolls its own oldest, and the share still rolls the oldest of all.
    for n in 28..=38 {
        let mut r = row(n, Kind::RunStart);
        r.about = Some("agent:busy".into());
        frame(&mut l, r);
    }
    let busy = l
        .app
        .state()
        .checkpoints
        .iter()
        .filter(|c| c.about.as_deref() == Some("agent:busy"))
        .count();
    assert_eq!((runs(&l), busy), (25, 10));
    let held = ids(&l);
    assert!(
        !held.contains(&"28".to_string()) && held.contains(&"29".to_string()),
        "its own oldest went"
    );
    assert!(
        !held.contains(&"11".to_string()) && held.contains(&"12".to_string()),
        "and the share's oldest rows"
    );
}

#[test]
fn a_server_without_the_module_leaves_no_list_and_no_noise() {
    let mut l = Loopback::live();
    loaded(&mut l, vec![row(1, Kind::Manual)], true);
    let request = CheckpointRequest::List {};
    l.send(Action::CheckpointRequestFailed {
        request: request.clone(),
        code: "unavailable".into(),
    });
    assert_eq!(refused(&mut l), vec!["unavailable"]);
    assert_eq!(ids(&l), ["1"], "a failed read changes nothing");
    assert_eq!(l.app.state().checkpoints_status, CheckpointsStatus::Loaded);
    l.send(Action::CheckpointRequestFailed {
        request,
        code: "not_found".into(),
    });
    assert!(refused(&mut l).is_empty());
    assert!(l.app.state().checkpoints.is_empty() && !l.app.state().can_create_checkpoint);
    assert_eq!(l.app.state().checkpoints_status, CheckpointsStatus::Absent);
    // Read again, the module is there after all.
    loaded(&mut l, vec![row(1, Kind::Manual)], true);
    assert_eq!(l.app.state().checkpoints_status, CheckpointsStatus::Loaded);
}

#[test]
fn the_actions_effects_frame_and_list_have_a_wire_shape() {
    let list = serde_json::json!({
        "type": "checkpointsLoaded", "canCreate": true,
        "checkpoints": [{"n": 2, "seq": 9, "kind": "runStart", "about": "agent:x", "atMs": 5}]
    });
    let a: Action = serde_json::from_value(list).unwrap();
    assert!(
        matches!(a, Action::CheckpointsLoaded { ref checkpoints, .. } if checkpoints[0].kind == Kind::RunStart)
    );
    let extra = serde_json::json!({"type": "checkpointsLoaded", "canCreate": true, "checkpoints": [], "x": 1});
    assert!(serde_json::from_value::<Action>(extra).is_err());
    let failed = serde_json::json!({"type": "checkpointRequestFailed", "request": {"type": "list"}, "code": "forbidden"});
    assert!(serde_json::from_value::<Action>(failed).is_ok());
    let f: Frame = serde_json::from_value(serde_json::json!({
        "type": "checkpoint", "checkpoint": {"n": 1, "seq": 0, "kind": "manual", "by": "user:me", "atMs": 1}
    }))
    .unwrap();
    assert!(matches!(f, Frame::Checkpoint { .. }));
    let e = serde_json::to_value(Effect::FetchCheckpoints {}).unwrap();
    assert_eq!(e, serde_json::json!({"type": "fetchCheckpoints"}));
    let mut l = Loopback::live();
    loaded(&mut l, vec![row(4, Kind::Manual)], true);
    let v = serde_json::to_value(l.app.state()).unwrap();
    assert_eq!(v["canCreateCheckpoint"], true);
    assert_eq!(v["checkpointsStatus"], "loaded");
    let c: &Checkpoint = &l.app.state().checkpoints[0];
    assert_eq!(v["checkpoints"][0], serde_json::to_value(c).unwrap());
    assert_eq!(
        (
            v["checkpoints"][0]["id"].as_str(),
            v["checkpoints"][0]["kind"].as_str()
        ),
        (Some("4"), Some("manual"))
    );
    assert!(
        v["checkpoints"][0].get("label").is_none() && v["checkpoints"][0].get("about").is_none()
    );
}
