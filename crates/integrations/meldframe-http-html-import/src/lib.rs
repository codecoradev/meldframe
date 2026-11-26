//! The MCP tool `import_html` (E4.4): the HTML importer over HTTP.
//!
//! The call reads the text on the blocking pool under the call's slot (a bounded parser of our own, nothing is
//! fetched or run), makes at most 100 `addNode` commands and writes them as one atomic batch with
//! `Shared::submit_ops`, the handler every transport ends in: the same gate, quota, limits and call log as
//! `apply_commands`, and one numbered batch in the Activity panel. The op of command `i` has the key
//! `{idempotencyKey}:{digest}:{i}`, where the digest is a hash of the input, so a call sent again after a timeout
//! writes nothing twice and the same key with other text is another call. The ids of the nodes come from the key and
//! the digest too (`imp-<hash>-<i>`), so the same call makes the same ids and the same text under another key makes
//! others.
//!
//! The tool is an [`Extra`] of the MCP route and [`html_import::ImportHtmlModule`] registers it, so a build without
//! the `html-import` feature of the server has neither.

mod answer;

use html_import::map::{ImportError, Imported, MAX_COMMANDS, Options, import};
use html_import::{DESCRIPTION, TOOL};
use meldframe_activity::{BatchCall, MAX_KEY_PREFIX, Note};
use meldframe_host::{Op, Principal};
use meldframe_http::mcp::{
    Extra, ExtraCall, FileView, Reader, change_tool_listing, tool_error as error,
    tool_failed as failed,
};
use meldframe_http::{Reply, Shared, valid_file};
use meldframe_module::Policy;
use meldframe_protocol::{Command, Doc, Node, Variables};
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The room between the root and the frames beside it, as `make_variant` leaves.
const GAP: f64 = meldframe_variant::GAP;
/// The longest key of a call: the key and the digest (a colon and 12 hex digits) are the key prefix of the batch
/// header, which is at most `MAX_KEY_PREFIX` bytes, and a header that is refused would leave the write with no number,
/// no note and no revert.
const MAX_TOOL_KEY: usize = MAX_KEY_PREFIX - 13;
/// The widest root frame a call may ask for (the model's size limit).
const MAX_WIDTH: f64 = 100_000.0;

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
struct Args {
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

/// A tool's answer: the body of a success, or the body of a tool error.
type Body = Result<Value, Value>;

fn invalid(message: impl Into<String>) -> Value {
    error("invalid", message, false)
}

/// FNV-1a over the parts, 64 bits, each followed by a zero byte.
fn fnv(parts: &[&[u8]]) -> u64 {
    parts
        .iter()
        .flat_map(|p| p.iter().copied().chain(std::iter::once(0)))
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
}

struct HtmlImport {
    shared: Arc<Shared>,
    reader: Arc<dyn Reader>,
    policy: Option<Policy>,
}

impl std::fmt::Debug for HtmlImport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtmlImport")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

/// The tool for `McpConfig::extra`. `policy` is what the registry says for [`TOOL`]; without the module (`None`) the
/// tool serves nothing. `reader` places the root beside the other frames and finds what a retry wrote; `shared` runs
/// the work under the slots and the time limit of every other call and ends in the one handler of writes.
pub fn tool(
    shared: Arc<Shared>,
    reader: Arc<dyn Reader>,
    policy: Option<Policy>,
) -> Arc<dyn Extra> {
    Arc::new(HtmlImport {
        shared,
        reader,
        policy,
    })
}

impl Extra for HtmlImport {
    fn tool(&self) -> Tool {
        change_tool_listing(TOOL, DESCRIPTION, input_schema())
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(async move {
            match self.run(&who, args).await {
                Ok(v) => CallToolResult::structured(v),
                Err(v) => CallToolResult::structured_error(v),
            }
        })
    }
}

/// What a write needs: the file, the key and digest of the call, its note and what the importer made.
struct Write<'a> {
    file: &'a str,
    key: &'a str,
    digest: &'a str,
    note: Option<Note>,
    imported: &'a Imported,
}

