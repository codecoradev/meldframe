//! The write of a call: the commands as one atomic batch through the gate, and the answer of the batch.

use super::tool::{Body, HtmlImport};
use html_import::TOOL;
use html_import::map::Imported;
use meldframe_activity::{BatchCall, Note};
use meldframe_host::{Op, Principal};
use serde_json::{Value, json};

/// What a write needs: the file, the key and digest of the call, its note and what the importer made.
pub(super) struct Write<'a> {
    pub(super) file: &'a str,
    pub(super) key: &'a str,
    pub(super) digest: &'a str,
    pub(super) note: Option<Note>,
    pub(super) imported: &'a Imported,
}

impl HtmlImport {
    pub(super) async fn write(&self, who: &Principal, w: Write<'_>, mut out: Value) -> Body {
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
            tool: TOOL,
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
