//! The rename commands over MCP against the real server (D8): `renameCollection`, `renameMode` and `renameVariable`
//! go through `apply_commands` under the same gate, atomic batch and `ifSeq` as the other variable commands, keep
//! every binding and alias, and carry the stable codes of the add. The tool text, the schema, the catalogue and the
//! skill name them.

// The shared helpers of the MCP tests, `mcp_common`.
#[path = "mcp_common/mod.rs"]
mod common;
use common::*;
use serde_json::{Value, json};

const CATALOGUE: &str = include_str!("../../../docs/mcp-tools.json");
const SKILL: &str = include_str!("../../../skills/meldframe/SKILL.md");
const RENAMES: [&str; 3] = ["renameCollection", "renameMode", "renameVariable"];

async fn apply(c: &Client, key: &str, commands: Vec<Value>) -> Called {
    call(
        c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": key, "commands": commands}),
    )
    .await
}

/// A collection `c1` (light, dark), two colours `v1` and `v2`, and a frame bound to `v1`, with `v2` an alias of it.
async fn seed(c: &Client) {
    let r = apply(
        c,
        "seed",
        vec![
            json!({"type": "addCollection", "collection": {"id": "c1", "name": "appearance",
                "modes": [{"id": "light", "name": "Light"}, {"id": "dark", "name": "Dark"}]}}),
            json!({"type": "addVariable", "variable": {"id": "v1", "collection": "c1", "name": "color/brand",
                "type": "color", "values": {"light": "#112233", "dark": "#aabbcc"}}}),
            json!({"type": "addVariable", "variable": {"id": "v2", "collection": "c1", "name": "color/accent",
                "type": "color", "values": {"light": {"var": "v1"}, "dark": "#ff0000"}}}),
            json!({"type": "addNode", "node": {"id": "card", "type": "frame", "name": "Card", "w": 100, "h": 50,
                "fill": {"var": "v1"}}}),
        ],
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn the_rename_commands_are_in_the_text_the_schema_the_catalogue_and_the_skill() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let apply = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    let text = apply.description.as_deref().unwrap();
    let schema = serde_json::to_string(&*apply.input_schema).unwrap();
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    let listed = file["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "apply_commands")
        .unwrap();
    let catalogue = serde_json::to_string(listed).unwrap();
    for t in RENAMES {
        assert!(text.contains(t), "{t} is in the description");
        assert!(schema.contains(&format!("\"{t}\"")), "{t} is in the schema");
        assert!(catalogue.contains(t), "{t} is in the catalogue");
        assert!(SKILL.contains(&format!("`{t}`")), "{t} is in the skill");
    }
}

#[tokio::test]
async fn a_rename_through_apply_commands_keeps_the_binding_and_the_alias() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let r = apply(
        &c,
        "rename",
        vec![
            json!({"type": "renameVariable", "id": "v1", "name": "color/primary"}),
            json!({"type": "renameMode", "collection": "c1", "mode": "dark", "name": "Night"}),
            json!({"type": "renameCollection", "id": "c1", "name": "theme"}),
        ],
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let v = call(&c, "get_variables", json!({"file": "f"})).await;
    assert_eq!(v.body["collections"][0]["name"], "theme");
    assert_eq!(
        v.body["collections"][0]["modes"],
        json!([{"id": "light", "name": "Light"}, {"id": "dark", "name": "Night"}])
    );
    assert_eq!(v.body["variables"][0]["id"], "v1");
    assert_eq!(v.body["variables"][0]["name"], "color/primary");
    // The alias and the binding hold the id: they did not move.
    assert_eq!(
        v.body["variables"][1]["values"]["light"],
        json!({"var": "v1"})
    );
    let n = call(&c, "get_nodes", json!({"file": "f", "ids": ["card"]})).await;
    assert_eq!(n.body["nodes"][0]["fill"], json!({"var": "v1"}));
    // The old name is free again, and `prefix` finds the new one.
    let by = call(
        &c,
        "get_variables",
        json!({"file": "f", "prefix": "color/pri"}),
    )
    .await;
    assert_eq!(by.body["total"], 1);
    let r = apply(
        &c,
        "reuse",
        vec![json!({"type": "renameVariable", "id": "v2", "name": "color/brand"})],
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn refused_renames_carry_their_stable_codes_and_write_nothing() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let cases: Vec<(Value, &str)> = vec![
        (
            json!({"type": "renameVariable", "id": "ghost", "name": "color/x"}),
            "E_VARIABLE_NOT_FOUND",
        ),
        (
            json!({"type": "renameVariable", "id": "v1", "name": "Color/Brand"}),
            "E_INVALID_NAME",
        ),
        (
            json!({"type": "renameVariable", "id": "v1", "name": "color/accent"}),
            "E_DUPLICATE_NAME",
        ),
        (
            json!({"type": "renameMode", "collection": "c1", "mode": "ghost", "name": "x"}),
            "E_MODE_NOT_FOUND",
        ),
        (
            json!({"type": "renameMode", "collection": "ghost", "mode": "dark", "name": "x"}),
            "E_COLLECTION_NOT_FOUND",
        ),
        (
            json!({"type": "renameCollection", "id": "c1", "name": "Not A Name"}),
            "E_INVALID_NAME",
        ),
        (
            json!({"type": "renameCollection", "id": "c1", "name": "a".repeat(300)}),
            "E_TOO_LONG",
        ),
    ];
    for (i, (command, code)) in cases.into_iter().enumerate() {
        let r = apply(&c, &format!("bad{i}"), vec![command]).await;
        assert!(r.is_error, "{code}: {}", r.body);
        assert_eq!(r.body["error"]["code"], code, "{}", r.body);
    }
    // The call is atomic: a good rename before a bad one is not written.
    let r = apply(
        &c,
        "mixed",
        vec![
            json!({"type": "renameVariable", "id": "v1", "name": "color/ok"}),
            json!({"type": "renameVariable", "id": "v2", "name": "color/ok"}),
        ],
    )
    .await;
    assert_eq!(r.body["error"]["code"], "E_DUPLICATE_NAME");
    assert_eq!(r.body["error"]["index"], 1);
    let v = call(&c, "get_variables", json!({"file": "f"})).await;
    assert_eq!(v.body["variables"][0]["name"], "color/brand");
    // A rename to the name the variable has is not an error.
    let r = apply(
        &c,
        "same",
        vec![json!({"type": "renameVariable", "id": "v1", "name": "color/brand"})],
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn a_rename_takes_part_in_if_seq() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let seq = call(&c, "get_variables", json!({"file": "f"})).await.body["seq"]
        .as_u64()
        .unwrap();
    let other = connect(&h, PERSON).await;
    let theirs = apply(
        &other,
        "theirs",
        vec![
            json!({"type": "renameVariable", "id": "v1", "name": "color/theirs"}),
            json!({"type": "renameMode", "collection": "c1", "mode": "dark", "name": "Dim"}),
        ],
    )
    .await;
    assert!(!theirs.is_error, "{}", theirs.body);
    for (key, command, node) in [
        (
            "mine1",
            json!({"type": "renameVariable", "id": "v1", "name": "color/mine"}),
            "$var:v1",
        ),
        (
            "mine2",
            json!({"type": "renameCollection", "id": "c1", "name": "mine"}),
            "$col:c1",
        ),
    ] {
        let r = call(
            &c,
            "apply_commands",
            json!({"file": "f", "idempotencyKey": key, "ifSeq": seq, "commands": [command]}),
        )
        .await;
        assert_eq!(r.body["error"]["code"], "conflict", "{}", r.body);
        assert_eq!(r.body["error"]["nodeId"], node);
    }
}
