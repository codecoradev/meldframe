//! What running an entry makes: the action the app dispatches, or the host switch.

use super::entries::Run;
use meldframe_app_model::{Action, AppState, Overlay, OverlayOp};

/// What a run comes to in a state.
#[allow(
    clippy::large_enum_variant,
    reason = "a short-lived value, as Action is; boxing would leak into every consumer pattern"
)]
#[derive(Debug, Clone)]
pub enum Plan {
    /// The app dispatches this action: the one the button or the key for the same thing sends.
    Dispatch(Action),
    /// Turn "Auto layout on canvas" on or off: a host setting of the app, not an action. Remembering the choice and the notice stay
    /// with the host.
    SetLayoutOnCanvas(bool),
    /// There is nothing for it to act on in this state.
    Nothing,
}

/// JavaScript's `Math.round`: halves go up, so a frame is placed where the web front end placed it.
fn round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// The plan for a run. `auto_layout` is whether the switch is on now, and `centre` the world point at the middle of the view,
/// where a new frame is centred.
pub fn plan(run: Run, state: &AppState, auto_layout: bool, centre: (f64, f64)) -> Plan {
    let target = state.export.target.as_ref();
    match run {
        Run::Tool(tool) => Plan::Dispatch(Action::SetTool { tool }),
        Run::Undo => Plan::Dispatch(Action::Undo {}),
        Run::Redo => Plan::Dispatch(Action::Redo {}),
        Run::DeleteSelection if !state.selection.is_empty() => Plan::Dispatch(Action::Delete {
            ids: state.selection.clone(),
        }),
        Run::NewFrame(preset) => {
            let (w, h) = preset.size();
            Plan::Dispatch(Action::CreatePreset {
                preset,
                x: round(centre.0 - w / 2.0),
                y: round(centre.1 - h / 2.0),
                parent: None,
            })
        }
        Run::Zoom(command) => Plan::Dispatch(Action::zoom(command)),
        Run::AutoLayout => Plan::SetLayoutOnCanvas(!auto_layout),
        Run::TogglePanel(panel) => Plan::Dispatch(Action::TogglePanel { panel }),
        Run::Export if target.is_some() => Plan::Dispatch(Action::OpenExport {}),
        Run::Variant if target.is_some() => target.map_or(Plan::Nothing, |t| {
            Plan::Dispatch(Action::MakeVariant { id: t.id.clone() })
        }),
        Run::Help => Plan::Dispatch(Action::Overlay {
            op: OverlayOp::Open {
                overlay: Overlay::Help,
            },
        }),
        Run::DeleteSelection | Run::Export | Run::Variant => Plan::Nothing,
    }
}
