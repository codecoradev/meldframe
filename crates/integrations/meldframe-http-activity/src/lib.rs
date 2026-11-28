//! The activity panel's server side over HTTP (E5.1, ADR-0021): the batch sink, the live frame, the list and review
//! routes and the tool `get_batches`.
//!
//! [`Activity`] is the one constructor: it holds the batch store and the reader, and hands out the [`BatchSink`] that
//! `apply_commands` calls after every call.
//!
//! A header is kept in three cases, all by [`BatchSink::record`]: a call that wrote (the normal case); a call that
//! failed after it wrote a prefix (the sink asks the reader how many of the call's keys are in the log, with
//! [`Reader::written`], which is not a `batch_status` read and writes no call-log entry); and a replay, the retry
//! after a timeout, which takes the header's `written` from the keys found and its time from the replay. A failed
//! header write never fails the call: it is logged and the answer says `batch: null`.
//!
//! The wire (slice 5): a header that was made or changed is queued as a `batch` frame on the live streams of the file,
//! after the call's op events and through the publisher given to [`Activity::with_live`] (the server sends it with
//! headroom: it never takes the last slots of a queue and so never causes a resync by itself, but it does use the
//! slack of a stalled reader); [`Activity::routes`] adds the list route, which is the
//! truth the frame is a hint of.
//!
//! The MCP tool `get_batches` (slice 6) is in `tool.rs`: [`tool`] gives it for `McpConfig::extra` and
//! [`ActivityModule`] registers it as a Read.

mod install;
mod review;
mod reviewers;
mod route;
mod service;
mod sink;
mod tool;
mod wire;

pub use install::{Policies, install, policies};
pub use review::{REVIEW_DESCRIPTION, REVIEW_TOOL};
pub use reviewers::{MAX_REVIEWERS, Reviewers, VAR as REVIEWERS_VAR};
pub use service::{Activity, Publish};
pub use tool::{ActivityModule, DESCRIPTION, NOTICE, TOOL, input_schema, tool};
