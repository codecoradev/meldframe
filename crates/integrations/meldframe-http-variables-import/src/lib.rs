//! The MCP tool `import_tokens` (E3.4): the variables import over HTTP.
//!
//! The call parses the text on the blocking pool under the call's slot (a text parser, no code runs), cuts the commands
//! into parts of at most [`MAX_BATCH`] and writes the one part the caller asked for with `Shared::submit_ops`, the
//! handler every transport ends in: the same gate, quota, limits and atomic batch as `apply_commands`. The op of
//! command `i` (counted over the whole import) has the key `{idempotencyKey}:{i}`, so a part sent again writes
//! nothing twice, and the parts of one import never share a key. The plan depends on the text alone, not on the
//! file, so the same text cuts into the same parts however many were written already.

use meldframe_host::{Op, Principal};
use meldframe_http::Shared;
use meldframe_http::mcp::{Extra, ExtraCall, MAX_BATCH, MAX_KEY, change_tool_listing, fnv1a};
use meldframe_module::Policy;
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;
use variables_import::{
    DESCRIPTION, Entry, ImportOptions, MAX_INPUT_BYTES, Report, TOOL, import_css,
};

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    let key = |other: &str| {
        json!({"type": "string", "minLength": 1, "maxLength": MAX_KEY,
            "description": format!("The idempotency key of the call (1 to 100 characters, no control or hidden characters). Send this or {other}, not both.")})
    };
    json!({"type": "object", "required": ["file", "css"], "additionalProperties": false, "properties": {
        "file": {"type": "string", "minLength": 1, "maxLength": 128,
            "description": "The name of the file: 1 to 128 characters, no slashes, control or invisible formatting characters."},
        "css": {"type": "string", "minLength": 1, "maxLength": MAX_INPUT_BYTES,
            "description": "The CSS text to import: custom properties of :root and of a dark rule, and Tailwind v4 @theme blocks. At most 262144 bytes (256 KiB, counted in bytes, not characters)."},
        "idempotencyKey": key("idempotency_key"),
        "idempotency_key": key("idempotencyKey"),
        "part": {"type": "integer", "minimum": 1, "default": 1,
            "description": "Which part of a long import to write (1 to parts). Send the parts in order with the same key and css."},
        "dryRun": {"type": "boolean", "default": false,
            "description": "True: write nothing and return the report and the number of parts."}
    }})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    file: String,
    css: String,
    #[serde(rename = "idempotencyKey")]
    camel: Option<String>,
    idempotency_key: Option<String>,
    part: Option<usize>,
    #[serde(rename = "dryRun")]
    dry_run: Option<bool>,
}

/// FNV-1a over the text, 64 bits, as 12 hex digits: the keys of the ops carry it, so that a key sent again with other
/// text is not taken for the call that wrote the first text.
fn fnv12(s: &str) -> String {
    let h = fnv1a(s.bytes());
    format!("{:012x}", h & 0xffff_ffff_ffff)
}

fn failure(code: &str, message: &str, retryable: bool) -> CallToolResult {
    CallToolResult::structured_error(json!({"ok": false, "error": {
        "code": code, "message": message, "retryable": retryable}}))
}

fn entry(e: &Entry) -> Value {
    json!({"kind": e.kind.as_str(), "code": e.code, "name": e.name, "message": e.message})
}

fn report(r: &Report) -> Value {
    json!({"collections": r.collections, "modes": r.modes, "variables": r.variables, "colors": r.colors,
        "numbers": r.numbers, "aliases": r.aliases, "skipped": r.skipped,
        "entries": r.entries.iter().map(entry).collect::<Vec<_>>(), "omitted": r.omitted})
}

/// The counts of a report, without its entries: what later parts of an import answer.
fn terse(r: &Report) -> Value {
    json!({"collections": r.collections, "modes": r.modes, "variables": r.variables, "colors": r.colors,
        "numbers": r.numbers, "aliases": r.aliases, "skipped": r.skipped, "omitted": r.omitted, "seeEntriesInPart": 1})
}

struct ImportTool {
    shared: Arc<Shared>,
    policy: Option<Policy>,
}

