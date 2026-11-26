//! The MCP tool `capture`.

use crate::hub::{Cancel, Hub};
use base64::Engine;
use capture::{CaptureError, DESCRIPTION};
use meldframe_host::Principal;
use meldframe_http::Failure;
use meldframe_http::mcp::{Extra, ExtraCall};
use meldframe_module::Policy;
use rmcp::model::{CallToolResult, ContentBlock, Tool, ToolAnnotations};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The tool's name.
pub const NAME: &str = "capture";

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    json!({"type": "object", "additionalProperties": false, "properties": {
        "file": {"type": "string", "minLength": 1, "maxLength": 128,
            "description": "The file to capture. Name it on your first call: the session is bound to it, and naming another file later is refused with room_mismatch. Leave it out to use the bound file."},
        "nodeId": {"type": "string", "minLength": 1, "maxLength": 128,
            "description": "The node to capture. Leave it out to capture the whole page."},
        "scale": {"type": "number", "exclusiveMinimum": 0, "maximum": 4, "default": 1,
            "description": "The render scale: above 0 and at most 4. Default 1."}
    }})
}

#[derive(Debug)]
pub(crate) struct CaptureTool(pub(crate) Arc<Hub>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    file: Option<String>,
    node_id: Option<String>,
    scale: Option<f64>,
}

fn retryable(code: &str) -> bool {
    matches!(
        code,
        "timeout"
            | "stale_client"
            | "busy"
            | "client_disconnected"
            | "no_capable_client"
            | "render_failed"
    )
}

fn failure(code: &str, message: &str) -> CallToolResult {
    CallToolResult::structured_error(json!({"ok": false, "error": {
        "code": code, "message": message, "retryable": retryable(code)}}))
}

fn refused(e: &CaptureError) -> CallToolResult {
    let why = match e {
        CaptureError::Invalid(m) | CaptureError::RenderFailed(m) => m.clone(),
        CaptureError::NoCapableClient => "no editor that can capture has this file open".into(),
        CaptureError::NoFileSelected => {
            "no file is selected: pass \"file\" (the file to capture) on your first call".into()
        }
        CaptureError::RoomMismatch => "the session is bound to another file".into(),
        CaptureError::Forbidden => "the policy does not allow capture for this call".into(),
        other => format!("the capture failed: {}", other.code()),
    };
    failure(e.code(), &why)
}

/// What a failed read of the file means to the caller: faithfully, not as `busy`.
fn read_failure(f: &Failure) -> CaptureError {
    match f {
        Failure::Refused(_) | Failure::Batch { .. } | Failure::Approval { .. } => {
            CaptureError::Forbidden
        }
        Failure::Permanent(_) => CaptureError::Invalid("the file cannot be read".into()),
        Failure::Unavailable(_) => CaptureError::Busy,
    }
}

impl CaptureTool {
    async fn run(
        &self,
        who: Principal,
        a: Map<String, Value>,
    ) -> Result<(String, Vec<u8>), CaptureError> {
        let hub = &self.0;
        if hub.policy != Some(Policy::Auto) {
            return Err(CaptureError::Forbidden);
        }
        let args: Args = serde_json::from_value(Value::Object(a)).map_err(|e| {
            CaptureError::Invalid(format!("the arguments do not fit the schema: {e}"))
        })?;
        if args
            .file
            .as_deref()
            .is_some_and(|f| !meldframe_http::valid_file(f))
        {
            return Err(CaptureError::Invalid(
                "a file name is 1 to 128 bytes without slashes or control characters".into(),
            ));
        }
        if args.node_id.as_deref().is_some_and(|n| {
            meldframe_text::plain(
                n,
                meldframe_text::MAX_NODE_REF,
                meldframe_text::MAX_NODE_REF,
            )
            .is_err()
        }) {
            return Err(CaptureError::Invalid(
                "nodeId is 1 to 128 bytes without control or hidden characters".into(),
            ));
        }
        let file = hub.file_of(&who, args.file.as_deref())?;
        // The sequence number of the file, read as part of this call (the call log has one entry, `capture`).
        let (reader, w, f) = (hub.reader.clone(), who.clone(), file.clone());
        let min_seq = match hub.shared.blocking(&who, move || reader.head(&w, &f)).await {
            Ok(Ok(seq)) => seq,
            Ok(Err(failure)) => return Err(read_failure(&failure)),
            Err(_) => return Err(CaptureError::Busy),
        };
        let (client, msg, id, rx) = hub.begin(
            &who,
            &file,
            args.node_id,
            args.scale.unwrap_or(1.0),
            min_seq,
        )?;
        hub.bind(&who, &file);
        // If this call is dropped from here on, the request is aborted and the editor's place freed.
        let _cancel = Cancel {
            hub,
            workspace: who.workspace.clone(),
            request_id: id.clone(),
        };
        let (source, w, f) = (hub.source.clone(), who.clone(), file.clone());
        let sent = hub
            .shared
            .blocking(&who, move || source.publish_ephemeral(&w, &f, client, msg))
            .await;
        // Fail only this request: the editor stays capable for everyone else.
        match sent {
            Ok(true) => {}
            Ok(false) => hub.abort(&who.workspace, &id, CaptureError::ClientDisconnected),
            Err(_) => hub.abort(&who.workspace, &id, CaptureError::Busy),
        }
        hub.wait(rx).await.map(|png| (file, png))
    }
}

impl Extra for CaptureTool {
    fn tool(&self) -> Tool {
        let schema = match input_schema() {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        Tool::new(NAME, DESCRIPTION, schema).annotate(
            ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .idempotent(true),
        )
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(async move {
            match self.run(who, args).await {
                Ok((file, png)) => {
                    let data = base64::engine::general_purpose::STANDARD.encode(&png);
                    let info = json!({"ok": true, "file": file, "bytes": png.len()});
                    CallToolResult::success(vec![
                        ContentBlock::image(data, "image/png"),
                        ContentBlock::text(info.to_string()),
                    ])
                }
                Err(e) => refused(&e),
            }
        })
    }
}
