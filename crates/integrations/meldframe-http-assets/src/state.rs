//! What the routes and the tool share: the store, the count of the bytes, the upload places and the limits.

use super::options::OpsStorage;
use assets::{AssetError, BlobStore, Reserve, Usage, check_upload};
use axum::{body::Body, extract::Request};
use meldframe_host::Principal;
use meldframe_http::{Reply, Shared, Slots, bearer, peer};
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct State_ {
    pub(crate) shared: Arc<Shared>,
    pub(super) blobs: Arc<dyn BlobStore>,
    pub(super) usage: Usage,
    pub(super) ops_storage: OpsStorage,
    pub(super) uploads: Slots,
    pub(super) upload_timeout: Duration,
}

/// Why a store call stopped: the asset is wrong, or the workspace has no room for it.
pub(crate) enum Refused {
    Asset(AssetError),
    Storage(u64),
}

impl State_ {
    /// Store the upload. A new image takes its bytes against the storage limit first, in a reservation that goes
    /// back when the write fails, panics, or finds that the image was there already (another upload wrote it first),
    /// so the same image sent twice at once is charged once. An image the workspace holds costs nothing.
    pub(crate) fn store(&self, workspace: &str, bytes: &[u8]) -> Result<assets::Stored, Refused> {
        let (_, id) = check_upload(bytes).map_err(Refused::Asset)?;
        // Held already (a file that no longer matches counts as held: its bytes were counted and the write below
        // mends it).
        if matches!(
            self.blobs.has(workspace, &id),
            Ok(true) | Err(AssetError::Corrupt)
        ) {
            return self.blobs.put(workspace, bytes).map_err(Refused::Asset);
        }
        let (ops, limit) = (self.ops_storage)(workspace);
        let reservation = self
            .usage
            .reserve(
                workspace,
                bytes.len() as u64,
                ops,
                limit.unwrap_or(u64::MAX),
            )
            .map_err(|e| match e {
                Reserve::Full(limit) => Refused::Storage(limit),
                Reserve::Unknown => {
                    Refused::Asset(AssetError::Unavailable("the asset count is unknown".into()))
                }
            })?;
        let stored = self.blobs.put(workspace, bytes).map_err(Refused::Asset)?;
        if stored.created {
            reservation.keep();
        }
        Ok(stored)
    }

    pub(super) fn who(&self, req: &Request<Body>) -> Result<Principal, Reply> {
        self.shared
            .client_ok(&peer(req))
            .and_then(|()| self.shared.principal(bearer(req.headers())))
    }
}
