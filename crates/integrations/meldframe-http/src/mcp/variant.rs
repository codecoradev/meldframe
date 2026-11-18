//! The `make_variant` tool: a copy of a frame and its subtree beside it (E5.4). The commands come from
//! `meldframe-variant`; the call ends where `apply_commands` does, in one atomic `submit_ops`, so the gate, the
//! limits and the log see it as the batch of `addNode`s it is.
//!
//! A retry must find what the first call wrote. The ids of the copy come from the file, the actor, the key and the
//! frame (FNV-1a, 64 bits), so the same call makes the same ids; a key that was used for another frame, or by
//! `apply_commands`, gives other ids and is told apart: a retry is a replay only when the root it names is in the
//! file.

use super::tools::{Body, args, check_file, check_key, error, failed, invalid, one_of};
use super::{FileView, Handler};
use axum::http::StatusCode;
use meldframe_check::Limits;
use meldframe_host::{Op, Principal};
use meldframe_mcp_defs::MAX_BATCH;
use serde::Deserialize;
use serde_json::{Map, Value, json};

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

impl Handler {
    /// How many commands of `key` the file holds, and the highest seq among them.
    async fn written(&self, who: &Principal, file: &str, key: &str) -> Result<(usize, u64), Value> {
        let (f, keys) = (
            file.to_owned(),
            (0..MAX_BATCH)
                .map(|i| format!("{key}:{i}"))
                .collect::<Vec<_>>(),
        );
        let found = self.read(who, move |r, w| r.applied(w, &f, &keys)).await?;
        let seqs: Vec<u64> = found.iter().map_while(|s| *s).collect();
        Ok((seqs.len(), seqs.iter().copied().max().unwrap_or(0)))
    }

    /// The answer of a retry whose keys are written: the copy of this call when its root is in the file, else the
    /// key belongs to another call.
    async fn replay(&self, who: &Principal, c: &Call<'_>) -> Result<Option<Value>, Value> {
        let (n, seq) = self.written(who, c.file, c.key).await?;
        if n == 0 {
            return Ok(None);
        }
        let name = c.file.to_owned();
        let f = self.read(who, move |r, w| r.view(w, &name)).await?;
        if f.nodes.iter().any(|x| x.id == c.root) {
            return Ok(Some(c.result(n, seq, true)));
        }
        Err(invalid(
            "this idempotencyKey was already used by another call (another frame, or apply_commands): use a new key",
        ))
    }

    pub(super) async fn make_variant(&self, who: &Principal, a: Map<String, Value>) -> Body {
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
        let name = file.clone();
        let FileView { nodes, .. } = self.read(who, move |r, w| r.view(w, &name)).await?;
        // Copying and checking the file is work in proportion to it: on the blocking pool, under its limits.
        let (f, actor, k, frame) = (file.clone(), who.actor.clone(), key.clone(), id.clone());
        let built = self
            .shared
            .blocking(who, move || {
                let mut n = 0;
                meldframe_variant::variant_of(
                    &nodes,
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
            .submit_ops(
                who.clone(),
                "make_variant".into(),
                file.clone(),
                ops,
                Some(vec![]),
            )
            .await;
        if reply.status != StatusCode::OK {
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
