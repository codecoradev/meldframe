//! How much of the derived state one dispatch has to bring up to date, from the action alone.

use meldframe_app_model::{Action, Frame};

/// How much of the derived state one dispatch has to bring up to date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Nothing derived can have changed (a heartbeat, a timer, a repeated frame).
    Nothing,
    /// Only the selection or a panel changed.
    Ui,
    /// The document or the ops may have changed.
    Full,
}

/// An action that changes the view only (selection, tool, panels, modes, search, picker, export dialog, the image picker and its list).
pub fn is_view(action: &Action) -> bool {
    matches!(
        action,
        Action::Select { .. }
            | Action::ToggleSelect { .. }
            | Action::ClearSelection {}
            | Action::SetTool { .. }
            | Action::SetPanel { .. }
            | Action::TogglePanel { .. }
            | Action::ToggleCollapsed { .. }
            | Action::Attribute { .. }
            | Action::SetMode { .. }
            | Action::SetVariablesQuery { .. }
            | Action::OpenPicker { .. }
            | Action::ClosePicker {}
            | Action::SetPickerQuery { .. }
            | Action::OpenExport {}
            | Action::SetExportFormat { .. }
            | Action::CloseExport {}
            | Action::OpenVariablesExport {}
            | Action::CloseVariablesExport {}
            | Action::CloseVariablesImport {}
    ) || action.is_assets()
        || matches!(action, Action::Manage { op } if op.is_view())
}

/// The level of an action; `ui` is `is_view(action)`.
pub fn of(action: &Action, ui: bool) -> Level {
    match action {
        _ if ui => Level::Ui,
        a if a.only_activity() || a.is_checkpoint() => Level::Nothing,
        Action::Start { .. }
        | Action::Disconnected {}
        | Action::Reconnect { .. }
        | Action::Tick { .. }
        | Action::Frame {
            frame:
                Frame::Heartbeat {}
                | Frame::Batch { .. }
                | Frame::Touch { .. }
                | Frame::Checkpoint { .. },
            ..
        } => Level::Nothing,
        _ => Level::Full,
    }
}
