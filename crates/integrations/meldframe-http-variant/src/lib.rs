//! The `make_variant` tool: a copy of a frame and its subtree beside it (E5.4). The commands come from
//! `meldframe-variant`; the call ends where `apply_commands` does, in one atomic `submit_ops`, so the gate, the
//! limits and the log see it as the batch of `addNode`s it is.
//!
//! A retry must find what the first call wrote. The ids of the copy come from the file, the actor, the key and the
//! frame (FNV-1a, 64 bits), so the same call makes the same ids; a key that was used for another frame, or by
//! `apply_commands`, gives other ids and is told apart: a retry is a replay only when the root it names is in the
//! file.
//!
//! The tool is an [`Extra`] of the MCP route and [`VariantModule`] registers it, so a build without the `variant`
//! feature of the server has neither.

use meldframe_check::Limits;
use meldframe_host::{Op, Principal};
use meldframe_http::mcp::{Extra, ExtraCall, FileView, MAX_BATCH, MAX_KEY, Reader};
use meldframe_http::{Failure, Shared, valid_file};
use meldframe_module::{Module, Registrar, RiskTier};
use rmcp::model::{CallToolResult, Tool, ToolAnnotations};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The name of the tool.
pub const TOOL: &str = "make_variant";

/// The text a client reads.
pub const DESCRIPTION: &str = "Copy a frame and everything inside it as a variant beside the original, so you can work on a take and keep the original: the copy has new ids, keeps every property and the order of its children, is named \"<name> variant\", and sits under the same parent to the right of every sibling, 40 px past the rightmost one, at the original's height. The copy is written as one atomic call, all or nothing, and the file's limits are checked before anything is written. A frame whose copy would need more than 100 commands (the frame and its descendants together are over 100 nodes) is refused as invalid: duplicate a smaller frame, or build the copy in several apply_commands calls. Only a frame can be copied (E_NOT_A_FRAME otherwise); an id that is not in the file is E_NODE_NOT_FOUND. Every call needs an idempotencyKey (also accepted as idempotency_key): send the same call again with the same key after a timeout and no second copy is made; the answer then has replayed true. The result names the copy's root id (root), the number of nodes written and seq. Name the layers of the copy as you would any others; it keeps the names of the original.";

const FILE: &str = "The name of the file: 1 to 128 characters, no slashes or control characters. A file belongs to the workspace of your token; a name that does not exist yet is a new, empty file.";

fn key_schema(spelling: &str, other: &str) -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": MAX_KEY,
        "description": format!("The idempotency key of the call (1 to 100 characters, no control or hidden characters). Send {spelling} or {other}, not both.")})
}

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    json!({"type": "object", "required": ["file", "id"], "additionalProperties": false, "properties": {
        "file": {"type": "string", "minLength": 1, "maxLength": 128, "description": FILE},
        "id": {"type": "string", "minLength": 1, "description": "The id of the frame to copy."},
        "idempotencyKey": key_schema("idempotencyKey", "idempotency_key"),
        "idempotency_key": key_schema("idempotency_key", "idempotencyKey")
    }})
}

/// Registers the tool in the registry as a Change. Without it the gate refuses the write as `forbidden`.
#[derive(Debug, Clone, Copy)]
pub struct VariantModule;

impl Module for VariantModule {
    fn name(&self) -> &'static str {
        "variant"
    }

    fn register(&self, r: &mut Registrar) {
        r.agent_tool(TOOL, DESCRIPTION, RiskTier::Change);
    }
}

/// A tool's answer: the body of a success, or the body of a tool error.
type Body = Result<Value, Value>;

fn error(code: &str, message: impl Into<String>, retryable: bool) -> Value {
    json!({"ok": false, "error": {"code": code, "message": message.into(), "retryable": retryable}})
}

fn invalid(message: impl Into<String>) -> Value {
    error("invalid", message, false)
}

/// A refused call, as the one handler words it: the same code, message and extras as REST.
fn failed(reply: &meldframe_http::Reply) -> Value {
    json!({"ok": false, "error": reply.body["error"]})
}

