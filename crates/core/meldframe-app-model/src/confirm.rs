//! The tab of the right card (audit finding T10, issue 323). It is view state with a rule (choosing Activity asks for the
//! newest batches, once), so it is state of the app layer: a second front end (a headless one among them) can open the
//! Activity tab without the rule moving.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The two tabs of the right card.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum RightTab {
    /// The properties of the selection and the Variables panel.
    #[default]
    Design,
    /// The lanes of batches and the checkpoints. Changing to it asks for the newest batches (`ActivityOp::SetTab` of
    /// `Action::Activity`).
    Activity,
}

/// What a person does with the tab of the right card (`Action::Activity`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ActivityOp {
    /// Select a tab. Only a change to Activity asks for the newest batches and for the checkpoints (heal rule (c), a person's
    /// action, as `Action::OpenActivity`); the tab that is selected already asks for nothing, so a second click or a key on it
    /// does not fetch again. The fetch goes out only while the app is live; the tab changes either way.
    SetTab { tab: RightTab },
}

impl ActivityOp {
    /// Whether the op changes the view only: no op is sent, no batch is selected, the file does not change.
    pub fn is_view(&self) -> bool {
        matches!(self, ActivityOp::SetTab { .. })
    }
}
