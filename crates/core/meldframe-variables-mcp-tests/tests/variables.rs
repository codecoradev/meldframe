//! The variables over MCP against the real server (E3.4): the seven first variable commands (the renames are in `rename.rs`) go through `apply_commands`
//! under the same gate, atomic batch and `ifSeq` as node commands, a node field is bound to `{var: id}`, and
//! `get_variables` reads collections and variables in bounded output.

// The helpers of `meldframe-mcp-tests`, by path: that crate is at its size ceiling.
#[path = "../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
use common::*;
use serde_json::{Value, json};

const CATALOGUE: &str = include_str!("../../../../docs/mcp-tools.json");

fn appearance() -> Value {
    json!({"type": "addCollection", "collection": {"id": "c1", "name": "appearance",
        "modes": [{"id": "light", "name": "Light"}, {"id": "dark", "name": "Dark"}]}})
}

fn colour(id: &str, name: &str, light: &str, dark: &str) -> Value {
    json!({"type": "addVariable", "variable": {"id": id, "collection": "c1", "name": name,
        "type": "color", "values": {"light": light, "dark": dark}}})
}

async fn apply(c: &Client, key: &str, commands: Vec<Value>) -> Called {
    call(
        c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": key, "commands": commands}),
    )
    .await
}

async fn seed(c: &Client) {
    let r = apply(
        c,
        "seed",
        vec![appearance(), colour("v1", "color/brand", "#112233", "#aabbcc"),
            json!({"type": "addVariable", "variable": {"id": "v2", "collection": "c1", "name": "radius/card",
                "type": "number", "values": {"light": 8, "dark": 12}}})],
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn the_seven_variable_commands_go_through_apply_commands_and_can_be_read() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let r = apply(
        &c,
        "more",
        vec![
            json!({"type": "addMode", "collection": "c1", "mode": {"id": "hc", "name": "High contrast"}, "copyFrom": "dark"}),
            json!({"type": "setVariableValue", "id": "v1", "mode": "hc", "value": "#000000"}),
            json!({"type": "addVariable", "variable": {"id": "v3", "collection": "c1", "name": "color/accent",
                "type": "color", "values": {"light": {"var": "v1"}, "dark": "#ff0000", "hc": "#ffffff"}}}),
            json!({"type": "removeVariable", "id": "v3"}),
            json!({"type": "removeMode", "collection": "c1", "mode": "hc"}),
            json!({"type": "addCollection", "collection": {"id": "c2", "name": "scratch", "modes": [{"id": "m", "name": "M"}]}}),
            json!({"type": "removeCollection", "id": "c2"}),
        ],
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let v = call(&c, "get_variables", json!({"file": "f"})).await;
    assert!(!v.is_error, "{}", v.body);
    assert_eq!(v.body["total"], 2);
    assert_eq!(v.body["next"], Value::Null);
    let cols = v.body["collections"].as_array().unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0]["id"], "c1");
    assert_eq!(cols[0]["variables"], 2);
    assert_eq!(
        cols[0]["modes"],
        json!([{"id": "light", "name": "Light"}, {"id": "dark", "name": "Dark"}])
    );
    let vars = v.body["variables"].as_array().unwrap();
    assert_eq!(vars[0]["id"], "v1");
    assert_eq!(vars[0]["type"], "color");
    assert_eq!(
        vars[0]["values"],
        json!({"light": "#112233", "dark": "#aabbcc"})
    );
    assert_eq!(vars[1]["values"], json!({"light": 8.0, "dark": 12.0}));
}

#[tokio::test]
async fn a_node_is_bound_to_a_variable_and_an_alias_shows_as_a_var() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let r = apply(
        &c,
        "bind",
        vec![
            json!({"type": "addVariable", "variable": {"id": "v3", "collection": "c1", "name": "color/alias",
                "type": "color", "values": {"light": {"var": "v1"}, "dark": {"var": "v1"}}}}),
            json!({"type": "addNode", "node": {"id": "card", "type": "frame", "name": "Card", "w": 100, "h": 50,
                "fill": {"var": "v1"}, "radius": {"var": "v2"}}}),
            json!({"type": "updateNode", "id": "card", "patch": {"opacity": 0.5}}),
        ],
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f", "ids": ["card"]})).await;
    assert_eq!(n.body["nodes"][0]["fill"], json!({"var": "v1"}));
    assert_eq!(n.body["nodes"][0]["radius"], json!({"var": "v2"}));
    let v = call(
        &c,
        "get_variables",
        json!({"file": "f", "prefix": "color/alias"}),
    )
    .await;
    assert_eq!(
        v.body["variables"][0]["values"]["light"],
        json!({"var": "v1"})
    );
}

