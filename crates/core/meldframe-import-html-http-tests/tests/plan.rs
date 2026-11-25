//! The plan of a call: the ids, the digest, the limits that are the tool's (100 Commands, `strict`) and the errors of the
//! parse (`docs/import-html.md` sections 2.3 and 4).

mod common;
use common::*;
use meldframe_protocol::Command;
use serde_json::json;

fn root_of(p: &meldframe_http_html_import::Planned) -> &meldframe_protocol::Node {
    match &p.plan.commands[0] {
        Command::AddNode { node, .. } => node,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_ids_are_fresh_short_and_in_the_order_of_the_text() {
    let p = planned(&boxes(3), json!({})).unwrap();
    assert_eq!(p.plan.commands.len(), 5);
    let ids: Vec<&str> = p.plan.nodes.iter().map(|n| n.id.as_str()).collect();
    let prefix = ids[0].strip_suffix("-0").unwrap();
    assert!(
        prefix.starts_with("imp-") && prefix.len() == 4 + 16,
        "{prefix}"
    );
    assert!(prefix[4..].chars().all(|c| c.is_ascii_hexdigit()));
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, format!("{prefix}-{i}"));
        assert!(id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }
    assert_eq!(p.root, ids[0]);
}

#[test]
fn the_same_call_gives_the_same_ids_and_another_key_other_ids() {
    let ids = |extra| {
        let p = planned("<div>hello</div>", extra).unwrap();
        (p.root.clone(), p.prefix.clone(), p.digest.clone())
    };
    let first = ids(json!({}));
    assert_eq!(first, ids(json!({})));
    let other = ids(json!({"idempotencyKey": "k2"}));
    assert_ne!(first.0, other.0, "another key, other ids");
    assert_eq!(first.2, other.2, "the digest does not depend on the key");
}

#[test]
fn the_digest_and_the_ids_follow_everything_that_shapes_the_commands() {
    let base = planned("<div>hello</div>", json!({})).unwrap();
    assert_eq!(base.prefix, format!("k1:{}", base.digest));
    assert!(base.digest.len() == 12 && base.digest.chars().all(|c| c.is_ascii_hexdigit()));
    for extra in [
        json!({"css": "div{color:red}"}),
        json!({"name": "Other"}),
        json!({"width": 800}),
        json!({"parentId": "frame"}),
    ] {
        let p = planned("<div>hello</div>", extra.clone()).unwrap();
        assert_ne!(p.digest, base.digest, "{extra}");
        assert_ne!(p.root, base.root, "{extra}");
    }
    let other = planned("<div>hello!</div>", json!({})).unwrap();
    assert_ne!(other.digest, base.digest);
    // What does not shape the Commands does not change the call.
    let same = planned(
        "<div>hello</div>",
        json!({"dryRun": true, "strict": false, "note": "n"}),
    )
    .unwrap();
    assert_eq!(
        (same.digest, same.root),
        (base.digest.clone(), base.root.clone())
    );
}

#[test]
fn another_actor_or_file_gets_other_ids_so_two_actors_can_send_one_key() {
    let a = meldframe_http_html_import::Args::parse(args_of("<div>x</div>", json!({}))).unwrap();
    let b = meldframe_http_html_import::Args::parse(args_of("<div>x</div>", json!({"file": "g"})))
        .unwrap();
    let id = |a: &meldframe_http_html_import::Args, who| {
        meldframe_http_html_import::make(a, who).unwrap().root
    };
    assert_eq!(id(&a, "agent:alpha"), id(&a, "agent:alpha"));
    assert_ne!(id(&a, "agent:alpha"), id(&a, "agent:beta"));
    assert_ne!(id(&a, "agent:alpha"), id(&b, "agent:alpha"));
}

#[test]
fn the_root_is_as_wide_as_the_call_says_when_the_page_does_not() {
    let w = |extra| root_of(&planned("<div>hello</div>", extra).unwrap()).w;
    assert_eq!(w(json!({})), 1440.0);
    assert_eq!(w(json!({"width": 800})), 800.0);
    let page = "<body style='width:300px'><div>hello</div></body>";
    assert_eq!(
        root_of(&planned(page, json!({"width": 800})).unwrap()).w,
        300.0
    );
}

#[test]
fn the_name_is_the_calls_then_the_title_then_imported_html() {
    let name = |html: &str, extra| root_of(&planned(html, extra).unwrap()).name.clone();
    let titled = "<html><head><title>My page</title></head><body><p>x</p><p>y</p></body></html>";
    assert_eq!(name(titled, json!({})), "My page");
    assert_eq!(name(titled, json!({"name": "Hero"})), "Hero");
    assert_eq!(name("<p>x</p><p>y</p>", json!({})), "Imported HTML");
}

#[test]
fn the_plan_has_no_parent_and_no_position_it_is_the_servers_to_set() {
    let p = planned(&boxes(2), json!({})).unwrap();
    assert_eq!(root_of(&p).parent_id, None);
    for c in &p.plan.commands {
        let Command::AddNode { node, index } = c else {
            panic!("{c:?}")
        };
        assert!(node.pos.is_empty() && index.is_none());
    }
}

#[test]
fn a_hundred_commands_are_written_and_a_hundred_and_one_are_refused() {
    let p = planned(&boxes(98), json!({})).unwrap();
    assert_eq!(p.plan.commands.len(), 100);
    let e = planned(&boxes(99), json!({})).unwrap_err();
    assert_eq!(code(&e), "E_IMPORT_HTML_TOO_MANY");
    let m = message(&e);
    assert!(
        m.contains("101 nodes") && m.contains("100") && m.contains("one section at a time"),
        "{m}"
    );
    assert!(m.contains("parentId"), "{m}");
}

#[test]
fn more_than_five_thousand_elements_are_refused_before_the_limit_of_commands() {
    let html = "<i></i>".repeat(5_001);
    let e = planned(&html, json!({})).unwrap_err();
    assert_eq!(code(&e), "E_IMPORT_HTML_TOO_MANY_ELEMENTS");
    assert!(message(&e).contains("one section at a time"), "{e}");
    // At the limit it is read (it may still make too many nodes, but not too many elements).
    if let Err(e) = planned(&"<i>a</i>".repeat(5_000), json!({})) {
        assert_ne!(code(&e), "E_IMPORT_HTML_TOO_MANY_ELEMENTS", "{e}");
    }
}

#[test]
fn text_that_makes_no_node_is_empty() {
    for html in [
        "",
        "just text",
        "<!-- c -->",
        "<script>1</script>",
        "<style>p{}</style>",
        "<div style='display:none'>x</div>",
        "<div hidden>x</div>",
    ] {
        let e = planned(html, json!({})).unwrap_err();
        assert_eq!(code(&e), "E_IMPORT_HTML_EMPTY", "{html:?}");
    }
}

#[test]
fn strict_fails_on_a_refused_property_and_lists_it() {
    let html = "<div><p style='line-height:2'>x</p></div>";
    assert!(planned(html, json!({})).is_ok());
    let e = planned(html, json!({"strict": true})).unwrap_err();
    assert_eq!(code(&e), "E_IMPORT_HTML_UNSUPPORTED");
    let refused = e["error"]["refused"].as_array().unwrap();
    assert!(
        refused.iter().any(|r| r["name"] == "line-height"
            && r["kind"] == "refused"
            && r["code"] == "refused_not_modelled"),
        "{e}"
    );
    assert_eq!(e["error"]["omitted"], json!(0));
}

#[test]
fn strict_fails_on_a_refused_selector_and_a_control_that_has_no_node() {
    for (html, css) in [
        (
            "<div>x</div>",
            "@media (min-width: 1px) { div { color: red } }",
        ),
        ("<div>x</div>", "div:hover { color: red }"),
        ("<div>x<input type=checkbox></div>", ""),
    ] {
        assert!(planned(html, json!({"css": css})).is_ok(), "{css}");
        let e = planned(html, json!({"css": css, "strict": true})).unwrap_err();
        assert_eq!(code(&e), "E_IMPORT_HTML_UNSUPPORTED", "{html} {css}");
    }
}

#[test]
fn strict_does_not_count_what_is_only_listed() {
    // A class that no rule names is listed for the agent: it is not a property that was lost.
    let html = "<div class='hero wide'><p>x</p></div>";
    let p = planned(html, json!({"strict": true})).unwrap();
    let listed = p
        .plan
        .report
        .entries
        .iter()
        .filter(|e| e.kind.as_str() == "listed")
        .count();
    assert!(listed >= 2, "{listed}");
    assert!(planned("<div><p>plain</p></div>", json!({"strict": true})).is_ok());
}

#[test]
fn strict_fails_closed_when_the_report_dropped_entries() {
    // 250 classes that no rule names are 250 listed entries; the report keeps 200, and a refusal after them would be
    // one of the dropped. With strict that cannot be told, so the call fails and says how many entries are missing.
    let classes: Vec<String> = (0..250).map(|i| format!("c{i}")).collect();
    let html = format!(
        "<div class='{}'><p style='line-height:2'>x</p></div>",
        classes.join(" ")
    );
    assert!(planned(&html, json!({})).is_ok());
    let e = planned(&html, json!({"strict": true})).unwrap_err();
    assert_eq!(code(&e), "E_IMPORT_HTML_UNSUPPORTED");
    assert!(e["error"]["omitted"].as_u64().unwrap() > 0, "{e}");
    assert!(e["error"]["refused"].as_array().unwrap().is_empty(), "{e}");
}
