//! The five tools. Each takes the caller's principal (from the token, never from the arguments) and the
//! arguments as JSON, and returns the body of its result: `Ok` for a success, `Err` for a tool error.

use super::defs::{MAX_BATCH, MAX_KEY};
use super::view;
use super::{FileView, Handler, Reader};
use crate::Guard;
use crate::reply::{Failure, Reply};
use axum::http::StatusCode;
use meldframe_batch::touches;
use meldframe_check::Limits;
use meldframe_host::{Op, Principal};
use meldframe_protocol::Command;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// Most log entries read to say who last changed each node of a tree.
const TREE_WINDOW: usize = 2_000;

/// A tool's answer: the body of a success, or the body of a tool error.
pub(super) type Body = Result<Value, Value>;

fn error(code: &str, message: impl Into<String>, retryable: bool) -> Value {
    json!({"ok": false, "error": {"code": code, "message": message.into(), "retryable": retryable}})
}

fn invalid(message: impl Into<String>) -> Value {
    error("invalid", message, false)
}

/// A refused call, as the one handler words it: the same code, message and extras as REST.
fn failed(reply: &Reply) -> Value {
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
    if crate::valid_file(file) {
        Ok(())
    } else {
        Err(invalid(crate::BAD_NAME))
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
    if key.is_empty() || key.chars().count() > MAX_KEY || key.chars().any(char::is_control) {
        return Err(invalid(format!(
            "the idempotency key is 1 to {MAX_KEY} characters without control characters"
        )));
    }
    Ok(())
}

impl Handler {
    pub(super) fn whoami(&self, who: &Principal) -> Body {
        let l = Limits::default();
        let rate = self.shared.principal_rate;
        Ok(
            json!({"ok": true, "workspace": who.workspace, "actor": who.actor,
            "batchLimit": MAX_BATCH, "maxIdempotencyKeyLength": MAX_KEY,
            "limits": {"maxNodes": l.max_nodes, "maxDepth": l.max_depth, "maxText": l.max_text,
                "maxName": l.max_name, "maxId": l.max_id, "maxShadows": l.max_shadows},
            "rate": {"burst": rate.burst, "perSecond": rate.per_second}}),
        )
    }

    pub(super) async fn get_tree(&self, who: &Principal, a: Map<String, Value>) -> Body {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct A {
            file: String,
            max_depth: Option<usize>,
        }
        let A { file, max_depth } = args(a)?;
        check_file(&file)?;
        let name = file.clone();
        let (f, recent) = self
            .read(who, move |r, w| r.tree(w, &name, TREE_WINDOW))
            .await?;
        let last = view::last_touch(&recent);
        // The layout rules of the fidelity contract first (a broken layout matters more than a default name), then layer
        // names; together at most MAX_NAME_WARNINGS.
        let layout = meldframe_layout::layout_findings(&f.nodes, &meldframe_layout::StubMeasurer);
        let mut warnings: Vec<Value> = layout
            .into_iter()
            .map(|w| json!({"code": w.code, "nodeId": w.id, "message": w.message}))
            .collect();
        warnings.extend(meldframe_check::name_warnings_of_nodes(&f.nodes));
        warnings.truncate(meldframe_check::MAX_NAME_WARNINGS);
        Ok(
            json!({"ok": true, "file": file, "seq": f.seq, "roots": view::tree(&f.nodes, &last, max_depth),
            "warnings": warnings}),
        )
    }

    pub(super) async fn get_nodes(&self, who: &Principal, a: Map<String, Value>) -> Body {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            file: String,
            ids: Vec<String>,
        }
        let A { file, ids } = args(a)?;
        check_file(&file)?;
        if ids.is_empty() || ids.len() > MAX_BATCH {
            return Err(invalid(format!(
                "ids holds 1 to {MAX_BATCH} ids; this call has {}",
                ids.len()
            )));
        }
        let f: FileView = self.read(who, move |r, w| r.view(w, &file)).await?;
        let (found, missing) = view::nodes(&f.nodes, &ids);
        Ok(json!({"ok": true, "seq": f.seq, "nodes": found, "missing": missing}))
    }

    pub(super) async fn batch_status(&self, who: &Principal, a: Map<String, Value>) -> Body {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            file: String,
            #[serde(rename = "idempotencyKey")]
            camel: Option<String>,
            idempotency_key: Option<String>,
            count: Option<usize>,
        }
        let A {
            file,
            camel,
            idempotency_key,
            count,
        } = args(a)?;
        check_file(&file)?;
        let key = one_of(camel, idempotency_key, "idempotencyKey")?
            .ok_or_else(|| invalid("send idempotencyKey or idempotency_key"))?;
        check_key(&key)?;
        let keys: Vec<String> = (0..MAX_BATCH).map(|i| format!("{key}:{i}")).collect();
        let found = self
            .read(who, move |r, w| r.applied(w, &file, &keys))
            .await?;
        let seqs: Vec<u64> = found.iter().map_while(|s| *s).collect();
        let state = match (seqs.len(), count) {
            (0, _) => "none",
            (n, Some(c)) if n < c => "partial",
            _ => "applied",
        };
        Ok(json!({"ok": true, "state": state, "applied": seqs.len(), "seqs": seqs}))
    }

    pub(super) async fn apply_commands(&self, who: &Principal, a: Map<String, Value>) -> Body {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            file: String,
            commands: Vec<Value>,
            #[serde(rename = "idempotencyKey")]
            camel: Option<String>,
            idempotency_key: Option<String>,
            #[serde(rename = "ifSeq")]
            if_seq_camel: Option<u64>,
            if_seq: Option<u64>,
            atomic: Option<bool>,
        }
        let a: A = args(a)?;
        check_file(&a.file)?;
        let key = one_of(a.camel, a.idempotency_key, "idempotencyKey")?.ok_or_else(|| {
            invalid(
                "every call needs an idempotencyKey (or idempotency_key): it makes a retry safe",
            )
        })?;
        check_key(&key)?;
        let if_seq = one_of(a.if_seq_camel, a.if_seq, "ifSeq")?;
        let n = a.commands.len();
        if n == 0 || n > MAX_BATCH {
            return Err(invalid(format!(
                "a call takes 1 to {MAX_BATCH} commands (the batch limit); this call has {n}"
            )));
        }
        let atomic = a.atomic.unwrap_or(true);
        let (ops, guards) = build_ops(&key, if_seq, a.commands.clone())?;
        if !atomic && let Some(g) = guards.first() {
            let mut v = invalid(format!(
                "command {} touches {} for the first time and a node an earlier command of the call \
                 touched; ifSeq cannot guard it unless the call is atomic. Send it in its own call or \
                 leave atomic on",
                g.index, g.node
            ));
            v["error"]["index"] = json!(g.index);
            return Err(v);
        }
        let reply = self
            .shared
            .submit_ops(
                who.clone(),
                "apply_commands".into(),
                a.file.clone(),
                ops,
                atomic.then_some(guards),
            )
            .await;
        if reply.status != StatusCode::OK {
            return Err(self.explain(who, &a.file, if_seq, &reply).await);
        }
        let done = reply.body["accepted"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let results: Vec<Value> = done
            .iter()
            .enumerate()
            .map(|(i, d)| {
                json!({"index": i, "key": format!("{key}:{i}"), "seq": d["seq"], "written": d["written"]})
            })
            .collect();
        let seq = done
            .iter()
            .filter_map(|d| d["seq"].as_u64())
            .max()
            .unwrap_or(0);
        let replayed = done.iter().all(|d| d["written"] == json!(false));
        Ok(
            json!({"ok": true, "file": a.file, "atomic": atomic, "idempotencyKey": key,
            "seq": seq, "replayed": replayed, "results": results, "warnings": meldframe_check::name_warnings_of_commands(&a.commands)}),
        )
    }

    /// The error of a refused call. A conflict also says what changed: the changes since `ifSeq` that
    /// touched the node, the node as it is now (`null` when it was removed) and the file's sequence number.
    async fn explain(
        &self,
        who: &Principal,
        file: &str,
        if_seq: Option<u64>,
        reply: &Reply,
    ) -> Value {
        let mut body = failed(reply);
        let (Some(node_id), Some(since)) = (
            reply.body["error"]["nodeId"].as_str().map(str::to_owned),
            if_seq,
        ) else {
            return body;
        };
        let f = file.to_owned();
        let id = node_id.clone();
        if let Ok(c) = self
            .read(who, move |r, w| r.conflict(w, &f, since, &id, MAX_BATCH))
            .await
        {
            body["error"]["currentSeq"] = json!(c.seq);
            body["error"]["current"] = c.node.map_or(Value::Null, |n| view::node(&n));
            body["error"]["changes"] = Value::Array(view::changes_of(&c.changes, &node_id));
        }
        body
    }

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
            Ok(Err(failure)) => Err(failed(&Reply::failure(&failure))),
            Ok(Ok(v)) => Ok(v),
        }
    }
}

