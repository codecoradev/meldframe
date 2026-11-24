//! The MCP tool `export_frame` (E4.1): the export feature over HTTP.
//!
//! The server gives a [`FrameSource`] (one read of a file's nodes and variables, under the Read policy of the tool);
//! the call runs on the blocking pool under the call's slot like every other read, and the answer is the document,
//! the stylesheet and the report as JSON. The call log has one entry for the call, written by the MCP handler
//! (`export_frame`, the file, the outcome code), so the source must not log a second one.
//!
//! The text measurer is the stub of the layout core: the server has no browser (`docs/fidelity-contract.md`,
//! section 6). The answer says so under `textMeasurer`, and the report warns `text_size_estimated` for the text
//! whose box that measurer decides (`ExportOptions::estimated_text`).

use export::{DESCRIPTION, ExportOptions, Format, TOOL};
use meldframe_host::Principal;
use meldframe_http::Failure;
use meldframe_http::Shared;
use meldframe_http::mcp::{Extra, ExtraCall};
use meldframe_layout::StubMeasurer;
use meldframe_module::Policy;
use meldframe_protocol::{Node, Variables};
use meldframe_render::ModeChoice;
use rmcp::model::{CallToolResult, Tool, ToolAnnotations};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The most modes one call may pick: a file holds at most 32 collections (ADR-0017).
const MAX_MODES: usize = 32;

/// A file as the export reads it.
#[derive(Debug, Clone)]
pub struct Frames {
    pub seq: u64,
    pub nodes: Vec<Node>,
    pub vars: Variables,
}

/// Read access to a file for the tool. It blocks, so the tool calls it from the blocking pool; a file that does not
/// exist reads as seq 0 with no nodes.
pub trait FrameSource: Send + Sync {
    fn read(&self, who: &Principal, file: &str) -> Result<Frames, Failure>;
}

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    json!({"type": "object", "required": ["file", "frame"], "additionalProperties": false, "properties": {
        "file": {"type": "string", "minLength": 1, "maxLength": 128,
            "description": "The name of the file: 1 to 128 characters, no slashes or control characters."},
        "frame": {"type": "string", "minLength": 1, "maxLength": 64, "pattern": "^[A-Za-z0-9_:.-]+$",
            "description": "The id of the frame to export, with everything inside it: 1 to 64 letters, digits, `_`, `-`, `:` and `.`."},
        "format": {"enum": ["css", "tailwind"], "default": "css",
            "description": "css: one rule per node in an inline stylesheet. tailwind: Tailwind v4 utilities on each element."},
        "modes": {"type": "object", "maxProperties": MAX_MODES, "additionalProperties": {"type": "string"},
            "description": "The mode to export for a collection, as collection id to mode id. A collection that is not listed shows its default mode."}
    }})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    file: String,
    frame: String,
    format: Option<String>,
    modes: Option<BTreeMap<String, String>>,
}

/// A node id as the file allows it (`meldframe-check`): checked before the id is looked up, logged or echoed.
fn valid_frame(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'))
}

fn failure(code: &str, message: &str, retryable: bool) -> CallToolResult {
    CallToolResult::structured_error(json!({"ok": false, "error": {
        "code": code, "message": message, "retryable": retryable}}))
}

fn read_failure(f: &Failure) -> CallToolResult {
    match f {
        Failure::Refused(r) => failure(r.code(), &r.to_string(), r.retryable()),
        Failure::Approval { code, message, .. } => failure(code, message, false),
        Failure::Batch { refusal, .. } => failure(refusal.code(), &refusal.to_string(), false),
        Failure::Unavailable(_) => failure(
            "unavailable",
            "the file cannot be read now; try again later",
            true,
        ),
        Failure::Permanent(_) => failure("unavailable", "the file cannot be read", false),
    }
}

fn findings(list: &[export::Finding]) -> Value {
    list.iter()
        .map(|f| json!({"code": f.code, "nodeId": f.node, "message": f.message}))
        .collect()
}

struct ExportTool {
    shared: Arc<Shared>,
    source: Arc<dyn FrameSource>,
    policy: Option<Policy>,
}

impl std::fmt::Debug for ExportTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportTool")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl ExportTool {
    async fn run(&self, who: Principal, a: Map<String, Value>) -> CallToolResult {
        if self.policy != Some(Policy::Auto) {
            return failure(
                "forbidden",
                "the policy does not allow an export for this call",
                false,
            );
        }
        let args: Args = match serde_json::from_value(Value::Object(a)) {
            Ok(a) => a,
            Err(e) => {
                return failure(
                    "bad_request",
                    &format!("the arguments do not fit the tool's input schema: {e}"),
                    false,
                );
            }
        };
        let format = match args.format.as_deref() {
            None | Some("css") => Format::Css,
            Some("tailwind") => Format::Tailwind,
            Some(other) => {
                return failure(
                    "invalid",
                    &format!("format is css or tailwind, not {other}"),
                    false,
                );
            }
        };
        if !valid_frame(&args.frame) {
            return failure(
                "invalid",
                "a frame id is 1 to 64 letters, digits, `_`, `-`, `:` and `.`",
                false,
            );
        }
        if !meldframe_http::valid_file(&args.file) {
            return failure(
                "invalid",
                "a file name is 1 to 128 bytes without slashes or control characters",
                false,
            );
        }
        let mut modes = ModeChoice::default();
        for (collection, mode) in args.modes.iter().flatten() {
            modes.set(collection, mode);
        }
        let (source, who2, file, frame) = (
            self.source.clone(),
            who.clone(),
            args.file.clone(),
            args.frame.clone(),
        );
        let job = move || {
            let f = source.read(&who2, &file)?;
            let options = ExportOptions::new(format, &StubMeasurer)
                .modes(modes)
                .estimated_text(true);
            Ok::<_, Failure>((
                f.seq,
                export::export_frame(&f.nodes, &f.vars, &frame, &options),
            ))
        };
        match self.shared.blocking(&who, job).await {
            Err(_) => failure(
                "unavailable",
                "the server cannot take this call now; try again later",
                true,
            ),
            Ok(Err(f)) => read_failure(&f),
            Ok(Ok((_, Err(e)))) => failure(e.code(), &e.to_string(), false),
            Ok(Ok((seq, Ok(e)))) => CallToolResult::structured(json!({
                "ok": true, "file": args.file, "frame": args.frame, "seq": seq,
                "format": args.format.unwrap_or_else(|| "css".into()),
                "html": e.html, "css": e.css, "bytes": e.html.len() + e.css.len(),
                "textMeasurer": "stub",
                "report": {"unsupported": findings(&e.report.unsupported),
                    "warnings": findings(&e.report.warnings), "assets": e.report.assets}})),
        }
    }
}

impl Extra for ExportTool {
    fn tool(&self) -> Tool {
        let schema = match input_schema() {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        Tool::new(TOOL, DESCRIPTION, schema).annotate(
            ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .idempotent(true),
        )
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(self.run(who, args))
    }
}

/// The tool for `McpConfig::extra`. `policy` is what the registry says for [`TOOL`]; the tool refuses unless it is
/// `Auto`, so a build without the module serves nothing.
pub fn tool(
    shared: Arc<Shared>,
    source: Arc<dyn FrameSource>,
    policy: Option<Policy>,
) -> Arc<dyn Extra> {
    Arc::new(ExportTool {
        shared,
        source,
        policy,
    })
}
