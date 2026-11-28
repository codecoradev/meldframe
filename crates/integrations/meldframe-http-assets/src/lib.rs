//! The asset routes and the `upload_asset` tool (E2.5, ADR-0015), each answering to the workspace of the Bearer token and to no other.
//!
//! - `POST /v1/assets`: the body is the bytes of one png, jpeg, webp or gif image (no multipart). The route
//!   checks the token and the declared length first, takes an upload place, reads the body as a stream and never
//!   past the limit, and stores it. The answer names the asset: `{"ok": true, "id": <SHA-256>, "type": "image/png",
//!   "bytes": n}`. The same bytes are the same asset.
//! - `GET /v1/assets`: the workspace's assets a page at a time, `{"assets": [{"id", "bytes", "mime"}], "next"}`, sorted
//!   by id (see `list.rs`); no content.
//! - `GET /v1/assets/{id}`: the bytes, served with the type told by the first bytes, `X-Content-Type-Options:
//!   nosniff` and a content security policy that allows no script, since only the first bytes of an upload are
//!   checked. An id that is not in the workspace's store is `asset_not_found`, whoever else has it.
//!
//! An error is `{"error": {"code", "message", "retryable"}}`, as on the other routes; the codes are those of
//! `docs/error-codes.md` ("Asset codes").

mod dir;
mod failure;
mod list;
mod options;
mod routes;
mod state;
mod tool;
mod transfer;

pub use dir::assets_dir;
pub use options::{OpsStorage, Options};
pub use routes::{Assets, routes, routes_with};
pub use tool::input_schema as upload_input_schema;
