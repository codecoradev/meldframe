//! Rewrites the catalogue of the tools, `docs/mcp-tools.json`, from the server.

use meldframe_server::Server;
use serde_json::{Value, json};

/// Rewrites `docs/mcp-tools.json` from the server: `cargo test -p meldframe-review-tests --test regenerate
/// -- --ignored regenerate`. The tests of `meldframe-mcp-tests` then check the file against a client and the registry.
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
    let up = server.registry().agent_tool("upload_asset").unwrap();
    tools.push(
        json!({"name": up.name, "tier": up.tier.as_str(), "description": up.description,
        "inputSchema": meldframe_http_assets::upload_input_schema()}),
    );
    let dr = server.registry().agent_tool(design_review::TOOL).unwrap();
    tools.push(
        json!({"name": dr.name, "tier": dr.tier.as_str(), "description": dr.description,
        "inputSchema": meldframe_http_review::input_schema()}),
    );
    let doc = json!({"batchLimit": meldframe_http::mcp::MAX_BATCH, "tools": tools});
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs/mcp-tools.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}
