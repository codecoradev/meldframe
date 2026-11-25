//! The MCP tool `design_review` (E5.4): a numbered critique list for a file, with each item linked to the batches
//! that address it. The list is kept by a [`ReviewStore`], never in the file's op log, so none of the actions writes
//! an op.
//!
//! The workspace and the actor come from the token of the request, never from the arguments (an unknown argument is
//! refused). A list is read under the Read tier's policy and written under the Change tier's. A batch is linked
//! only when the caller wrote it: the tool looks for the op `{batch}:0` of the caller's own in the file, the first
//! op of an `apply_commands` call with that idempotency key, which is also the name `revert_batch` uses.

use design_review::{
    DESCRIPTION, Item, MAX_ADD, MAX_ITEMS, MAX_TEXT, ReviewError, ReviewStore, Status, TOOL,
    check_batch, check_item,
};
use meldframe_host::Principal;
use meldframe_http::mcp::{Extra, ExtraCall, Reader};
use meldframe_http::{Failure, Shared, valid_file};
use meldframe_mcp_defs::MAX_KEY;
use meldframe_module::Policy;
use rmcp::model::{CallToolResult, Tool, ToolAnnotations};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    json!({"type": "object", "required": ["file", "action"], "additionalProperties": false, "properties": {
        "file": {"type": "string", "minLength": 1, "maxLength": 128,
            "description": "The name of the file whose list this is: 1 to 128 bytes (UTF-8), no slashes, control or invisible formatting characters. The list belongs to the workspace of your token."},
        "action": {"enum": ["add", "list", "link", "mark"],
            "description": "add: add items; list: read them; link: tie an item to a batch; mark: set an item's status."},
        "items": {"type": "array", "minItems": 1, "maxItems": MAX_ADD,
            "items": {"type": "string", "minLength": 1, "maxLength": MAX_TEXT},
            "description": "For add: 1 to 20 plain texts of at most 500 characters, one critique each."},
        "item": {"type": "integer", "minimum": 1,
            "description": "For link and mark: the number of the item, as list shows it."},
        "batch": {"type": "string", "minLength": 1, "maxLength": MAX_KEY,
            "description": "For link: the idempotencyKey of your apply_commands call that addressed the item."},
        "status": {"enum": ["open", "addressed"], "description": "For mark: the new status."}
    }})
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Add,
    List,
    Link,
    Mark,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    file: String,
    action: Action,
    items: Option<Vec<String>>,
    item: Option<u32>,
    batch: Option<String>,
    status: Option<String>,
}

/// A failed call: the code, the words and whether the same call can work later.
struct Fail(&'static str, String, bool);

impl From<ReviewError> for Fail {
    fn from(e: ReviewError) -> Self {
        // The text of a store failure is for the log, not for the caller.
        if let ReviewError::Unavailable(why) = &e {
            eprintln!("meldframe-http-review: unavailable: {why}");
        }
        Fail(e.code(), e.to_string(), e.retryable())
    }
}

fn invalid(why: impl Into<String>) -> Fail {
    ReviewError::Invalid(why.into()).into()
}

fn unavailable() -> Fail {
    Fail(
        "unavailable",
        "the server cannot take this call now; try again later".into(),
        true,
    )
}

fn view(i: &Item) -> Value {
    json!({"n": i.n, "text": i.text, "by": i.by, "status": i.status.as_str(), "batches": i.batches})
}

fn listing(file: &str, items: &[Item]) -> Value {
    let open = items.iter().filter(|i| i.status == Status::Open).count();
    json!({"ok": true, "file": file, "total": items.len(), "open": open,
        "limit": MAX_ITEMS, "items": items.iter().map(view).collect::<Vec<_>>()})
}

pub(crate) struct DesignReview {
    shared: Arc<Shared>,
    reader: Arc<dyn Reader>,
    store: Arc<dyn ReviewStore>,
    read: Option<Policy>,
    change: Option<Policy>,
}

impl std::fmt::Debug for DesignReview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesignReview")
            .field("read", &self.read)
            .field("change", &self.change)
            .finish_non_exhaustive()
    }
}

/// The tool, for `McpConfig::extra`. `read` and `change` are the policies of the two tiers it serves (a list is a
/// Read, add, link and mark are Changes): only `Some(Policy::Auto)` lets a call of that tier run. `reader` finds the
/// batches a caller wrote, `shared` runs the work under the slots and the time limit of every other call.
pub fn tool(
    shared: Arc<Shared>,
    reader: Arc<dyn Reader>,
    store: Arc<dyn ReviewStore>,
    read: Option<Policy>,
    change: Option<Policy>,
) -> Arc<dyn Extra> {
    Arc::new(DesignReview {
        shared,
        reader,
        store,
        read,
        change,
    })
}

