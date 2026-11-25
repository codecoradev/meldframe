//! A call sent again writes nothing twice: after a timeout, after a failure part way, and under the same key with the
//! same text (`docs/import-html.md` section 4.3). The same key with other text is another call.

mod common;
mod harness;
use harness::*;
use serde_json::json;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn the_same_call_again_writes_nothing_and_says_replayed() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let first = import(&c, "k1", HERO, json!({"note": "once"})).await;
    let (before, submits) = (
        head(&c).await,
        w.h.spy.atomic_submits.load(Ordering::SeqCst),
    );
    let again = import(&c, "k1", HERO, json!({"note": "twice"})).await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(again.body["replayed"], json!(true));
    assert_eq!(
        (again.body["root"].clone(), ids(&again)),
        (first.body["root"].clone(), ids(&first))
    );
    assert_eq!(again.body["seq"], first.body["seq"]);
    assert_eq!(again.body["batch"]["n"], json!(1), "the same batch");
    assert_eq!(
        again.body["batch"]["note"],
        json!("kept_earlier"),
        "the first note stays"
    );
    assert_eq!(head(&c).await, before);
    assert_eq!(
        w.h.spy.atomic_submits.load(Ordering::SeqCst),
        submits,
        "nothing was sent to the room"
    );
}

#[tokio::test]
async fn a_retry_after_the_person_changed_the_root_still_writes_nothing_and_keeps_their_change() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let first = import(&c, "k1", HERO, json!({})).await;
    let root = first.body["root"].as_str().unwrap().to_owned();
    let p = connect(&w.h, PERSON).await;
    let moved = json!({"type": "updateNode", "id": root, "patch": {"x": 777, "name": "Renamed"}});
    assert!(
        !call(
            &p,
            "apply_commands",
            json!({"file": "f", "idempotencyKey": "mv", "commands": [moved]})
        )
        .await
        .is_error
    );
    let again = import(&c, "k1", HERO, json!({})).await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(again.body["replayed"], json!(true));
    let now = nodes(&c, &[root.as_str()]).await.remove(0);
    assert_eq!(
        (now["x"].clone(), now["name"].clone()),
        (json!(777.0), json!("Renamed"))
    );
}

#[tokio::test]
async fn the_same_key_with_other_text_is_another_call_and_two_actors_may_share_a_key() {
    let w = start().await;
    let (a, p) = (connect(&w.h, AGENT).await, connect(&w.h, PERSON).await);
    let one = import(&a, "k1", HERO, json!({})).await;
    let other = import(&a, "k1", &HERO.replace("Hello", "Bye"), json!({})).await;
    assert!(!other.is_error, "{}", other.body);
    assert_eq!(other.body["replayed"], json!(false));
    assert_ne!(other.body["root"], one.body["root"]);
    let beside = import(&a, "k1", HERO, json!({"parentId": one.body["root"]})).await;
    assert!(
        !beside.is_error && beside.body["replayed"] == json!(false),
        "{}",
        beside.body
    );
    // The person sends the agent's key and text into the same file: other ids, no `E_DUPLICATE_ID`.
    let theirs = import(&p, "k1", HERO, json!({})).await;
    assert!(!theirs.is_error, "{}", theirs.body);
    assert_ne!(theirs.body["root"], one.body["root"]);
    assert_eq!(
        head(&a).await.1,
        3,
        "three top-level frames and one inside the first"
    );
}

#[tokio::test]
async fn a_retry_after_a_timeout_finds_what_the_first_call_wrote() {
    let w = start_with(Setup {
        timeout_ms: Some(150),
        ..Setup::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    w.faulty.slow.store(true, Ordering::SeqCst);
    let r = import(&c, "k1", HERO, json!({})).await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(
        (
            r.body["error"]["code"].clone(),
            r.body["error"]["retryable"].clone()
        ),
        (json!("timeout"), json!(true))
    );
    w.faulty.ended_after(0).await;
    w.faulty.slow.store(false, Ordering::SeqCst);
    let again = import(&c, "k1", HERO, json!({})).await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(again.body["replayed"], json!(true));
    assert_eq!(head(&c).await.1, 1, "one import, not two");
    assert_eq!(again.body["batch"]["n"], json!(1));
}

#[tokio::test]
async fn a_retry_after_a_store_failed_part_way_writes_the_rest_where_the_root_already_is() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    apply(&c, "seed", vec![json!({"type": "addNode", "node": {"id": "a", "type": "frame", "name": "A", "w": 800, "h": 400}})]).await;
    w.faulty.cut.store(2, Ordering::SeqCst);
    let failed = import(&c, "k1", HERO, json!({})).await;
    assert!(failed.is_error, "{}", failed.body);
    assert_eq!(failed.body["error"]["code"], json!("unavailable"));
    // The first two commands are in the file; then something else is added to the right of them.
    w.faulty.cut.store(0, Ordering::SeqCst);
    let far = json!({"type": "addNode", "node": {"id": "far", "type": "frame", "name": "Far", "x": 5000, "w": 100, "h": 100}});
    apply(&c, "far", vec![far]).await;
    let retry = import(&c, "k1", HERO, json!({})).await;
    assert!(!retry.is_error, "{}", retry.body);
    assert_eq!(
        retry.body["replayed"],
        json!(false),
        "the third command is new"
    );
    let all = ids(&retry);
    let got = nodes(&c, &all.iter().map(String::as_str).collect::<Vec<_>>()).await;
    assert_eq!(got.len(), 3);
    let root = got.iter().find(|n| n["id"] == retry.body["root"]).unwrap();
    assert_eq!(
        root["x"],
        json!(840.0),
        "the root kept the place the first try gave it, not 40 px past `far`"
    );
    assert_eq!(head(&c).await.1, 3, "the frame a, far and one import");
}
