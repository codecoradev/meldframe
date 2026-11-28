//! The Export dialog (D9), the parts that read no `App`: the frame an export acts on, and the HTML built for it.
//!
//! The
//! export is the `export` feature's, run on the person's own view of the file (pending ops included, the modes on
//! view). Text is measured by the measurer the caller gives: the app layer gives the one its host injected (the browser's
//! Pixi metrics on the font the canvas draws with), so a hug text exports the width the canvas draws, and the layout
//! core's stub when the host gave none. The MCP tool `export_frame` runs on the server, which has no fonts: it keeps the stub.

mod cache;
mod frame;
mod name;
mod variables;

pub use cache::Cache;
pub(crate) use frame::MAX_MESSAGE;
pub use frame::{dialog, target};
pub use name::file_stem;
pub use variables::variables_dialog;
