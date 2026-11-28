//! The checks of the arguments of a call, before anything is read or written.

use super::schema::{Args, MAX_ID, MAX_TOOL_KEY, MAX_WIDTH};
use super::tool::HtmlImport;
use meldframe_activity::Note;
use meldframe_http::mcp::tool_error as error;
use meldframe_http::valid_file;
use meldframe_module::Policy;
use serde_json::{Map, Value};

fn invalid(message: impl Into<String>) -> Value {
    error("invalid", message, false)
}

/// The checked arguments of a call.
pub(super) struct Call {
    pub(super) args: Args,
    pub(super) key: Option<String>,
    pub(super) note: Option<Note>,
}

impl HtmlImport {
    pub(super) fn check(&self, a: Map<String, Value>) -> Result<Call, Value> {
        if self.policy != Some(Policy::Auto) {
            return Err(error(
                "forbidden",
                "the policy does not allow an import for this call",
                false,
            ));
        }
        let args: Args = serde_json::from_value(Value::Object(a)).map_err(|e| {
            error(
                "bad_request",
                format!("the arguments do not fit the tool's input schema: {e}"),
                false,
            )
        })?;
        if !valid_file(&args.file) {
            return Err(invalid(
                "a file name is 1 to 128 bytes without slashes, control or invisible formatting characters",
            ));
        }
        let key = match (&args.camel, &args.idempotency_key) {
            (Some(_), Some(_)) => {
                return Err(invalid(
                    "send idempotencyKey or its snake_case spelling, not both",
                ));
            }
            (a, b) => a.clone().or_else(|| b.clone()),
        };
        let dry = args.dry_run.unwrap_or(false);
        match &key {
            Some(k) if meldframe_http::mcp::check_key(k).is_err() || k.len() > MAX_TOOL_KEY => {
                return Err(invalid(format!(
                    "the idempotency key is 1 to {MAX_TOOL_KEY} characters (and bytes) without control or hidden characters, since the digest of the call is added to it"
                )));
            }
            None if !dry => {
                return Err(invalid(
                    "every call needs an idempotencyKey (or idempotency_key): it makes a retry safe",
                ));
            }
            _ => {}
        }
        if let Some(n) = &args.name
            && (n.is_empty() || meldframe_text::plain(n, 256, 1024).is_err())
        {
            return Err(invalid(
                "name is 1 to 256 characters without control or invisible formatting characters",
            ));
        }
        if let Some(p) = &args.parent_id
            && (p.is_empty() || meldframe_text::plain(p, MAX_ID, 4 * MAX_ID).is_err())
        {
            return Err(invalid(format!(
                "parentId is 1 to {MAX_ID} characters without control or invisible formatting characters"
            )));
        }
        if let Some(w) = args.width
            && !(w.is_finite() && w > 0.0 && w <= MAX_WIDTH)
        {
            return Err(invalid(format!(
                "width is a number above 0 and at most {MAX_WIDTH}"
            )));
        }
        let note = Note::parse_arg(args.note.as_deref()).map_err(|e| invalid(e.to_string()))?;
        if note.is_some() && self.shared.batches.get().is_none() {
            return Err(invalid("this server keeps no notes: leave note out"));
        }
        Ok(Call { args, key, note })
    }
}
