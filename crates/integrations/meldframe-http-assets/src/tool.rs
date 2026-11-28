//! The MCP tool `upload_asset`: an agent sends an image as base64 and gets back its id.
//!
//! It ends where the upload route does, in `State_::store`: the same checks of the bytes, the same storage limit and
//! the same count. A call is bounded by the size of an MCP request, so it carries a smaller image than the route;
//! a base64 text that is over the image limit is refused before it is decoded.

use super::state::{Refused, State_};
use assets::{AssetError, MAX_ASSET_BYTES, UPLOAD_DESCRIPTION, UPLOAD_TOOL};
use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use meldframe_host::{Principal, Refusal};
use meldframe_http::mcp::{Extra, ExtraCall};
use meldframe_module::Policy;
use rmcp::model::{CallToolResult, Tool, ToolAnnotations};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The standard alphabet, with the padding optional.
const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    json!({"type": "object", "required": ["data"], "additionalProperties": false, "properties": {
        "data": {"type": "string", "minLength": 1,
            "description": "The bytes of one png, jpeg, webp or gif image, as base64 (standard alphabet, padding allowed)."}
    }})
}

pub(crate) struct UploadTool {
    pub(crate) state: Arc<State_>,
    pub(crate) policy: Option<Policy>,
}

impl std::fmt::Debug for UploadTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadTool")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    data: String,
}

fn failure(code: &str, message: &str, retryable: bool) -> CallToolResult {
    CallToolResult::structured_error(json!({"ok": false, "error": {
        "code": code, "message": message, "retryable": retryable}}))
}

fn asset_failure(e: &AssetError) -> CallToolResult {
    failure(e.code(), &e.to_string(), e.retryable())
}

impl UploadTool {
    async fn run(&self, who: Principal, a: Map<String, Value>) -> CallToolResult {
        if self.policy != Some(Policy::Auto) {
            return failure(
                "forbidden",
                "the policy does not allow an upload for this call",
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
        // Base64 is 4 characters for 3 bytes: text over that is more than an image can be.
        if args.data.len() > MAX_ASSET_BYTES / 3 * 4 + 4 {
            return asset_failure(&AssetError::TooLarge {
                limit: MAX_ASSET_BYTES,
            });
        }
        // Line breaks and a missing or present padding are accepted: that is what `base64` of a file prints and what
        // many encoders leave out.
        let text: String = args.data.split_ascii_whitespace().collect();
        let Ok(bytes) = LENIENT.decode(text) else {
            return failure(
                "invalid",
                "data is not base64 (the standard alphabet, padding allowed)",
                false,
            );
        };
        let (state, ws) = (self.state.clone(), who.workspace.clone());
        match self
            .state
            .shared
            .blocking(&who, move || state.store(&ws, &bytes))
            .await
        {
            Err(_) => failure(
                "unavailable",
                "the server cannot take this call now; try again later",
                true,
            ),
            Ok(Err(Refused::Asset(e))) => asset_failure(&e),
            Ok(Err(Refused::Storage(limit))) => {
                let r = Refusal::StorageExceeded { limit };
                failure(r.code(), &r.to_string(), r.retryable())
            }
            Ok(Ok(stored)) => CallToolResult::structured(json!({"ok": true, "id": stored.id,
                "type": stored.kind.mime(), "bytes": stored.bytes,
                "use": {"image": {"assetId": stored.id, "fit": "cover"}}})),
        }
    }
}

impl Extra for UploadTool {
    fn tool(&self) -> Tool {
        let schema = match input_schema() {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        Tool::new(UPLOAD_TOOL, UPLOAD_DESCRIPTION, schema).annotate(
            ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(true),
        )
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(self.run(who, args))
    }
}
