//! The tool as the MCP route serves it: the checks of the call, the plan on the blocking pool, the place of the root,
//! the one atomic `Shared::submit_ops` and the answer.

use crate::answer::describe;
use crate::args::Args;
use crate::file::{dry_run, place};
use crate::plan::{Planned, make};
use crate::schema::{DESCRIPTION, TOOL, input_schema};
use crate::{failed, invalid, refuse};
use meldframe_activity::{BatchCall, Marked};
use meldframe_host::{Op, Principal};
use meldframe_http::mcp::{Extra, ExtraCall, FileView, Reader};
use meldframe_http::{Failure, Reply, Shared};
use meldframe_module::Policy;
use meldframe_protocol::Doc;
use rmcp::model::{CallToolResult, Tool, ToolAnnotations};
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// A tool's answer: the body of a success, or the body of a tool error.
type Body = Result<Value, Value>;

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

/// The tool, for `McpConfig::extra`. `policy` is what the registry says for [`TOOL`]; without the module (`None`) or
/// under any policy but Auto the tool serves nothing (the gate still decides every write). `reader` reads the file to
/// place the root; `shared` runs the work under the slots and the time limit of every other call and ends in the one
/// handler of writes.
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
            match self.run(&who, args).await {
                Ok(v) => CallToolResult::structured(v),
                Err(v) => CallToolResult::structured_error(v),
            }
        })
    }
}

impl HtmlImport {
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

    /// The header of the batch the call made, kept by the sink after the call, in a slot of the principal under the
    /// call time limit. `None`: no sink, nothing written, or the sink failed: the answer says `batch: null`.
    async fn mark(
        &self,
        who: &Principal,
        call: BatchCall,
        done: Vec<(u64, bool)>,
    ) -> Option<Marked> {
        let sink = self.shared.batches.get()?.clone();
        let w = who.clone();
        let job = move || sink.record(&w.workspace, &w.actor, &call, Some(&done));
        self.shared.blocking(who, job).await.ok().flatten()
    }

    async fn run(&self, who: &Principal, a: Map<String, Value>) -> Body {
        if self.policy != Some(Policy::Auto) {
            return Err(refuse(
                "forbidden",
                "the policy does not allow an import for this call",
            ));
        }
        let args = Arc::new(Args::parse(a)?);
        if args.note.is_some() && self.shared.batches.get().is_none() {
            return Err(invalid("this server keeps no notes: leave note out"));
        }
        let (job, actor) = (args.clone(), who.actor.clone());
        let planned = self
            .shared
            .blocking(who, move || make(&job, &actor))
            .await
            .map_err(|r| failed(&r))??;
        let (file, count) = (args.file.clone(), planned.plan.commands.len());
        // A call sent again after a timeout finds its commands in the log: every key is there, so nothing is sent to
        // the room. The root may have been moved or renamed since, so the commands built now would differ from the
        // ones written: a retry is answered from the log and not tried again.
        if !args.dry_run {
            let keys: Vec<String> = (0..count)
                .map(|i| format!("{}:{i}", planned.prefix))
                .collect();
            let name = file.clone();
            let seqs = self
                .read(who, move |r, w| r.written(w, &name, &keys))
                .await?;
            if seqs.len() == count {
                let done = seqs.iter().map(|s| (*s, false)).collect();
                return Ok(self.finish(who, &args, &planned, done).await);
            }
        }
        let name = file.clone();
        let (FileView { nodes, seq }, vars) = self
            .read(who, move |r, w| r.view_with_variables(w, &name))
            .await?;
        let mut planned = planned;
        place(&mut planned, args.parent_id.as_deref(), &nodes);
        if args.dry_run {
            // The checks of a write on a copy of the file, in the order of the commands.
            let (doc, commands) = (Doc { nodes, vars }, planned.plan.commands.clone());
            let checked = (self.shared)
                .blocking(who, move || dry_run(doc, &commands))
                .await;
            checked
                .map_err(|r| failed(&r))?
                .map_err(|r| with_source(failed(&r), &planned))?;
            let call = json!({"file": file, "idempotencyKey": args.key, "dryRun": true,
                "replayed": false, "seq": seq, "batch": null});
            return Ok(answer(&planned, call));
        }
        let ops: Vec<Op> = (planned.plan.commands.iter().enumerate())
            .map(|(i, c)| Op::new(format!("{}:{i}", planned.prefix), c.clone()))
            .collect();
        let reply = self
            .shared
            .submit_ops(who.clone(), TOOL.into(), file, ops, Some(vec![]))
            .await;
        if !reply.status.is_success() {
            return Err(with_source(failed(&reply), &planned));
        }
        let accepted = reply.body["accepted"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        // An answer with no seq for a command gets no header, never one built on a made-up number.
        let done = (accepted.iter())
            .map(|d| Some((d["seq"].as_u64()?, d["written"] == json!(true))))
            .collect::<Option<Vec<(u64, bool)>>>();
        Ok(self
            .finish(who, &args, &planned, done.unwrap_or_default())
            .await)
    }

    /// The answer of a call that wrote (or found written) its commands: `done` is the `(seq, written)` of each. It
    /// keeps the batch header and says whether every command was there already.
    async fn finish(
        &self,
        who: &Principal,
        args: &Args,
        planned: &Planned,
        done: Vec<(u64, bool)>,
    ) -> Value {
        let seq = done.iter().map(|d| d.0).max().unwrap_or(0);
        let replayed = !done.is_empty() && done.iter().all(|d| !d.1);
        let call = BatchCall {
            file: args.file.clone(),
            key: planned.prefix.clone(),
            commands: planned.plan.commands.len(),
            atomic: true,
            note: args.note.clone(),
        };
        let marked = if done.is_empty() {
            None
        } else {
            self.mark(who, call, done).await
        };
        let batch = marked.map(|m| json!({"n": m.n, "note": m.note}));
        answer(
            planned,
            json!({"file": args.file, "idempotencyKey": args.key, "dryRun": false,
                "replayed": replayed, "seq": seq, "batch": batch}),
        )
    }
}

/// The answer of a call: the plan's part and what the call adds to it.
fn answer(p: &Planned, call: Value) -> Value {
    let mut out = json!({"ok": true});
    for part in [call, describe(p)] {
        if let (Some(o), Value::Object(m)) = (out.as_object_mut(), part) {
            o.extend(m);
        }
    }
    out
}

/// The element a refused Command came from: the error of a write names the Command by `index`, and the plan knows
/// the element of each.
fn with_source(mut e: Value, p: &Planned) -> Value {
    let source = (e["error"]["index"].as_u64())
        .and_then(|i| p.plan.nodes.get(usize::try_from(i).ok()?))
        .map(|n| html_import::html::clean(&n.source, 200));
    if let (Some(s), Some(err)) = (source, e["error"].as_object_mut()) {
        err.insert("source".into(), json!(s));
    }
    e
}
