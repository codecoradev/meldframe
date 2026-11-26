//! Rule U9 at the app seam, for what the server sends in a live frame: the actor of an op (a lane key, a layer's last
//! actor, the actor of a change event), the idempotency key of an op (a row's key prefix), a touch and the code of a
//! refusal. Each is an identifier: made safe with the strict profile and cut, once, where the frame enters the app
//! layer. The replica and the file keep what the server sent, so the file never drifts.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Event, Frame};
use meldframe_app_model::{BatchRef, CheckpointRequest};
use meldframe_protocol::{Command, NodePatch};

const T: u64 = 1_700_000_000_000;

fn paint(id: &str, x: f64) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            x: Some(x),
            ..Default::default()
        },
    }
}

/// A live app and one op of `actor` under `key` on `r1`.
fn after_op(actor: &str, key: &str) -> Loopback {
    let mut l = Loopback::live();
    l.remote(actor, key, T, paint("r1", 50.0));
    l
}

fn lanes(l: &Loopback) -> Vec<String> {
    let lanes = &l.app.state().activity.lanes;
    lanes.iter().map(|x| x.actor.clone()).collect()
}

fn rows(l: &Loopback) -> Vec<(String, String)> {
    let rows = &l.app.state().activity.rows;
    let key = |r: &meldframe_app_model::BatchRow| (r.actor.clone(), r.key_prefix.clone());
    rows.iter().map(key).collect()
}

