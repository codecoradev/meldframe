#![allow(dead_code, reason = "each test binary uses a part of these helpers")]
//! Helpers of the import_html tests: the arguments of a call and the code of an error.

use meldframe_http_html_import::{Args, Planned, make};
use serde_json::{Map, Value, json};

pub const ACTOR: &str = "agent:alpha";

/// The arguments of a valid call, with `extra` set over them.
pub fn args_of(html: &str, extra: Value) -> Map<String, Value> {
    let mut a = json!({"file": "f", "html": html, "idempotencyKey": "k1"})
        .as_object()
        .unwrap()
        .clone();
    a.extend(extra.as_object().unwrap().clone());
    a
}

/// The code of the error a body holds.
pub fn code(e: &Value) -> String {
    assert_eq!(e["ok"], json!(false), "{e}");
    assert_eq!(e["error"]["retryable"], json!(false), "{e}");
    e["error"]["code"].as_str().unwrap().to_owned()
}

pub fn message(e: &Value) -> String {
    e["error"]["message"].as_str().unwrap().to_owned()
}

pub fn parse(extra: Value) -> Result<Args, Value> {
    Args::parse(args_of("<div>x</div>", extra))
}

/// Parse and plan a call for `ACTOR`.
pub fn planned(html: &str, extra: Value) -> Result<Planned, Value> {
    make(&Args::parse(args_of(html, extra))?, ACTOR)
}

/// `n` empty boxes of a size inside one box, in a fragment: `n + 2` nodes (the page of a fragment is the root).
pub fn boxes(n: usize) -> String {
    let one = "<div style='width:10px;height:10px;background:#eee'></div>";
    format!("<div id=r style='width:200px'>{}</div>", one.repeat(n))
}
