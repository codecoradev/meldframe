//! What the server decides around the asset routes.

use std::sync::Arc;
use std::time::Duration;

/// What the server decides around the routes.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Uploads being read at once, in all.
    pub uploads: usize,
    /// The same, per principal.
    pub uploads_per_principal: usize,
    /// The longest an upload may take to arrive: an image of 10 MiB needs far longer than a JSON call.
    pub upload_timeout: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            uploads: 8,
            uploads_per_principal: 2,
            upload_timeout: Duration::from_secs(60),
        }
    }
}

/// What a workspace has written besides its assets and the storage limit it is held to: `(bytes of ops, limit)`.
/// The server answers it from its gate.
pub type OpsStorage = Arc<dyn Fn(&str) -> (u64, Option<u64>) + Send + Sync>;
