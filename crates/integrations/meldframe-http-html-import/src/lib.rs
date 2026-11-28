//! The MCP tool `import_html` (E4.4): the HTML importer over HTTP.
//!
//! The call reads the text on the blocking pool under the call's slot (a bounded parser of our own, nothing is
//! fetched or run), makes at most 100 `addNode` commands and writes them as one atomic batch with
//! `Shared::submit_ops`, the handler every transport ends in: the same gate, quota, limits and call log as
//! `apply_commands`, and one numbered batch in the Activity panel. The op of command `i` has the key
//! `{idempotencyKey}:{digest}:{i}`, where the digest is a hash of the input, so a call sent again after a timeout
//! writes nothing twice and the same key with other text is another call. The ids of the nodes come from the key and
//! the digest too (`imp-<hash>-<i>`), so the same call makes the same ids and the same text under another key makes
//! others.
//!
//! The tool is an [`Extra`] of the MCP route and [`html_import::ImportHtmlModule`] registers it, so a build without
//! the `html-import` feature of the server has neither.

mod answer;
mod check;
mod run;
mod schema;
mod tool;
mod write;

pub use schema::input_schema;
pub use tool::tool;