#[tokio::test]
async fn refused_variable_commands_carry_their_stable_codes_and_write_nothing() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let node = |fill: Value| json!({"type": "addNode", "node": {"id": "n", "type": "rect", "name": "R", "fill": fill}});
    let cases: Vec<(Value, &str)> = vec![
        (node(json!({"var": "nope"})), "E_VARIABLE_NOT_FOUND"),
        (node(json!({"var": "v2"})), "E_TYPE_MISMATCH"),
        (
            json!({"type": "removeVariable", "id": "ghost"}),
            "E_VARIABLE_NOT_FOUND",
        ),
        (
            json!({"type": "setVariableValue", "id": "v1", "mode": "x", "value": "#fff"}),
            "E_MODE_NOT_FOUND",
        ),
        (
            json!({"type": "setVariableValue", "id": "v1", "mode": "light", "value": {"var": "v1"}}),
            "E_ALIAS_CYCLE",
        ),
        (colour("v9", "Color/Bad", "#111", "#222"), "E_INVALID_NAME"),
        (
            json!({"type": "removeCollection", "id": "c1"}),
            "E_COLLECTION_NOT_EMPTY",
        ),
    ];
    for (i, (command, code)) in cases.into_iter().enumerate() {
        let r = apply(&c, &format!("bad{i}"), vec![command]).await;
        assert!(r.is_error, "{code}: {}", r.body);
        assert_eq!(r.body["error"]["code"], code, "{}", r.body);
    }
    let r = apply(
        &c,
        "mixed",
        vec![
            colour("v4", "color/ok", "#111", "#222"),
            node(json!({"var": "nope"})),
        ],
    )
    .await;
    assert_eq!(r.body["error"]["index"], 1);
    let v = call(&c, "get_variables", json!({"file": "f"})).await;
    assert_eq!(
        v.body["total"], 2,
        "the batch was atomic: color/ok was not written"
    );
}

#[tokio::test]
async fn a_variable_takes_part_in_if_seq() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let seq = call(&c, "get_variables", json!({"file": "f"})).await.body["seq"]
        .as_u64()
        .unwrap();
    let other = connect(&h, PERSON).await;
    let r = call(
        &other,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "theirs", "commands": [
            {"type": "setVariableValue", "id": "v1", "mode": "light", "value": "#abcdef"}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let mine = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "mine", "ifSeq": seq, "commands": [
            {"type": "setVariableValue", "id": "v1", "mode": "light", "value": "#000000"}]}),
    )
    .await;
    assert!(mine.is_error, "{}", mine.body);
    assert_eq!(mine.body["error"]["code"], "conflict");
    assert_eq!(mine.body["error"]["nodeId"], "$var:v1");
}

#[tokio::test]
async fn get_variables_pages_filters_and_is_bounded() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let mut cmds = vec![appearance()];
    for i in 0..30 {
        cmds.push(colour(
            &format!("v{i}"),
            &format!("{}/n{i}", if i % 2 == 0 { "color" } else { "bg" }),
            "#111",
            "#222",
        ));
    }
    let r = apply(&c, "many", cmds).await;
    assert!(!r.is_error, "{}", r.body);
    let first = call(&c, "get_variables", json!({"file": "f", "limit": 7})).await;
    assert_eq!(first.body["variables"].as_array().unwrap().len(), 7);
    assert_eq!(
        (first.body["total"].clone(), first.body["next"].clone()),
        (json!(30), json!("v6"))
    );
    let mut seen = 7;
    let mut after = first.body["next"].as_str().unwrap().to_owned();
    loop {
        let p = call(
            &c,
            "get_variables",
            json!({"file": "f", "limit": 7, "after": after}),
        )
        .await;
        seen += p.body["variables"].as_array().unwrap().len();
        match p.body["next"].as_str() {
            Some(n) => after = n.to_owned(),
            None => break,
        }
    }
    assert_eq!(seen, 30);
    let colours = call(
        &c,
        "get_variables",
        json!({"file": "f", "prefix": "color/", "limit": 200}),
    )
    .await;
    assert_eq!(colours.body["total"], 15);
    let by = call(
        &c,
        "get_variables",
        json!({"file": "f", "collection": "c1", "limit": 1}),
    )
    .await;
    assert_eq!(by.body["total"], 30);
    for bad in [
        json!({"limit": 0}),
        json!({"limit": 201}),
        json!({"after": "nope"}),
        json!({"prefix": "x".repeat(257)}),
    ] {
        let mut a = bad.as_object().unwrap().clone();
        a.insert("file".into(), json!("f"));
        let r = call(&c, "get_variables", Value::Object(a)).await;
        assert!(r.is_error, "{bad}");
        assert!(
            ["invalid", "bad_request"].contains(&r.body["error"]["code"].as_str().unwrap()),
            "{}",
            r.body
        );
    }
    let r = call(
        &c,
        "get_variables",
        json!({"file": "f", "collection": "nope"}),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "E_COLLECTION_NOT_FOUND");
    let r = call(&c, "get_variables", json!({"file": "f", "extra": 1})).await;
    assert_eq!(r.body["error"]["code"], "bad_request");
}

#[tokio::test]
async fn a_file_without_variables_and_another_workspace_read_as_empty() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let empty = call(&c, "get_variables", json!({"file": "never-written"})).await;
    assert_eq!(
        (empty.body["seq"].clone(), empty.body["total"].clone()),
        (json!(0), json!(0))
    );
    assert_eq!(empty.body["collections"], json!([]));
    let other = connect(&h, OTHER).await;
    let theirs = call(&other, "get_variables", json!({"file": "f"})).await;
    assert_eq!(theirs.body["total"], 0, "the token decides the workspace");
    assert_eq!(theirs.body["collections"], json!([]));
}

#[tokio::test]
async fn the_variable_commands_are_in_the_text_and_the_schema_of_apply_commands() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let apply = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    let text = apply.description.as_deref().unwrap();
    let schema = serde_json::to_string(&*apply.input_schema).unwrap();
    for t in [
        "addCollection",
        "removeCollection",
        "addMode",
        "removeMode",
        "addVariable",
        "removeVariable",
        "setVariableValue",
    ] {
        assert!(text.contains(t), "{t} is in the description");
        assert!(schema.contains(&format!("\"{t}\"")), "{t} is in the schema");
    }
    assert!(text.contains("get_variables") && text.contains("$var:"));
    assert!(tools.iter().any(|t| t.name == "get_variables"));
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    assert!(
        file["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "get_variables")
    );
}
