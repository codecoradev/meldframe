//! The one handler every transport ends in: validate the names, take a slot, call the submitter, tell the
//! sinks and map the answer.

use crate::names::{BAD_NAME, valid_file, valid_tool};
use crate::reply::{Failure, Reply};
use crate::shared::Shared;
use crate::wire::WireOp;
use meldframe_batch::Guard;
use meldframe_checkpoints::{RunCall, RunSink, is_agent};
use meldframe_host::{Op, Principal, Refusal};
use std::sync::Arc;

impl Shared {
    /// The shared handler of REST and WebSocket: each op is its own write.
    pub(crate) async fn submit(
        &self,
        who: Principal,
        tool: String,
        file: String,
        ops: Vec<WireOp>,
    ) -> Reply {
        let ops = ops.into_iter().map(Op::from).collect();
        self.submit_ops(who, tool, file, ops, None).await
    }

    /// The one handler every transport ends in, MCP included: validate the names, take a slot, call the
    /// submitter (`Some(guards)` picks [`crate::Submitter::submit_atomic`]) and map the answer.
    pub async fn submit_ops(
        &self,
        who: Principal,
        tool: String,
        file: String,
        ops: Vec<Op>,
        atomic: Option<Vec<Guard>>,
    ) -> Reply {
        if !valid_file(&file) || !valid_tool(&tool) {
            return Reply::failure(&Failure::Refused(Refusal::Invalid {
                reason: BAD_NAME.into(),
            }));
        }
        let submitter = self.submitter.clone();
        let (sent, woken) = (who.clone(), file.clone());
        // An agent's call keeps its keys, so a call that fails after it wrote a prefix is found by key; any other
        // call costs the prefix test.
        let run = self
            .runs
            .get()
            .filter(|_| is_agent(&who.actor))
            .map(|sink| {
                let keys = ops.iter().map(|o| o.key.clone()).collect();
                (
                    sink.clone(),
                    RunCall {
                        file: file.clone(),
                        keys,
                    },
                )
            });
        let done = self
            .blocking(&who, move || {
                if let Some(guards) = atomic {
                    submitter.submit_atomic(&tool, &file, &sent, ops, guards)
                } else {
                    submitter.submit(&tool, &file, &sent, ops)
                }
            })
            .await;
        match done {
            Err(reply) => reply,
            Ok(Ok(accepted)) => {
                if let (Some(hook), true) =
                    (self.on_write.get(), accepted.iter().any(|a| a.written))
                {
                    hook(&who, &woken);
                }
                let seen: Vec<(u64, bool)> = accepted.iter().map(|a| (a.seq, a.written)).collect();
                self.run_started(&who, run, Some(seen)).await;
                Reply::accepted(&accepted)
            }
            Ok(Err(failure)) => {
                self.run_started(&who, run, None).await;
                if let Failure::Unavailable(why) | Failure::Permanent(why) = &failure {
                    eprintln!("meldframe-http: unavailable: {why:?}");
                }
                Reply::failure(&failure)
            }
        }
    }

    /// Tell the run sink about an agent's call that ran. `done` is what a call that succeeded accepted (a run starts
    /// only when it wrote an op and the in-memory detector says so); `None` is a call that failed, which the sink
    /// looks into on its own. The store write is its own job under the caller's slot, so it never holds the call's own
    /// job, and nothing here changes the reply; the reply does wait for the job, for at most the call's time limit, as
    /// it does for the batch sink. A job that cannot run, outlasts the limit or panics is reported to the sink, which
    /// logs and counts it and gives the claim back.
    async fn run_started(
        &self,
        who: &Principal,
        run: Option<(Arc<dyn RunSink>, RunCall)>,
        done: Option<Vec<(u64, bool)>>,
    ) {
        let Some((sink, call)) = run else {
            return;
        };
        if let Some(done) = &done
            && !(done.iter().any(|d| d.1) && sink.starts(&who.workspace, &call.file, &who.actor))
        {
            return;
        }
        let (w, file, taker) = (who.clone(), call.file.clone(), sink.clone());
        let job = move || taker.take(&w.workspace, &w.actor, &call, done.as_deref());
        if let Err(reply) = self.blocking(who, job).await {
            let why = reply.body["error"]["code"].as_str().unwrap_or("internal");
            sink.lost(&who.workspace, &file, &who.actor, why);
        }
    }
}
