//! The policies the server reads and the one `install` call that wires the activity panel.

use crate::review::REVIEW_TOOL;
use crate::reviewers::Reviewers;
use crate::service::{Activity, Publish};
use crate::tool::{TOOL, tool};
use activity_store::BatchStore;
use meldframe_http::Shared;
use meldframe_http::mcp::{Extra, Reader};
use meldframe_module::{Policy, Registry};
use std::sync::Arc;

/// What the server reads before it hands the registry over: the policies for the two calls of this crate and the
/// list of reviewers.
#[derive(Debug, Clone)]
pub struct Policies {
    /// `get_batches`, a Read.
    pub get_batches: Option<Policy>,
    /// `batch_review`, a Change.
    pub review: Option<Policy>,
    /// Who may set a review state: [`REVIEWERS_VAR`].
    pub reviewers: Reviewers,
}

/// The policies of `registry` for [`TOOL`] and [`REVIEW_TOOL`], and the reviewers of the environment. A bad list is
/// an error; the binary reads it once before it opens the store, so a bad list fails before any side effect, and
/// again here to use it.
pub fn policies(registry: &Registry) -> Result<Policies, String> {
    Ok(Policies {
        get_batches: registry.policy_for(TOOL),
        review: registry.policy_for(REVIEW_TOOL),
        reviewers: Reviewers::from_env()?,
    })
}

/// Everything the server wires for the activity panel in one call: the sink is set on `shared` (so `apply_commands`
/// keeps headers, takes a `note` and sends the frame through `publish`) and the routes come back to merge into the
/// router. `tools` is the list the MCP route serves besides its own and the registry's policies: `get_batches` is
/// added to the list when its policy is `Auto`. The review setter is a route only, never a tool of that list, and
/// only the listed reviewers may use it. Later slices join here, so the binary's `main` does not grow with them.
pub fn install(
    shared: &Arc<Shared>,
    store: Arc<dyn BatchStore>,
    reader: Arc<dyn Reader>,
    publish: Publish,
    tools: (&mut Vec<Arc<dyn Extra>>, Policies),
) -> axum::Router {
    let (list, policy) = tools;
    list.extend(tool(shared.clone(), store.clone(), policy.get_batches));
    let activity = Activity::with_live(store, reader, Some(publish));
    let activity = activity.with_review(policy.reviewers, policy.review);
    // Set once: a second install on the same `Shared` keeps the first sink.
    let _ = shared.batches.set(activity.sink());
    activity.routes(shared.clone())
}
