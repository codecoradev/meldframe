//! The file a session is bound to.

use crate::CaptureError;

/// The file bound to one agent session. A capture uses it; it never needs a room argument.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    file: Option<String>,
}

impl Binding {
    /// Bind the session to `file` (the session layer calls this when the principal selects a file).
    /// `file` is the namespaced room string clients subscribe with. A session keeps one file: binding the
    /// same file again is fine, a different one is `room_mismatch`. There is no rebinding in this slice;
    /// a new session binds the new file.
    pub fn bind(&mut self, file: &str) -> Result<(), CaptureError> {
        match self.file.as_deref() {
            Some(bound) if bound != file => Err(CaptureError::RoomMismatch),
            _ => {
                self.file = Some(file.to_string());
                Ok(())
            }
        }
    }

    /// The file a call works on: the bound one. A call may name the file it expects; naming another is
    /// `room_mismatch`, and an unbound session with no name is `no_file_selected`.
    pub fn resolve(&self, requested: Option<&str>) -> Result<&str, CaptureError> {
        match (self.file.as_deref(), requested) {
            (None, _) => Err(CaptureError::NoFileSelected),
            (Some(bound), Some(r)) if r != bound => Err(CaptureError::RoomMismatch),
            (Some(bound), _) => Ok(bound),
        }
    }
}