fn args<T: for<'de> Deserialize<'de>>(a: Map<String, Value>) -> Result<T, Value> {
    serde_json::from_value(Value::Object(a)).map_err(|e| {
        error(
            "bad_request",
            format!("the arguments do not fit the tool's input schema: {e}"),
            false,
        )
    })
}

fn check_file(file: &str) -> Result<(), Value> {
    if valid_file(file) {
        Ok(())
    } else {
        Err(invalid(
            "a file name is 1 to 128 bytes without slashes or control characters, and a tool name is 1 to 64 letters, digits, _ - or .",
        ))
    }
}

/// One value from its two spellings: exactly one may be given.
fn one_of<T>(camel: Option<T>, snake: Option<T>, name: &str) -> Result<Option<T>, Value> {
    match (camel, snake) {
        (Some(_), Some(_)) => Err(invalid(format!(
            "send {name} or its snake_case spelling, not both"
        ))),
        (a, b) => Ok(a.or(b)),
    }
}

fn check_key(key: &str) -> Result<(), Value> {
    meldframe_http::mcp::check_key(key).map_err(|_| {
        invalid(format!(
            "the idempotency key is 1 to {MAX_KEY} characters without control or hidden characters"
        ))
    })
}

/// The id of node `n` of the copy of `frame` made under `key` by `actor` in `file`.
fn fresh_id(file: &str, actor: &str, key: &str, frame: &str, n: usize) -> String {
    let h = [file, actor, key, frame]
        .iter()
        .flat_map(|s| s.bytes().chain(std::iter::once(0)))
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("v-{h:016x}-{n}")
}

struct Variant {
    shared: Arc<Shared>,
    reader: Arc<dyn Reader>,
}

impl std::fmt::Debug for Variant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Variant").finish_non_exhaustive()
    }
}

/// The tool, for `McpConfig::extra`. `reader` finds what a retry wrote and reads the frame; `shared` runs the work
/// under the slots and the time limit of every other call and ends in the one handler of writes.
pub fn tool(shared: Arc<Shared>, reader: Arc<dyn Reader>) -> Arc<dyn Extra> {
    Arc::new(Variant { shared, reader })
}

impl Extra for Variant {
    fn tool(&self) -> Tool {
        let schema = match input_schema() {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        Tool::new(TOOL, DESCRIPTION, schema).annotate(
            ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(true),
        )
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(async move {
            match self.make_variant(&who, args).await {
                Ok(v) => CallToolResult::structured(v),
                Err(v) => CallToolResult::structured_error(v),
            }
        })
    }
}

impl Variant {
    /// Run a read on the blocking pool under the same slot and time limit as a write.
    async fn read<T: Send + 'static>(
        &self,
        who: &Principal,
        f: impl FnOnce(&dyn Reader, &Principal) -> Result<T, Failure> + Send + 'static,
    ) -> Result<T, Value> {
        let reader = self.reader.clone();
        let w = who.clone();
        match self
            .shared
            .blocking(who, move || f(reader.as_ref(), &w))
            .await
        {
            Err(reply) => Err(failed(&reply)),
            Ok(Err(failure)) => Err(failed(&meldframe_http::Reply::failure(&failure))),
            Ok(Ok(v)) => Ok(v),
        }
    }

    /// How many commands of `key` the file holds, and the highest seq among them.
    async fn written(&self, who: &Principal, file: &str, key: &str) -> Result<(usize, u64), Value> {
        let (f, keys) = (
            file.to_owned(),
            (0..MAX_BATCH)
                .map(|i| format!("{key}:{i}"))
                .collect::<Vec<_>>(),
        );
        // `written` is the look at the leading keys that are in the log without being a call of its own: not an entry of
        // `batch_status` that the agent never made, nor a dependency on that tool's policy (as in the activity and
        // checkpoint tools).
        let seqs = self.read(who, move |r, w| r.written(w, &f, &keys)).await?;
        Ok((seqs.len(), seqs.iter().copied().max().unwrap_or(0)))
    }