impl std::fmt::Debug for ImportTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportTool")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl ImportTool {
    async fn run(&self, who: Principal, a: Map<String, Value>) -> CallToolResult {
        if self.policy != Some(Policy::Auto) {
            return failure(
                "forbidden",
                "the policy does not allow an import for this call",
                false,
            );
        }
        let args: Args = match serde_json::from_value(Value::Object(a)) {
            Ok(a) => a,
            Err(e) => {
                let m = format!("the arguments do not fit the tool's input schema: {e}");
                return failure("bad_request", &m, false);
            }
        };
        if !meldframe_http::valid_file(&args.file) {
            let m = "a file name is 1 to 128 bytes without slashes, control or invisible formatting characters";
            return failure("invalid", m, false);
        }
        let key = match (args.camel, args.idempotency_key) {
            (Some(_), Some(_)) => {
                return failure(
                    "invalid",
                    "send idempotencyKey or its snake_case spelling, not both",
                    false,
                );
            }
            (a, b) => a.or(b),
        };
        let dry = args.dry_run.unwrap_or(false);
        let key = match (key, dry) {
            (Some(k), _) if meldframe_http::mcp::check_key(&k).is_ok() => Some(k),
            (None, true) => None,
            _ => {
                let m = format!(
                    "every call needs an idempotencyKey of 1 to {MAX_KEY} characters without control or hidden characters: it makes a retry safe"
                );
                return failure("invalid", &m, false);
            }
        };
        let css = args.css;
        let digest = fnv12(&css);
        let job = move || import_css(&css, &ImportOptions::default());
        let imported = match self.shared.blocking(&who, job).await {
            // The reply of the shared handler: busy (`unavailable`, retryable), too slow (`timeout`), a panic (`internal`).
            Err(reply) => {
                return CallToolResult::structured_error(
                    json!({"ok": false, "error": reply.body["error"]}),
                );
            }
            Ok(Err(e)) => return failure(e.code(), &e.to_string(), false),
            Ok(Ok(i)) => i,
        };
        let parts = imported.commands.len().div_ceil(MAX_BATCH);
        let part = args.part.unwrap_or(1);
        let mut out = json!({"ok": true, "file": args.file, "parts": parts, "part": part,
            "commands": imported.commands.len(),
            "report": if part > 1 && !dry { terse(&imported.report) } else { report(&imported.report) }});
        if part == 0 || (parts > 0 && part > parts) {
            let m = format!("part is 1 to {parts} for this text; this call has {part}");
            return failure("invalid", &m, false);
        }
        if dry || parts == 0 {
            out["written"] = json!(0);
            out["dryRun"] = json!(dry);
            return CallToolResult::structured(out);
        }
        let key = key.unwrap_or_default();
        let first = (part - 1) * MAX_BATCH;
        let ops: Vec<Op> = imported
            .commands
            .iter()
            .enumerate()
            .skip(first)
            .take(MAX_BATCH)
            .map(|(i, c)| Op::new(format!("{key}:{digest}:{i}"), c.clone()))
            .collect();
        let count = ops.len();
        let reply = self
            .shared
            .submit_ops(who, TOOL.into(), args.file.clone(), ops, Some(vec![]))
            .await;
        if !reply.status.is_success() {
            let mut e = reply.body["error"].clone();
            // The index the batch names counts from this part; the caller counts commands over the whole import.
            if let Some(i) = e["index"].as_u64() {
                e["index"] = json!(i + first as u64);
            }
            if e["code"] == "E_DUPLICATE_ID" {
                e["message"] = json!(format!(
                    "{} (the file already holds a variable or collection with an id of this import. To finish an import that stopped part way, send the same call again with the same idempotencyKey and css; an import adds and does not update, so what a file holds is changed or removed with apply_commands, not imported again)",
                    e["message"].as_str().unwrap_or("duplicate id")
                ));
            }
            return CallToolResult::structured_error(json!({"ok": false, "error": e}));
        }
        let done = reply.body["accepted"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let seq = done
            .iter()
            .filter_map(|d| d["seq"].as_u64())
            .max()
            .unwrap_or(0);
        let replayed = !done.is_empty() && done.iter().all(|d| d["written"] == json!(false));
        out["idempotencyKey"] = json!(key);
        out["written"] = json!(count);
        out["seq"] = json!(seq);
        out["replayed"] = json!(replayed);
        CallToolResult::structured(out)
    }
}

impl Extra for ImportTool {
    fn tool(&self) -> Tool {
        change_tool_listing(TOOL, DESCRIPTION, input_schema())
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(self.run(who, args))
    }
}

/// The tool for `McpConfig::extra`. `policy` is what the registry says for [`TOOL`]; without the module (`None`) the
/// tool serves nothing. The gate still decides every write.
pub fn tool(shared: Arc<Shared>, policy: Option<Policy>) -> Arc<dyn Extra> {
    Arc::new(ImportTool { shared, policy })
}