fn change_actors(l: &mut Loopback) -> Vec<String> {
    let all = l.app.take_events();
    l.events.extend(all);
    l.events
        .iter()
        .filter_map(|e| match e {
            Event::DocumentChanged { actor, .. } => Some(actor.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_op_of_an_actor_with_a_bidi_override_gets_a_clean_lane_a_clean_layer_actor_and_a_clean_event()
{
    let mut l = after_op("agent:x\u{202E}evil", "b1:0");
    assert_eq!(lanes(&l), ["agent:xevil"]);
    assert_eq!(l.app.state().last_actor["r1"], "agent:xevil");
    assert_eq!(change_actors(&mut l), ["agent:xevil"]);
    assert_eq!(rows(&l), [("agent:xevil".to_string(), "b1".to_string())]);
    // The file changed all the same: the replica holds the op as the server sent it.
    assert_eq!(get(&l.app, "r1").x, 50.0);
}

#[test]
fn each_hidden_character_of_the_table_goes_from_an_actor_and_from_a_key() {
    for c in [
        '\u{2800}', '\u{3164}', '\u{17B4}', '\u{200B}', '\u{00AD}', '\u{202E}',
    ] {
        let l = after_op(&format!("agent:a{c}b"), &format!("k{c}1:0"));
        assert_eq!(lanes(&l), ["agent:ab"], "U+{:04X}", c as u32);
        assert_eq!(rows(&l)[0].1, "k1", "U+{:04X}", c as u32);
    }
}

#[test]
fn a_128_character_actor_of_two_byte_characters_is_cut_between_characters_to_128_bytes() {
    let l = after_op(&"\u{00E9}".repeat(128), "b1:0");
    let lane = &lanes(&l)[0];
    assert_eq!(lane, &"\u{00E9}".repeat(64));
    assert_eq!(lane.len(), 128);
    assert_eq!(l.app.state().last_actor["r1"], *lane);
}

#[test]
fn an_actor_of_four_byte_characters_is_cut_at_the_byte_cap() {
    let l = after_op(&"\u{1F600}".repeat(100), "b1:0");
    assert_eq!(lanes(&l)[0], "\u{1F600}".repeat(32));
}

#[test]
fn a_zero_width_joiner_emoji_actor_is_an_identifier_so_the_joiners_go() {
    let l = after_op("agent:\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}", "b1:0");
    assert_eq!(lanes(&l), ["agent:\u{1F468}\u{1F469}\u{1F467}"]);
}

#[test]
fn an_actor_the_server_could_have_sent_comes_back_as_it_was() {
    for actor in ["agent:claude", "person:ana", "a\u{00A0}\u{00A0}b", " x "] {
        let l = after_op(actor, "b1:0");
        assert_eq!(lanes(&l), [actor]);
        assert_eq!(l.app.state().last_actor["r1"], actor);
    }
}

#[test]
fn an_op_of_an_actor_with_nothing_left_changes_the_file_and_makes_no_lane() {
    let mut l = after_op("\u{200B}\u{202E}", "\u{200B}:0");
    assert_eq!(get(&l.app, "r1").x, 50.0);
    assert!(lanes(&l).is_empty() && rows(&l).is_empty());
    assert!(l.app.state().last_actor.is_empty());
    // The change still reaches the front end, with the actor as nothing.
    assert_eq!(change_actors(&mut l), [""]);
}

#[test]
fn two_spellings_of_an_actor_meet_in_one_lane_the_one_a_clean_server_would_have_sent() {
    let mut l = after_op("agent:x", "b1:0");
    l.remote("agent:x\u{200B}", "b1:1", T + 1, paint("t1", 20.0));
    assert_eq!(lanes(&l), ["agent:x"]);
    assert_eq!(rows(&l), [("agent:x".to_string(), "b1".to_string())]);
}

#[test]
fn a_batch_of_a_hostile_actor_can_be_shown_and_reverted_by_its_clean_name() {
    let mut l = after_op("agent:x\u{202E}", "b1:0");
    let batch = BatchRef {
        actor: "agent:x".into(),
        key_prefix: "b1".into(),
    };
    l.app.dispatch(Action::SelectBatch {
        batch: batch.clone(),
    });
    assert_eq!(l.app.state().selection, ["r1"]);
    l.send(Action::RevertBatch {
        actor: batch.actor,
        key_prefix: batch.key_prefix,
    });
    assert_eq!(get(&l.app, "r1").x, 0.0, "the revert found the op");
}

#[test]
fn a_key_is_held_to_the_longest_idempotency_key_in_characters_and_bytes() {
    // 98 four-byte characters and ":0" are 100 characters and 394 bytes: the longest key, kept whole.
    let key = format!("{}:0", "\u{1F600}".repeat(98));
    let l = after_op("agent:x", &key);
    assert_eq!(rows(&l)[0].1, "\u{1F600}".repeat(98));
    // A key past 100 characters is cut, loses the number the batch rule reads, and so makes no row.
    let key = format!("{}:0", "\u{1F600}".repeat(150));
    let l = after_op("agent:x", &key);
    assert!(rows(&l).is_empty());
    assert_eq!(lanes(&l), ["agent:x"], "the op still feeds the lane");
}

#[test]
fn a_touch_of_a_hostile_actor_draws_a_ring_with_the_clean_name() {
    let mut l = Loopback::live();
    l.app.dispatch(Action::Frame {
        now_ms: T,
        frame: Frame::Touch {
            actor: "agent:x\u{202E}".into(),
            ids: vec!["r1".into()],
            seq: 1,
        },
    });
    let a = &l.app.state().activity;
    assert_eq!(
        a.touches
            .iter()
            .map(|t| t.actor.as_str())
            .collect::<Vec<_>>(),
        ["agent:x"]
    );
    assert_eq!(lanes(&l), ["agent:x"]);
    // A touch with no actor left draws nothing and makes no lane.
    let mut l = Loopback::live();
    l.app.dispatch(Action::Frame {
        now_ms: T,
        frame: Frame::Touch {
            actor: "\u{200B}".into(),
            ids: vec!["r1".into()],
            seq: 1,
        },
    });
    assert!(l.app.state().activity.touches.is_empty() && lanes(&l).is_empty());
}

/// Send a local edit and answer it with `Rejected` carrying `code`; the `OpRejected` event's code.
fn rejected_with(code: &str, retryable: bool) -> Vec<String> {
    let mut l = Loopback::live();
    let fx = l.app.dispatch(Action::Move {
        id: "r1".into(),
        x: 9.0,
        y: 9.0,
    });
    let key = match &fx[..] {
        [.., meldframe_app::Effect::SendOps { ops }] => ops[0].key.clone(),
        _ => panic!("the move sends an op: {fx:?}"),
    };
    l.app.dispatch(Action::Frame {
        now_ms: T,
        frame: Frame::Rejected {
            key,
            code: code.into(),
            retryable,
        },
    });
    l.app
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::OpRejected { code, .. } => Some(code),
            _ => None,
        })
        .collect()
}

#[test]
fn a_refusal_code_is_cleaned_and_cut_to_64_bytes_where_the_frame_enters() {
    assert_eq!(rejected_with("quota_exceeded", false), ["quota_exceeded"]);
    assert_eq!(rejected_with("bad\u{202E}code\u{200B}", false), ["badcode"]);
    let long = "x".repeat(10_000);
    assert_eq!(rejected_with(&long, false), ["x".repeat(64)]);
    let wide = "\u{1F600}".repeat(100);
    assert_eq!(rejected_with(&wide, false), ["\u{1F600}".repeat(16)]);
    // A code with nothing left is a code all the same.
    assert_eq!(rejected_with("\u{200B}", false), ["unknown"]);
    assert_eq!(rejected_with("", false), ["unknown"]);
}

#[test]
fn the_codes_of_a_failed_request_are_cleaned_the_same_way() {
    let mut l = Loopback::live();
    let hostile = "no\u{202E}pe";
    l.app.dispatch(Action::BatchRequestFailed {
        code: hostile.into(),
    });
    l.app.dispatch(Action::CheckpointRequestFailed {
        request: CheckpointRequest::List {},
        code: hostile.into(),
    });
    l.app.dispatch(Action::Select { ids: ids(&["r1"]) });
    let open = l.app.dispatch(Action::OpenAssets { id: "r1".into() });
    let request = open
        .iter()
        .find_map(|e| match e {
            meldframe_app::Effect::FetchAssets { request, .. } => Some(*request),
            _ => None,
        })
        .expect("a fetch");
    l.app.dispatch(Action::AssetsRequestFailed {
        request,
        code: "x".repeat(500),
    });
    let codes: Vec<String> = l
        .app
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect();
    assert!(codes.contains(&"nope".to_string()), "{codes:?}");
    assert!(
        codes
            .iter()
            .all(|c| !c.contains('\u{202E}') && c.len() <= 64)
    );
    assert_eq!(
        l.app.state().assets.error.as_deref(),
        Some("x".repeat(64).as_str())
    );
}

#[test]
fn stacked_combining_marks_in_an_actor_a_key_and_a_code_are_capped_at_four() {
    // A server that accepted an actor of 100 marks within 128 bytes could not have: the rule of `actor` refuses a run of
    // more than four. A frame that carries one reaches no screen with the pile (the marks past the fourth are dropped).
    let pile = "\u{0301}".repeat(100);
    let four = "\u{0301}".repeat(4);
    let l = after_op(&format!("agent:a{pile}"), &format!("k{pile}1:0"));
    assert_eq!(lanes(&l), [format!("agent:a{four}")]);
    assert_eq!(rows(&l)[0].1, format!("k{four}1"));
    assert_eq!(
        rejected_with(&format!("bad{pile}"), false),
        [format!("bad{four}")]
    );
    // Marks split by a hidden character are one run once it is gone.
    let split = "\u{0301}\u{200B}".repeat(50);
    assert_eq!(
        lanes(&after_op(&format!("agent:a{split}"), "k:0")),
        [format!("agent:a{four}")]
    );
}