/// An argument an action does not take is refused, so a call that means something else does not pass quietly.
fn takes_only(a: &Args, items: bool, item: bool, batch: bool, status: bool) -> Result<(), Fail> {
    for (given, used, name) in [
        (a.items.is_some(), items, "items"),
        (a.item.is_some(), item, "item"),
        (a.batch.is_some(), batch, "batch"),
        (a.status.is_some(), status, "status"),
    ] {
        if given && !used {
            return Err(invalid(format!("this action does not take {name}")));
        }
    }
    Ok(())
}

fn need<T>(v: Option<T>, name: &str, action: &str) -> Result<T, Fail> {
    v.ok_or_else(|| invalid(format!("{action} needs {name}")))
}

impl DesignReview {
    /// Whether the policy of the tier lets the call run.
    fn allowed(&self, a: &Action) -> bool {
        let policy = if matches!(a, Action::List) {
            self.read
        } else {
            self.change
        };
        policy == Some(Policy::Auto)
    }

    async fn run(&self, who: Principal, a: Map<String, Value>) -> Result<Value, Fail> {
        let args: Args = serde_json::from_value(Value::Object(a)).map_err(|e| {
            Fail(
                "bad_request",
                format!("the arguments do not fit the tool's input schema: {e}"),
                false,
            )
        })?;
        if !self.allowed(&args.action) {
            return Err(Fail(
                "forbidden",
                "the policy does not allow this call".into(),
                false,
            ));
        }
        if !valid_file(&args.file) {
            return Err(invalid(
                "a file name is 1 to 128 bytes without slashes, control or invisible formatting characters",
            ));
        }
        let (store, reader, file) = (self.store.clone(), self.reader.clone(), args.file.clone());
        let ws = who.workspace.clone();
        let job: Box<dyn FnOnce() -> Result<Value, Fail> + Send> = match args.action {
            Action::Add => {
                takes_only(&args, true, false, false, false)?;
                let items = need(args.items, "items", "add")?;
                let actor = who.actor.clone();
                Box::new(move || {
                    let added = store.add(&ws, &file, &actor, &items)?;
                    let total = store.list(&ws, &file)?.len();
                    Ok(json!({"ok": true, "file": file, "total": total,
                        "items": added.iter().map(view).collect::<Vec<_>>()}))
                })
            }
            Action::List => {
                takes_only(&args, false, false, false, false)?;
                Box::new(move || Ok(listing(&file, &store.list(&ws, &file)?)))
            }
            Action::Link => {
                takes_only(&args, false, true, true, false)?;
                let (n, batch) = (
                    need(args.item, "item", "link")?,
                    need(args.batch, "batch", "link")?,
                );
                check_item(n)?;
                check_batch(&batch)?;
                let who = who.clone();
                Box::new(move || {
                    let key = format!("{batch}:0");
                    let written = reader.applied(&who, &file, &[key]).map_err(|f| match f {
                        Failure::Refused(r) => Fail(r.code(), r.to_string(), r.retryable()),
                        _ => unavailable(),
                    })?;
                    if written.first().copied().flatten().is_none() {
                        return Err(ReviewError::BatchNotFound.into());
                    }
                    Ok(
                        json!({"ok": true, "file": file, "item": view(&store.link(&ws, &file, n, &batch)?)}),
                    )
                })
            }
            Action::Mark => {
                takes_only(&args, false, true, false, true)?;
                let n = need(args.item, "item", "mark")?;
                check_item(n)?;
                let name = need(args.status, "status", "mark")?;
                let status =
                    Status::parse(&name).ok_or_else(|| invalid("status is open or addressed"))?;
                Box::new(move || {
                    Ok(
                        json!({"ok": true, "file": file, "item": view(&store.mark(&ws, &file, n, status)?)}),
                    )
                })
            }
        };
        self.shared
            .blocking(&who, job)
            .await
            .map_err(|_| unavailable())?
    }
}

impl Extra for DesignReview {
    fn tool(&self) -> Tool {
        let schema = match input_schema() {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        Tool::new(TOOL, DESCRIPTION, schema).annotate(
            ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(false),
        )
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(async move {
            match self.run(who, args).await {
                Ok(v) => CallToolResult::structured(v),
                Err(Fail(code, message, retryable)) => CallToolResult::structured_error(
                    json!({"ok": false, "error": {"code": code, "message": message, "retryable": retryable}}),
                ),
            }
        })
    }
}
