//! Server composition root. It assembles the registry from the compiled-in feature modules and builds the
//! room registry (one room per file) over an in-memory or Postgres backing. HTTP, MCP, REST and WebSocket
//! transports are the `meldframe-http` adapter over [`SharedServer`]; the binary only assembles and holds no
//! business logic. [`PgStore`] is the Postgres adapter of the store seam; [`Backing`] picks it.

mod approval;
mod calls;
mod clock;
mod config;
mod error;
mod http;
mod limits;
mod live;
mod modules;
mod owners;
mod server;
mod submit;
mod tools;

pub use approval::APPROVAL_TTL_MS;
pub use config::{IDLE_AFTER_MS, MAX_CALL_LOG, MAX_OPEN_ROOMS};
pub use error::SubmitError;
pub use http::{SharedServer, failure_of};
pub use limits::{
    MAX_BATCH_OPS, MAX_NAME_BYTES, MAX_OPS_PER_MINUTE, MAX_ROOMS_PER_WORKSPACE, MAX_STORAGE_BYTES,
};
pub use live::{
    Frame, LIVE_CONFIG, MAX_LIVE_STREAMS, StreamId, SubscribeError, Subscription, TOUCH_WINDOW_MS,
};
pub use meldframe_approvals::AnswerError;
pub use meldframe_store_pg::{
    AnyStore, Backing, PERMANENT_STORE_ERROR, PgCallLog, PgStore, RAW_WORKSPACE, Usage, room_key,
    split_room_key,
};
pub use meldframe_stream::{Config, StreamError};
pub use server::Server;

pub(crate) use server::Rooms;
