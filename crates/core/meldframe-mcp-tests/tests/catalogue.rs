//! The tool text and schemas are listed in `docs/mcp-tools.json`; this compares the file with what the
//! server says to a real client and with the registry.

mod common;
use common::*;
use meldframe_server::Server;
use serde_json::{Value, json};

const CATALOGUE: &str = include_str!("../../../../docs/mcp-tools.json");

#[tokio::test]
async fn the_catalogue_equals_what_a_client_lists() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let listed = c.list_all_tools().await.unwrap();
    let live: Value = serde_json::to_value(
        listed
            .iter()
            .map(|t| {
                json!({"name": t.name, "description": t.description, "inputSchema": t.input_schema})
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    let file_tools: Vec<Value> = file["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["name"] != "capture")
        .map(|t| json!({"name": t["name"], "description": t["description"], "inputSchema": t["inputSchema"]}))
        .collect();
    assert_eq!(
        live,
        Value::Array(file_tools),
        "docs/mcp-tools.json is out of date"
    );
}

#[test]
fn the_catalogue_equals_the_registry_with_tiers() {
    let server = Server::assemble();
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    for t in file["tools"].as_array().unwrap() {
        let name = t["name"].as_str().unwrap();
        let reg = server
            .registry()
            .agent_tool(name)
            .unwrap_or_else(|| panic!("{name} is not in the registry"));
        // The capture tool is served by the capture module; its own test lists it (`meldframe-capture-tests`).
        let module = if name == "capture" { "capture" } else { "mcp" };
        assert_eq!(reg.module, module);
        assert_eq!(t["tier"], reg.tier.as_str(), "{name}");
        assert_eq!(t["description"], reg.description, "{name}");
    }
}

#[tokio::test]
async fn the_read_only_annotation_follows_the_registry_tier() {
    let server = Server::assemble();
    let h = start().await;
    let c = connect(&h, AGENT).await;
    for t in c.list_all_tools().await.unwrap() {
        let reg = server.registry().agent_tool(&t.name).unwrap();
        let read_only = t.annotations.as_ref().and_then(|a| a.read_only_hint);
        let is_read = reg.tier == meldframe_module::RiskTier::Read;
        assert_eq!(read_only, Some(is_read), "{}", t.name);
    }
}

#[test]
fn descriptions_are_english_ascii_and_apply_commands_names_the_batch_limit() {
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    let tools = file["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 6);
    for t in tools {
        let d = t["description"].as_str().unwrap();
        assert!(d.is_ascii() && d.len() > 40, "{}", t["name"]);
    }
    let apply = tools
        .iter()
        .find(|t| t["name"] == "apply_commands")
        .unwrap();
    assert!(
        apply["description"]
            .as_str()
            .unwrap()
            .contains("100 commands")
    );
    assert_eq!(file["batchLimit"], 100);
}

/// Rewrites `docs/mcp-tools.json` from the server: `cargo test -p meldframe-mcp-tests --test catalogue
/// -- --ignored regenerate`. The other tests then check the file against a client and the registry.
#[test]
#[ignore = "writes docs/mcp-tools.json"]
fn regenerate() {
    let server = Server::assemble();
    let tools: Vec<Value> = meldframe_http::mcp::TOOLS
        .iter()
        .map(|t| {
            let tier = server.registry().agent_tool(t.name).unwrap().tier;
            json!({"name": t.name, "tier": tier.as_str(), "description": t.description,
                "inputSchema": t.input_schema()})
        })
        .collect();
    let mut tools = tools;
    let cap = server.registry().agent_tool("capture").unwrap();
    tools.push(
        json!({"name": cap.name, "tier": cap.tier.as_str(), "description": cap.description,
        "inputSchema": meldframe_http_capture::input_schema()}),
    );
    let doc = json!({"batchLimit": meldframe_http::mcp::MAX_BATCH, "tools": tools});
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs/mcp-tools.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}
