//! What the transport is told about a failed submit.

use crate::{PERMANENT_STORE_ERROR, SubmitError};
use meldframe_host::{Refusal, RoomError, StoreError};
use meldframe_http::Failure;
use meldframe_rooms::RegistryError;

/// What the transport is told about a failed submit: a refusal, a store error that can never succeed (the
/// text starts with [`PERMANENT_STORE_ERROR`], which only `PgStore` writes), or anything else as unavailable.
pub fn failure_of(e: &SubmitError) -> Failure {
    if let Some(r) = e.refusal() {
        return Failure::Refused(r.clone());
    }
    match e {
        SubmitError::Approval(a) => Failure::Approval {
            code: a.code(),
            id: a.id().unwrap_or_default().to_string(),
            message: a.to_string(),
        },
        SubmitError::Rooms(RegistryError::Room(RoomError::Store(StoreError::Unavailable(m))))
            if m.starts_with(PERMANENT_STORE_ERROR) =>
        {
            Failure::Permanent(e.to_string())
        }
        SubmitError::Rooms(RegistryError::InvalidName { reason }) => {
            Failure::Refused(Refusal::Invalid {
                reason: reason.clone(),
            })
        }
        // A stored file that fails the load check is the operator's to repair, never the caller's: 503
        // `unavailable`, with the stable code in the text for the log (the caller is not told).
        SubmitError::Rooms(RegistryError::Room(RoomError::BadSnapshot { error, .. })) => {
            Failure::Unavailable(format!("{}: {e}", error.code()))
        }
        _ => Failure::Unavailable(e.to_string()),
    }
}