/// The ops of a call: command `i` has the key `{key}:{i}`. A command whose nodes were all untouched by the
/// call so far carries `ifSeq` itself, so the room checks it on every path. A command that touches a node an
/// earlier command of the call touched and also a new one cannot carry it (the room would count the earlier
/// command as a change after `ifSeq`), so each new node gets a guard that an atomic call checks before it
/// writes anything.
fn build_ops(
    key: &str,
    if_seq: Option<u64>,
    commands: Vec<Value>,
) -> Result<(Vec<Op>, Vec<Guard>), Value> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut ops = Vec::with_capacity(commands.len());
    let mut guards = Vec::new();
    for (i, raw) in commands.into_iter().enumerate() {
        let command: Command = serde_json::from_value(raw).map_err(|e| {
            let mut v = error(
                "bad_request",
                format!("command {i} is not a command: {e}"),
                false,
            );
            v["error"]["index"] = json!(i);
            v
        })?;
        let t = touches(&command);
        let fresh: Vec<&String> = t.iter().filter(|n| !seen.contains(*n)).collect();
        let mut op = Op::new(format!("{key}:{i}"), command);
        match if_seq {
            Some(s) if fresh.len() == t.len() => op.if_seq = Some(s),
            Some(s) => guards.extend(fresh.iter().map(|n| Guard {
                index: i,
                if_seq: s,
                node: (*n).clone(),
            })),
            None => {}
        }
        seen.extend(t);
        ops.push(op);
    }
    Ok((ops, guards))
}
