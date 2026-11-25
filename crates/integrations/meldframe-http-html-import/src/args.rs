//! The arguments of `import_html`, read and checked whole before anything is parsed: a value that is not what its slot
//! wants is refused with the reason, never cut or guessed (`docs/import-html.md` section 4.1).

use crate::schema::{MAX_IMPORT_KEY, MAX_NAME};
use crate::{invalid, refuse};
use html_import::html::clean;
use html_import::map::MAX_INPUT_BYTES;
use meldframe_activity::Note;
use meldframe_check::Limits;
use serde::Deserialize;
use serde_json::{Map, Value};

/// The arguments as they arrive. Unknown names are refused.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    file: String,
    html: String,
    css: Option<String>,
    #[serde(rename = "parentId")]
    parent_id: Option<String>,
    name: Option<String>,
    width: Option<f64>,
    #[serde(rename = "idempotencyKey")]
    camel: Option<String>,
    idempotency_key: Option<String>,
    note: Option<String>,
    #[serde(rename = "dryRun")]
    dry_run: Option<bool>,
    strict: Option<bool>,
}

/// The arguments of a call that passed every check that needs no parse.
#[derive(Debug)]
pub struct Args {
    pub file: String,
    pub html: String,
    pub css: String,
    pub parent_id: Option<String>,
    pub name: Option<String>,
    pub width: Option<f64>,
    pub key: String,
    pub note: Option<Note>,
    pub dry_run: bool,
    pub strict: bool,
}

impl Args {
    /// Check the arguments. The error is the body of a tool error.
    pub fn parse(a: Map<String, Value>) -> Result<Args, Value> {
        let raw: Raw = serde_json::from_value(Value::Object(a)).map_err(|e| {
            refuse(
                "bad_request",
                format!("the arguments do not fit the tool's input schema: {e}"),
            )
        })?;
        if !meldframe_http::valid_file(&raw.file) {
            return Err(invalid(
                "a file name is 1 to 128 bytes without slashes or control characters",
            ));
        }
        let css = raw.css.unwrap_or_default();
        let total = raw.html.len() + css.len();
        if total > MAX_INPUT_BYTES {
            return Err(refuse(
                "E_IMPORT_HTML_TOO_LARGE",
                format!(
                    "the html and the css are {total} bytes together and an import reads at most {MAX_INPUT_BYTES}: send one section at a time"
                ),
            ));
        }
        let key = match (raw.camel, raw.idempotency_key) {
            (Some(_), Some(_)) => {
                return Err(invalid(
                    "send idempotencyKey or its snake_case spelling, not both",
                ));
            }
            (a, b) => a.or(b),
        };
        let key = key.ok_or_else(|| {
            invalid(
                "every call needs an idempotencyKey (or idempotency_key): it makes a retry safe",
            )
        })?;
        if key.is_empty()
            || key.chars().count() > MAX_IMPORT_KEY
            || key.chars().any(char::is_control)
        {
            return Err(invalid(format!(
                "the idempotency key of this tool is 1 to {MAX_IMPORT_KEY} characters without control characters (the digest of the text is added to it)"
            )));
        }
        let limits = Limits::default();
        if let Some(p) = &raw.parent_id
            && (p.is_empty()
                || p.chars().count() > limits.max_id
                || p.chars().any(char::is_control))
        {
            return Err(invalid(format!(
                "parentId is the id of a frame: 1 to {} characters without control characters",
                limits.max_id
            )));
        }
        if let Some(n) = &raw.name
            && (n.trim().is_empty() || n.chars().count() > MAX_NAME || clean(n, usize::MAX) != *n)
        {
            return Err(invalid(format!(
                "name is 1 to {MAX_NAME} characters, not blank, without control or hidden characters"
            )));
        }
        if let Some(w) = raw.width
            && !(w.is_finite() && w > 0.0 && w <= limits.max_size)
        {
            return Err(refuse(
                "E_OUT_OF_RANGE",
                format!(
                    "width is a number above 0 and at most {} px",
                    limits.max_size
                ),
            ));
        }
        let note = Note::parse_arg(raw.note.as_deref()).map_err(|e| invalid(e.to_string()))?;
        Ok(Args {
            file: raw.file,
            html: raw.html,
            css,
            parent_id: raw.parent_id,
            name: raw.name,
            width: raw.width,
            key,
            note,
            dry_run: raw.dry_run.unwrap_or(false),
            strict: raw.strict.unwrap_or(false),
        })
    }
}
