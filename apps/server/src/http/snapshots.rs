//! The snapshot of a file for the transport: the room read under the lock, opened for the call when it is closed.

use super::{SharedServer, failure_of};
use meldframe_host::{Principal, RoomError};
use meldframe_http::Failure;
use meldframe_http_snapshot::{AttributionCache, Read, SCAN_WINDOW, Snapshots, snapshot_cached};
use meldframe_rooms::RegistryError;

/// The token decides the workspace, so a principal never reads another workspace's file.
impl Snapshots for SharedServer {
    fn snapshot(
        &self,
        who: &Principal,
        file: &str,
        cache: &AttributionCache,
    ) -> Result<Option<Read>, Failure> {
        self.with_open_room(who, file, |r| snapshot_cached(r, SCAN_WINDOW, cache))?
            .map_err(|e| failure_of(&RegistryError::Room(RoomError::Store(e)).into()))
    }
}
