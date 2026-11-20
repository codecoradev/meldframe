//! The wire shape of what the activity panel adds to the app layer: the JSON the front end sends as actions and
//! frames, reads as effects and as state.

use meldframe_app::{Action, App, AppState, Effect, Frame};
use meldframe_app_model::ReviewState;
use serde_json::json;

fn from<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
    serde_json::from_value(v).unwrap()
}

#[test]
fn the_new_actions_and_frames_read_as_the_front_end_sends_them() {
    let a: Action = from(
        json!({"type":"setBatchReview","batch":{"actor":"agent:x","keyPrefix":"b1"},"state":"needs_review"}),
    );
    assert!(matches!(
        a,
        Action::SetBatchReview {
            state: ReviewState::NeedsReview,
            ..
        }
    ));
    let row = json!({"n":1,"rev":1,"actor":"agent:x","keyPrefix":"b1","tool":"apply_commands","commands":1,"written":1,
        "atomic":false,"firstSeq":1,"lastSeq":1,"atMs":5,"review":"unreviewed"});
    let page = json!({"batches":[row],"hasMore":false,"unreviewedCount":1,"canReview":false});
    assert!(matches!(
        from::<Action>(json!({"type":"batchesLoaded","page":page})),
        Action::BatchesLoaded { .. }
    ));
    for t in ["openActivity", "loadOlderBatches"] {
        from::<Action>(json!({ "type": t }));
    }
    from::<Action>(json!({"type":"batchRequestFailed","code":"forbidden"}));
    let reviewed: Action = from(json!({"type":"batchReviewed","batch":row}));
    assert!(
        reviewed.only_activity(),
        "it touches only the panel, so the app does not refresh the document for it"
    );
    let pick: Action =
        from(json!({"type":"selectBatch","batch":{"actor":"agent:x","keyPrefix":"b1"}}));
    assert!(
        pick.is_activity() && !pick.only_activity(),
        "it selects nodes, which the properties panel follows"
    );
    assert!(!Action::Undo {}.is_activity());
    let f: Frame = from(json!({"type":"touch","actor":"agent:x","ids":["a"],"seq":3}));
    assert!(matches!(f, Frame::Touch { .. }));
    assert!(serde_json::from_value::<Action>(json!({"type":"openActivity","x":1})).is_err());
}

#[test]
fn the_new_effects_and_the_state_write_as_the_front_end_reads_them() {
    let fetch = Effect::FetchBatches {
        before_n: Some(5),
        after_n: None,
        limit: 100,
    };
    let put = Effect::PutBatchReview {
        n: 2,
        state: ReviewState::Accepted,
    };
    let fx = serde_json::to_value([fetch, put]).unwrap();
    assert_eq!(
        fx[0],
        json!({"type":"fetchBatches","beforeN":5,"afterN":null,"limit":100})
    );
    assert_eq!(
        fx[1],
        json!({"type":"putBatchReview","n":2,"state":"accepted"})
    );
    let mut app = App::new("user:me", "c0", 1, vec![], 0).unwrap();
    app.dispatch(from(json!({"type":"frame","nowMs":9,"frame":{"type":"batch","unreviewedCount":1,"header":{
        "n":1,"rev":1,"actor":"agent:x","keyPrefix":"b1","tool":"apply_commands","commands":1,"written":1,
        "atomic":false,"firstSeq":1,"lastSeq":1,"atMs":5,"review":"unreviewed","note":"Hero"}}})));
    let state = serde_json::to_value(app.state()).unwrap();
    assert_eq!(state["activity"]["rows"][0]["note"], "Hero");
    assert_eq!(state["activity"]["unreviewedCount"], 1);
    let back: AppState = from(state);
    assert_eq!(&back, app.state());
}
