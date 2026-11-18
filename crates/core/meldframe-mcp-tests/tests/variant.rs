//! `make_variant`: a copy of a frame and its subtree beside it, one atomic call, safe to retry.

mod common;
use common::*;
use serde_json::{Value, json};

fn roots(tree: &Value) -> Vec<&Value> {
    tree["roots"].as_array().unwrap().iter().collect()
}

async fn hero(c: &Client) {
    let r = call(
        c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "seed", "commands": [
            frame("hero", "Hero"), text("title", "hero", "Hello"), text("sub", "hero", "World")]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn a_variant_copies_the_frame_and_its_children_beside_it() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "v1"}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["nodes"], 3);
    assert_eq!(r.body["replayed"], false);
    assert_eq!(r.body["seq"], 6);
    let root = r.body["root"].as_str().unwrap().to_owned();

    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    let rs = roots(&t.body);
    assert_eq!(rs.len(), 2, "the original and its variant");
    let copy = rs.iter().find(|n| n["id"] == root.as_str()).unwrap();
    assert_eq!(copy["name"], "Hero variant");
    let texts: Vec<&str> = copy["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(texts, ["title", "sub"], "the children came along, in order");
    let x = |n: &Value| n["x"].as_f64().unwrap();
    let orig = rs.iter().find(|n| n["id"] == "hero").unwrap();
    assert!(x(copy) > x(orig), "to the right of the original");
}

#[tokio::test]
async fn a_retry_with_the_same_key_makes_no_second_copy() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let args = json!({"file": "f", "id": "hero", "idempotency_key": "v2"});
    let first = call(&c, "make_variant", args.clone()).await;
    let again = call(&c, "make_variant", args).await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(again.body["replayed"], true);
    assert_eq!(again.body["root"], first.body["root"]);
    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(roots(&t.body).len(), 2);
}

#[tokio::test]
async fn only_a_frame_that_exists_can_be_copied() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "title", "idempotencyKey": "a"}),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "E_NOT_A_FRAME");
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "nope", "idempotencyKey": "b"}),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "E_NODE_NOT_FOUND");
    let r = call(&c, "make_variant", json!({"file": "f", "id": "hero"})).await;
    assert_eq!(r.body["error"]["code"], "invalid", "a key is needed");
    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(roots(&t.body).len(), 1, "nothing was written");
}

#[tokio::test]
async fn a_copy_of_more_than_a_batch_is_refused_whole() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let mut cmds = vec![frame("big", "Big")];
    for i in 0..60 {
        cmds.push(text(&format!("t{i}"), "big", "x"));
    }
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "g", "idempotencyKey": "s1", "commands": cmds}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let mut more = vec![];
    for i in 60..120 {
        more.push(text(&format!("t{i}"), "big", "x"));
    }
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "g", "idempotencyKey": "s2", "commands": more}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let r = call(
        &c,
        "make_variant",
        json!({"file": "g", "id": "big", "idempotencyKey": "v"}),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "invalid");
    let t = call(&c, "get_tree", json!({"file": "g"})).await;
    assert_eq!(roots(&t.body).len(), 1, "nothing was written");
}

#[tokio::test]
async fn a_key_used_for_another_frame_or_by_apply_commands_is_not_a_replay() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let two = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "mk", "commands": [frame("other", "Other")]}),
    )
    .await;
    assert!(!two.is_error, "{}", two.body);
    // `mk` was used by apply_commands: not a replay, and nothing is copied.
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "mk"}),
    )
    .await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "invalid");
    // The same key for another frame after a copy: also refused, not a second copy and not a false replay.
    let ok = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "mv"}),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "other", "idempotencyKey": "mv"}),
    )
    .await;
    assert!(r.is_error, "{}", r.body);
    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(roots(&t.body).len(), 3, "hero, other and one copy of hero");
}

#[tokio::test]
async fn another_actor_with_the_same_key_gets_a_copy_of_its_own() {
    let h = start().await;
    let a = connect(&h, AGENT).await;
    hero(&a).await;
    let p = connect(&h, PERSON).await;
    let one = call(
        &a,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "same"}),
    )
    .await;
    let two = call(
        &p,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "same"}),
    )
    .await;
    assert!(!one.is_error && !two.is_error, "{} {}", one.body, two.body);
    assert_ne!(one.body["root"], two.body["root"]);
    assert_eq!(two.body["replayed"], false);
}
