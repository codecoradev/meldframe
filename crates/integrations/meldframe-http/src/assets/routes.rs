//! The asset routes of one server and the MCP tool beside them, built over one shared state.

use super::list;
use super::options::{OpsStorage, Options};
use super::state::State_;
use super::tool;
use super::transfer::{download, upload};
use crate::{Shared, Slots};
use assets::{BlobStore, Usage};
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

/// The asset routes of one server, with the default options and no storage limit (the assets are counted).
pub fn routes(shared: Arc<Shared>, blobs: Arc<dyn BlobStore>) -> Router {
    let usage = Usage::new(blobs.clone());
    routes_with(
        shared,
        blobs,
        usage,
        Arc::new(|_| (0, None)),
        Options::default(),
    )
}

/// The asset routes of one server. The store is shared by every call: it takes `&self`, so a slow write of one
/// image never makes another call wait. An upload that is not stored yet counts against the storage limit of the
/// workspace together with its ops (`usage` holds the bytes of its assets; give the same `usage` to the gate with
/// `Server::with_extra_storage`), and is refused with `quota_storage` when it does not fit.
pub fn routes_with(
    shared: Arc<Shared>,
    blobs: Arc<dyn BlobStore>,
    usage: Usage,
    ops_storage: OpsStorage,
    options: Options,
) -> Router {
    Assets::new(shared, blobs, usage, ops_storage, options).routes()
}

/// The assets of one server: the routes and the MCP tool, which share the store, the count and the limits.
#[derive(Clone)]
pub struct Assets {
    state: Arc<State_>,
}

impl std::fmt::Debug for Assets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Assets").finish_non_exhaustive()
    }
}

impl Assets {
    /// See [`routes_with`] for what the arguments mean.
    pub fn new(
        shared: Arc<Shared>,
        blobs: Arc<dyn BlobStore>,
        usage: Usage,
        ops_storage: OpsStorage,
        options: Options,
    ) -> Self {
        Assets {
            state: Arc::new(State_ {
                shared,
                blobs,
                usage,
                ops_storage,
                uploads: Slots::new(options.uploads, options.uploads_per_principal),
                upload_timeout: options.upload_timeout,
            }),
        }
    }

    /// `POST /v1/assets`, `GET /v1/assets` and `GET /v1/assets/{id}`, to merge into the router.
    pub fn routes(&self) -> Router {
        Router::new()
            .route("/v1/assets", post(upload).get(list::list))
            .route("/v1/assets/{id}", get(download))
            .with_state(self.state.clone())
    }

    /// The MCP tool `upload_asset`, for `McpConfig::extra`. `policy` is the registry's policy for the tool: only
    /// `Some(Policy::Auto)` lets a call run.
    pub fn tool(&self, policy: Option<meldframe_module::Policy>) -> Arc<dyn crate::mcp::Extra> {
        Arc::new(tool::UploadTool {
            state: self.state.clone(),
            policy,
        })
    }
}