/// The checked arguments of a call.
struct Call {
    args: Args,
    key: Option<String>,
    note: Option<Note>,
}

impl HtmlImport {
    /// The file as the tool needs it: nodes and variables, from one read that is not a call of its own.
    async fn view(&self, who: &Principal, file: &str) -> Result<(FileView, Variables), Value> {
        let (reader, w, f) = (self.reader.clone(), who.clone(), file.to_owned());
        match self
            .shared
            .blocking(who, move || reader.view_with_variables(&w, &f))
            .await
        {
            Err(reply) => Err(failed(&reply)),
            Ok(Err(failure)) => Err(failed(&Reply::failure(&failure))),
            Ok(Ok(v)) => Ok(v),
        }
    }

    fn check(&self, a: Map<String, Value>) -> Result<Call, Value> {
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

    async fn run(&self, who: &Principal, a: Map<String, Value>) -> Body {
        let Call { args, key, note } = self.check(a)?;
        let dry = args.dry_run.unwrap_or(false);
        let css = args.css.clone().unwrap_or_default();
        let width = args.width.unwrap_or(1440.0);
        let digest = format!(
            "{:012x}",
            fnv(&[
                args.html.as_bytes(),
                css.as_bytes(),
                args.name.as_deref().unwrap_or("").as_bytes(),
                &width.to_bits().to_le_bytes(),
                args.parent_id.as_deref().unwrap_or("").as_bytes(),
            ]) & 0xffff_ffff_ffff
        );
        // A dry run without a key makes its ids from the key `dry-run`: they are not the ids of a call that writes. The
        // actor is in the hash, so two actors that send the same key and the same HTML do not make the same ids.
        let key = key.unwrap_or_else(|| "dry-run".into());
        let prefix = format!(
            "imp-{:012x}",
            fnv(&[
                who.actor.as_bytes(),
                args.file.as_bytes(),
                key.as_bytes(),
                digest.as_bytes()
            ]) & 0xffff_ffff_ffff
        );
        let opts = Options {
            id_prefix: prefix,
            name: args.name.clone(),
            width,
            origin: (0.0, 0.0),
            parent: args.parent_id.clone(),
        };
        let html = args.html.clone();
        let mut imported = match self
            .shared
            .blocking(who, move || import(&html, &css, &opts))
            .await
        {
            Err(reply) => return Err(failed(&reply)),
            Ok(Err(e)) => return Err(refusal(&e)),
            Ok(Ok(i)) => i,
        };
        if args.strict.unwrap_or(false) && imported.unsupported_count() > 0 {
            let mut e = error(
                "E_IMPORT_HTML_UNSUPPORTED",
                format!(
                    "strict is on and {} properties were not carried (the first 100 are listed under unsupported): send strict false to import what can be read, or change the HTML",
                    imported.unsupported_count()
                ),
                false,
            );
            e["error"]["unsupported"] = Value::Array(answer::unsupported(&imported));
            return Err(e);
        }
        // The file is read after the text is known to be importable. Where the root goes: beside the other top-level
        // frames, or where the root of an earlier call stands when the file already holds it, so that a retry sends the
        // commands the first call wrote.
        let view = if args.parent_id.is_none() || dry {
            Some(self.view(who, &args.file).await?)
        } else {
            None
        };
        let replay = view
            .as_ref()
            .is_some_and(|(v, _)| v.nodes.iter().any(|n| n.id == imported.root));
        if let (Some((v, _)), None) = (&view, &args.parent_id)
            && let Some(Command::AddNode { node, .. }) = imported.commands.first_mut()
        {
            (node.x, node.y) = place(&v.nodes, &imported.root);
        }
        let mut out = answer::result(&imported);
        if dry {
            // A dry run after the call that wrote is that call again: its root is in the file, so nothing is applied.
            if let Some((v, vars)) = view.filter(|_| !replay) {
                preflight(&imported, v.nodes, vars)?;
            }
            out["dryRun"] = json!(true);
            out["written"] = json!(0);
            out["replayed"] = json!(replay);
            return Ok(out);
        }
        let call = Write {
            file: &args.file,
            key: &key,
            digest: &digest,
            note,
            imported: &imported,
        };
        self.write(who, call, out).await
    }

    async fn write(&self, who: &Principal, w: Write<'_>, mut out: Value) -> Body {
        let Write {
            file,
            key,
            digest,
            note,
            imported,
        } = w;
        let ops: Vec<Op> = imported
            .commands
            .iter()
            .enumerate()
            .map(|(i, c)| Op::new(format!("{key}:{digest}:{i}"), c.clone()))
            .collect();
        let call = BatchCall {
            file: file.to_owned(),
            key: format!("{key}:{digest}"),
            commands: ops.len(),
            atomic: true,
            note,
        };
        let reply = self
            .shared
            .submit_ops(who.clone(), TOOL.into(), file.to_owned(), ops, Some(vec![]))
            .await;
        if !reply.status.is_success() {
            let mut e = reply.body["error"].clone();
            if let Some(src) = e["index"]
                .as_u64()
                .and_then(|i| imported.nodes.get(usize::try_from(i).ok()?))
            {
                e["source"] = json!(src.source);
                e["nodeId"] = json!(src.id);
            }
            return Err(json!({"ok": false, "error": e}));
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
        let seen: Option<Vec<(u64, bool)>> = done
            .iter()
            .map(|d| Some((d["seq"].as_u64()?, d["written"] == json!(true))))
            .collect();
        let batch = match (seen, self.shared.batches.get().cloned()) {
            (Some(seen), Some(sink)) => {
                let w = who.clone();
                self.shared
                    .blocking(who, move || {
                        sink.record(&w.workspace, &w.actor, &call, Some(&seen))
                    })
                    .await
                    .ok()
                    .flatten()
            }
            _ => None,
        };
        out["idempotencyKey"] = json!(key);
        out["dryRun"] = json!(false);
        out["written"] = json!(imported.commands.len());
        out["seq"] = json!(seq);
        out["replayed"] = json!(replayed);
        out["batch"] = batch.map_or(Value::Null, |m| json!({"n": m.n, "note": m.note}));
        Ok(out)
    }
}

/// To the right of every top-level frame, [`GAP`] apart, at the top; where the root of an earlier call stands when
/// the file already holds it.
fn place(nodes: &[Node], root: &str) -> (f64, f64) {
    if let Some(n) = nodes.iter().find(|n| n.id == root) {
        return (n.x, n.y);
    }
    let right = nodes
        .iter()
        .filter(|n| n.parent_id.is_none())
        .map(|n| n.x + n.w)
        .reduce(f64::max);
    (right.map_or(0.0, |r| r + GAP), 0.0)
}

/// The refusals of the importer as tool errors.
fn refusal(e: &ImportError) -> Value {
    error(e.code(), e.to_string(), false)
}

/// What a write would be refused for: the commands applied in order to a copy of the file under the limits of the
/// server, as `make_variant` checks its copy. The error names the command and the element it came from.
fn preflight(i: &Imported, nodes: Vec<Node>, vars: Variables) -> Result<(), Value> {
    let mut doc = Doc { nodes, vars };
    let limits = meldframe_core::Limits::default();
    for (n, c) in i.commands.iter().enumerate() {
        if let Err(e) = meldframe_core::apply_doc(&mut doc, c, &limits) {
            let mut v = error(e.code(), e.to_string(), false);
            v["error"]["index"] = json!(n);
            v["error"]["source"] = json!(i.nodes[n].source);
            v["error"]["nodeId"] = json!(i.nodes[n].id);
            return Err(v);
        }
    }
    debug_assert!(i.commands.len() <= MAX_COMMANDS);
    Ok(())
}
