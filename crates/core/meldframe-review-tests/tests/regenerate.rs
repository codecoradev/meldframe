//! Rewrites the catalogue of the tools, `docs/mcp-tools.json`, from the server.

use meldframe_server::Server;
use serde_json::{Value, json};

/// Rewrites `docs/mcp-tools.json` from the server: `cargo test -p meldframe-review-tests --test regenerate
/// -- --ignored regenerate`. The tests of `meldframe-server` (`catalogue.rs`, `skill.rs`) then check the file against a
/// client and the registry.
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
    let mv = server
        .registry()
        .agent_tool(meldframe_http_variant::TOOL)
        .unwrap();
    tools.push(
        json!({"name": mv.name, "tier": mv.tier.as_str(), "description": mv.description,
        "inputSchema": meldframe_http_variant::input_schema()}),
    );
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
    let ex = server.registry().agent_tool(export::TOOL).unwrap();
    tools.push(
        json!({"name": ex.name, "tier": ex.tier.as_str(), "description": ex.description,
        "inputSchema": meldframe_http_export::input_schema()}),
    );
    let vi = server
        .registry()
        .agent_tool(variables_import::TOOL)
        .unwrap();
    tools.push(
        json!({"name": vi.name, "tier": vi.tier.as_str(), "description": vi.description,
        "inputSchema": meldframe_http_variables_import::input_schema()}),
    );
    let hi = server.registry().agent_tool(html_import::TOOL).unwrap();
    tools.push(
        json!({"name": hi.name, "tier": hi.tier.as_str(), "description": hi.description,
        "inputSchema": meldframe_http_html_import::input_schema()}),
    );
    let gb = server
        .registry()
        .agent_tool(meldframe_http_activity::TOOL)
        .unwrap();
    tools.push(
        json!({"name": gb.name, "tier": gb.tier.as_str(), "description": gb.description,
        "inputSchema": meldframe_http_activity::input_schema()}),
    );
    let doc = json!({"batchLimit": meldframe_http::mcp::MAX_BATCH, "tools": tools});
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs/mcp-tools.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}
