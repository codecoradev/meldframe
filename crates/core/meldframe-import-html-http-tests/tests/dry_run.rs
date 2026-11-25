//! `dryRun` writes nothing and answers as the write will, after the commands are checked against the file with the checks
//! of every write: an agent reads what would be refused before it writes (`docs/import-html.md` section 4.1).

mod common;
mod harness;
use harness::*;
use serde_json::{Value, json};

/// The answer of a call without what only a write has.
fn plan_part(r: &Called) -> Value {
    let mut b = r.body.clone();
    for k in ["seq", "batch", "dryRun", "replayed"] {
        b.as_object_mut().unwrap().remove(k);
    }
    b
}

#[tokio::test]
async fn a_dry_run_writes_nothing_and_the_real_call_answers_the_same() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let seed = json!({"type": "addNode", "node": {"id": "a", "type": "frame", "name": "A", "w": 10, "h": 10}});
    apply(&c, "seed", vec![seed]).await;
    let before = head(&c).await;
    assert!(before.0 > 0);
    let dry = import(&c, "k1", HERO, json!({"dryRun": true})).await;
    assert!(!dry.is_error, "{}", dry.body);
    assert_eq!(
        (
            dry.body["dryRun"].clone(),
            dry.body["replayed"].clone(),
            dry.body["batch"].clone()
        ),
        (json!(true), json!(false), Value::Null)
    );
    assert_eq!(
        dry.body["seq"],
        json!(before.0),
        "the sequence number of the file, which did not move"
    );
    assert_eq!(head(&c).await, before, "nothing was written");
    assert_eq!(
        call(&c, "get_nodes", json!({"file": "f", "ids": ids(&dry)}))
            .await
            .body["nodes"],
        json!([])
    );
    let page = w
        .store
        .list(
            "ws_1",
            "f",
            &Query {
                before: None,
                after: None,
                limit: 10,
            },
        )
        .unwrap();
    assert_eq!(
        page.headers.len(),
        1,
        "a dry run keeps no batch: only the seed has one"
    );

    let real = import(&c, "k1", HERO, json!({})).await;
    assert!(!real.is_error, "{}", real.body);
    assert_eq!(
        plan_part(&dry),
        plan_part(&real),
        "the same ids, nodes, warnings and report"
    );
    assert_eq!(
        real.body["batch"],
        json!({"n": 2, "note": "none"}),
        "and the batch number was not spent: the seed is batch 1"
    );
}

#[tokio::test]
async fn a_dry_run_is_refused_for_what_the_write_is_refused_for_with_the_same_error() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let chain: Vec<Value> = (0..32)
        .map(|i| {
            let mut n = json!({"id": format!("d{i}"), "type": "frame", "name": format!("Level {i}"), "w": 100, "h": 100});
            if i > 0 {
                n["parentId"] = json!(format!("d{}", i - 1));
            }
            json!({"type": "addNode", "node": n})
        })
        .collect();
    apply(&c, "chain", chain).await;
    let deep = "<body style='margin:0'><div data-name='a' style='background:#eee;padding:4px'>\
        <div data-name='b' style='background:#ddd;padding:4px'><div data-name='c' style='background:#ccc;padding:4px'>x</div></div></div></body>";
    for (html, parent, code, index) in [
        (deep, "d31", "E_TOO_DEEP", 1),
        (HERO, "nope", "E_PARENT_NOT_FOUND", 0),
        (HERO, "d0", "ok", 0),
    ] {
        let (dry, real) = (
            import(&c, "k1", html, json!({"parentId": parent, "dryRun": true})).await,
            import(
                &c,
                &format!("real-{parent}"),
                html,
                json!({"parentId": parent}),
            )
            .await,
        );
        if code == "ok" {
            assert!(
                !dry.is_error && !real.is_error,
                "{} {}",
                dry.body,
                real.body
            );
            continue;
        }
        assert!(dry.is_error && real.is_error, "{code}");
        for r in [&dry, &real] {
            let e = &r.body["error"];
            assert_eq!(
                (e["code"].clone(), e["index"].clone()),
                (json!(code), json!(index)),
                "{e}"
            );
            assert!(
                e["source"].as_str().unwrap().starts_with(if index == 1 {
                    "div"
                } else {
                    "section"
                }),
                "{e}"
            );
        }
        assert_eq!(dry.body["error"]["message"], real.body["error"]["message"]);
    }
}

#[tokio::test]
async fn a_dry_run_has_the_other_checks_of_a_call_and_costs_no_quota() {
    let w = start_with(Setup {
        quota: Some(4),
        ..Setup::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    for bad in [
        json!({"note": "a\u{200b}b"}),
        json!({"width": -1}),
        json!({"strict": "yes"}),
    ] {
        let mut extra = bad.clone();
        extra["dryRun"] = json!(true);
        assert!(import(&c, "k1", HERO, extra).await.is_error, "{bad}");
    }
    let strict = import(
        &c,
        "k1",
        "<div><p style='line-height:2'>x</p></div>",
        json!({"dryRun": true, "strict": true}),
    )
    .await;
    assert_eq!(
        strict.body["error"]["code"],
        json!("E_IMPORT_HTML_UNSUPPORTED")
    );
    // A note on a dry run is checked and not kept; ten dry runs spend no op of the quota of 4 a minute.
    for _ in 0..10 {
        let r = import(&c, "k1", HERO, json!({"dryRun": true, "note": "n"})).await;
        assert!(!r.is_error, "{}", r.body);
    }
    assert!(!import(&c, "k1", HERO, json!({})).await.is_error);
}