    /// The answer of a retry whose keys are written: the copy of this call when its root is in the file, else the
    /// key belongs to another call.
    async fn replay(&self, who: &Principal, c: &Call<'_>) -> Result<Option<Value>, Value> {
        let (n, seq) = self.written(who, c.file, c.key).await?;
        if n == 0 {
            return Ok(None);
        }
        // The same unlogged read as the copy itself makes (the variables are not needed here): the call has its own entries,
        // and a retry leaves the same log as a first call.
        let name = c.file.to_owned();
        let (f, _) = self
            .read(who, move |r, w| r.view_with_variables(w, &name))
            .await?;
        if f.nodes.iter().any(|x| x.id == c.root) {
            return Ok(Some(c.result(n, seq, true)));
        }
        Err(invalid(
            "this idempotencyKey was already used by another call (another frame, or apply_commands): use a new key",
        ))
    }

    async fn make_variant(&self, who: &Principal, a: Map<String, Value>) -> Body {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            file: String,
            id: String,
            #[serde(rename = "idempotencyKey")]
            camel: Option<String>,
            idempotency_key: Option<String>,
        }
        let A {
            file,
            id,
            camel,
            idempotency_key,
        } = args(a)?;
        check_file(&file)?;
        let key = one_of(camel, idempotency_key, "idempotencyKey")?.ok_or_else(|| {
            invalid(
                "every call needs an idempotencyKey (or idempotency_key): it makes a retry safe",
            )
        })?;
        check_key(&key)?;
        let call = Call {
            file: &file,
            key: &key,
            root: fresh_id(&file, &who.actor, &key, &id, 0),
        };
        if let Some(done) = self.replay(who, &call).await? {
            return Ok(done);
        }
        // The copy keeps the fields bound to variables, and the variables live in the file: they are checked together, so
        // the nodes and the variables are read as one picture of the file.
        let name = file.clone();
        let (FileView { nodes, .. }, vars) = self
            .read(who, move |r, w| r.view_with_variables(w, &name))
            .await?;
        // Copying and checking the file is work in proportion to it: on the blocking pool, under its limits.
        let (f, actor, k, frame) = (file.clone(), who.actor.clone(), key.clone(), id.clone());
        let built = self
            .shared
            .blocking(who, move || {
                let mut n = 0;
                meldframe_variant::variant_of(
                    &nodes,
                    &vars,
                    &frame,
                    || {
                        n += 1;
                        fresh_id(&f, &actor, &k, &frame, n - 1)
                    },
                    &Limits::default(),
                )
            })
            .await
            .map_err(|r| failed(&r))?;
        let v = built.map_err(|e| {
            let mut v = error(e.code(), e.to_string(), false);
            v["error"]["nodeId"] = json!(id);
            v
        })?;
        if v.commands.len() > MAX_BATCH {
            return Err(invalid(format!(
                "the copy of {id} needs {} commands and a call takes at most {MAX_BATCH}: duplicate a \
                 smaller frame, or build the copy with apply_commands in several calls",
                v.commands.len()
            )));
        }
        let ops: Vec<Op> = v
            .commands
            .iter()
            .enumerate()
            .map(|(i, c)| Op::new(format!("{key}:{i}"), c.clone()))
            .collect();
        let count = ops.len();
        let reply = self
            .shared
            .submit_ops(who.clone(), TOOL.into(), file.clone(), ops, Some(vec![]))
            .await;
        if reply.status.as_u16() != 200 {
            // A retry that met the call still in flight: the copy it built differs from the one now written,
            // so the refusal is a replay when the root of this call is in the file.
            if let Some(done) = self.replay(who, &call).await.ok().flatten() {
                return Ok(done);
            }
            return Err(failed(&reply));
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
        // Ops that were already written (a call that timed out but wrote) come back with `written: false`.
        let replayed = !done.is_empty() && done.iter().all(|d| d["written"] == json!(false));
        Ok(Call {
            root: v.root,
            ..call
        }
        .result(count, seq, replayed))
    }
}

/// The call being answered: the file, the key and the id of the copy's root.
struct Call<'a> {
    file: &'a str,
    key: &'a str,
    root: String,
}

impl Call<'_> {
    fn result(&self, nodes: usize, seq: u64, replayed: bool) -> Value {
        json!({"ok": true, "file": self.file, "idempotencyKey": self.key, "root": self.root,
            "nodes": nodes, "seq": seq, "replayed": replayed})
    }
}
