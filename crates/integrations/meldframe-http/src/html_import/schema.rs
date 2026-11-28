//! The input schema of the tool, the arguments it deserializes and the bounds the schema names.

use meldframe_activity::MAX_KEY_PREFIX;
use serde::Deserialize;
use serde_json::{Value, json};

/// The longest key of a call: the key and the digest (a colon and 12 hex digits) are the key prefix of the batch
/// header, which is at most `MAX_KEY_PREFIX` bytes, and a header that is refused would leave the write with no number,
/// no note and no revert.
pub(super) const MAX_TOOL_KEY: usize = MAX_KEY_PREFIX - 13;
/// The widest root frame a call may ask for (the model's size limit).
pub(super) const MAX_WIDTH: f64 = 100_000.0;
/// The longest id of a node (`meldframe-check` `Limits::max_id`): a `parentId` is looked up and hashed, so it is bounded.
pub(super) const MAX_ID: usize = 64;

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    let key = |other: &str| {
        json!({"type": "string", "minLength": 1, "maxLength": MAX_TOOL_KEY,
            "description": format!("The idempotency key of the call (1 to {MAX_TOOL_KEY} characters, no control or hidden characters). Send this or {other}, not both. Not needed with dryRun.")})
    };
    json!({"type": "object", "required": ["file", "html"], "additionalProperties": false, "properties": {
        "file": {"type": "string", "minLength": 1, "maxLength": 128,
            "description": "The name of the file: 1 to 128 characters, no slashes, control or invisible formatting characters. A name that does not exist yet is a new, empty file."},
        "html": {"type": "string", "minLength": 1, "maxLength": 262_144,
            "description": "The HTML to import: a fragment or a whole document. html and css together are at most 262144 bytes (256 KiB, counted in bytes, not characters)."},
        "css": {"type": "string", "maxLength": 262_144,
            "description": "More CSS rules, read after every style block of the HTML (so at equal specificity it wins): the compiled style sheet of the page, or the css that export_frame returned with its tailwind html."},
        "parentId": {"type": "string", "minLength": 1, "maxLength": 64, "pattern": "^[A-Za-z0-9_:.-]+$",
            "description": "The id of an existing frame to add the nodes into. Absent: a new top-level frame to the right of the others."},
        "name": {"type": "string", "minLength": 1, "maxLength": 256,
            "description": "The name of the root frame, 1 to 256 characters; control and invisible formatting characters are refused. Absent: the title of the page, else Imported HTML."},
        "width": {"type": "number", "exclusiveMinimum": 0, "maximum": MAX_WIDTH, "default": 1440,
            "description": "The width of the root frame when the source gives none."},
        "idempotencyKey": key("idempotency_key"),
        "idempotency_key": key("idempotencyKey"),
        "note": {"type": "string", "maxLength": 500,
            "description": "A note for the person who reviews the batch, as in apply_commands: at most 500 characters and 3 line breaks, no hidden characters."},
        "dryRun": {"type": "boolean", "default": false,
            "description": "True: write nothing and return the same result, with the refusals the file would give."},
        "strict": {"type": "boolean", "default": false,
            "description": "True: refuse the call as E_IMPORT_HTML_UNSUPPORTED, and write nothing, when any property was not carried."}
    }})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Args {
    pub(super) file: String,
    pub(super) html: String,
    pub(super) css: Option<String>,
    #[serde(rename = "parentId")]
    pub(super) parent_id: Option<String>,
    pub(super) name: Option<String>,
    pub(super) width: Option<f64>,
    #[serde(rename = "idempotencyKey")]
    pub(super) camel: Option<String>,
    pub(super) idempotency_key: Option<String>,
    pub(super) note: Option<String>,
    #[serde(rename = "dryRun")]
    pub(super) dry_run: Option<bool>,
    pub(super) strict: Option<bool>,
}
