//! The idempotency key of `make_variant` is worded and bounded as the one of `apply_commands`: one schema, defined
//! in `meldframe-mcp-defs`.

use super::*;
use meldframe_http::mcp::TOOLS;

fn apply_commands_property(name: &str) -> Value {
    let tool = TOOLS.iter().find(|t| t.name == "apply_commands").unwrap();
    tool.input_schema()["properties"][name].clone()
}

#[test]
fn both_spellings_of_the_key_are_the_schema_apply_commands_gives() {
    let schema = input_schema();
    for name in ["idempotencyKey", "idempotency_key"] {
        assert_eq!(
            schema["properties"][name],
            apply_commands_property(name),
            "{name}"
        );
    }
}

#[test]
fn the_key_names_its_other_spelling_and_the_shared_cap() {
    let schema = key_schema("idempotencyKey", "idempotency_key");
    assert_eq!(schema["maxLength"], MAX_KEY);
    let said = schema["description"].as_str().unwrap();
    assert!(
        said.contains("Send idempotencyKey or idempotency_key, not both."),
        "{said}"
    );
    assert!(
        said.contains(&format!("1 to {MAX_KEY} characters")),
        "{said}"
    );
    assert_eq!(input_schema()["properties"]["idempotencyKey"], schema);
}
