//! The answer of the tool: the report, the nodes, what was skipped and the assets, as JSON a client reads. Every
//! string that came from the text is cleaned again here, so a control or bidi character never reaches the answer.

use html_import::html::{Entry, clean};
use serde_json::{Value, json};

/// The longest string of the text echoed in the answer, in characters.
const ECHO: usize = 240;

/// One entry of the report.
pub fn entry(e: &Entry) -> Value {
    json!({"kind": e.kind.as_str(), "code": e.code, "name": clean(&e.name, ECHO),
        "message": clean(&e.message, ECHO), "at": e.at})
}
