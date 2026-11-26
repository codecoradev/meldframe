//! Capture error codes.

/// Every code a capture can fail with, as documented in `docs/error-codes.md`.
pub const CODES: [&str; 12] = [
    "no_file_selected",
    "room_mismatch",
    "no_capable_client",
    "invalid",
    "timeout",
    "too_large",
    "node_not_found",
    "stale_client",
    "render_failed",
    "client_disconnected",
    "busy",
    "forbidden",
];

/// Why a capture did not produce an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    /// The session is bound to no file and the call named none.
    NoFileSelected,
    /// The call named a file other than the one bound to the session.
    RoomMismatch,
    /// No client with the capture capability has this file open.
    NoCapableClient,
    /// The request is malformed (bad scale, request id already pending).
    Invalid(String),
    /// No answer before the deadline.
    Timeout,
    /// The image is over the size limit.
    TooLarge,
    /// The client has no such node.
    NodeNotFound,
    /// The client had not caught up to the file's sequence number.
    StaleClient,
    /// The client could not render, or sent something that is not a PNG.
    RenderFailed(String),
    /// The client went away before it answered.
    ClientDisconnected,
    /// Too many captures or capable clients at once; retry later.
    Busy,
    /// The policy for the capture tool does not allow the call.
    Forbidden,
}

impl CaptureError {
    /// The stable machine code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoFileSelected => "no_file_selected",
            Self::RoomMismatch => "room_mismatch",
            Self::NoCapableClient => "no_capable_client",
            Self::Invalid(_) => "invalid",
            Self::Timeout => "timeout",
            Self::TooLarge => "too_large",
            Self::NodeNotFound => "node_not_found",
            Self::StaleClient => "stale_client",
            Self::RenderFailed(_) => "render_failed",
            Self::ClientDisconnected => "client_disconnected",
            Self::Busy => "busy",
            Self::Forbidden => "forbidden",
        }
    }

    /// The error a client's `CaptureFailed` code stands for; an unknown code is `render_failed`.
    pub(crate) fn from_client(code: &str, message: &str) -> Self {
        match code {
            "node_not_found" => Self::NodeNotFound,
            "too_large" => Self::TooLarge,
            "stale_client" => Self::StaleClient,
            _ => Self::RenderFailed(clean(message)),
        }
    }
}

/// A client message shown to an agent: every hidden character removed (the display profile of `meldframe_text`, which
/// keeps an emoji whole), at most 200 bytes.
fn clean(message: &str) -> String {
    let shown = meldframe_text::sanitize(message, meldframe_text::Profile::Display);
    meldframe_text::clip(&shown, usize::MAX, 200).to_owned()
}
