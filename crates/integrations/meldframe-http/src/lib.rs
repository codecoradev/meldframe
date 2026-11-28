//! A thin axum adapter over the admission gate: REST (`POST /v1/files/{file}/tools/{tool}`) and WebSocket
//! (`GET /v1/ws`) both end in one handler, `Shared::submit`, which calls [`Submitter::submit`] and nothing
//! else. The adapter holds no policy: it authenticates, rate-limits, bounds size and time, parses, and maps
//! the answer to a stable code and an HTTP status.
//!
//! Order of work for every call: client rate limit, the token, the principal's rate limit, and only then the
//! body. The submitter blocks on its store, so each call runs on `spawn_blocking` under a timeout.

mod auth;
mod config;
mod conn;
pub mod live;
#[cfg(feature = "mcp")]
pub mod mcp;
mod names;
mod reply;
mod rest;
mod routing;
mod shared;
mod slots;
pub mod snapshot;
mod submit;
mod submitter;
mod wire;
mod ws;

pub use auth::{
    Authenticator, DevTokens, DevTokensRefused, MIN_TOKEN_LEN, SpecError, check_dev_listen,
    check_dev_tokens_allowed, dev_tokens_warning,
};
pub use config::Config;
pub use conn::{CappedListener, real_client, serve};
pub use meldframe_batch::Guard;
pub use meldframe_ratelimit::{Clock, Rate, RateLimiter};
pub use names::{client_key, valid_file};
pub use reply::{Failure, Reply};
pub use rest::{bearer, peer};
#[cfg(feature = "mcp")]
pub use routing::router_with_mcp_config;
pub use routing::{router, routes};
pub use shared::{Shared, WriteHook, shared};
pub use slots::{Slot, Slots};
pub use submitter::Submitter;
