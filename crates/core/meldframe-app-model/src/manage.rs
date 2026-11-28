//! The state of the Variables manage view (audit finding T5, issue 322) as plain data: which collection is shown, the
//! removal being asked about, the field a name is being edited in, the text of the three forms that add something, and
//! the limits the view says. Every rule that decides these is in `meldframe-app` (`parts::manage_names`, `manage_ui`);
//! the front end draws this and sends `Action::Manage`. The words are the front end's:
//! this holds codes, names and counts.

use crate::event::VariableEntity;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A collection, a mode or a variable, by id (a mode with the collection that holds it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ManageTarget {
    Collection { id: String },
    Mode { collection: String, id: String },
    Variable { id: String },
}

impl ManageTarget {
    /// What the target is.
    pub fn entity(&self) -> VariableEntity {
        match self {
            ManageTarget::Collection { .. } => VariableEntity::Collection,
            ManageTarget::Mode { .. } => VariableEntity::Mode,
            ManageTarget::Variable { .. } => VariableEntity::Variable,
        }
    }
}

/// Why a name cannot be sent. The front end words each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum NameError {
    /// Nothing, or only spaces.
    Empty,
    /// A control, direction, zero-width or other invisible character, or a space that is not the plain one.
    Hidden,
    /// Longer than `ManageLimits::name_max` (`mode_name_max` for a mode).
    TooLong,
    /// A collection or a variable is lowercase segments of letters, digits and single hyphens joined by `/`.
    NotPath,
    /// A variable of the collection has this name (the core refuses it).
    Duplicate,
}

/// A name the core takes but that the person probably did not mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum NameWarning {
    /// A collection of the file or a mode of the collection has this name already (the ids differ, so both are kept).
    Same,
}

/// What a typed name comes to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NameCheck {
    /// The name to send: the text without the spaces around it.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<NameError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub warning: Option<NameWarning>,
}

/// The text of a form that adds something, and what it comes to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NameDraft {
    pub text: String,
    pub check: NameCheck,
}

/// The three forms: a new collection, a new mode of the shown collection, a new variable of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManageDrafts {
    pub collection: NameDraft,
    pub mode: NameDraft,
    pub variable: NameDraft,
}

/// The question before a removal, with what the front end says about it. It follows the file: the numbers change when
/// another actor adds or removes a use, and it ends when its target is gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Removal {
    pub target: ManageTarget,
    /// The name of the target, made safe to show.
    pub name: String,
    /// Whether confirming lets go of what uses the target first (`RemoveVariable` and `RemoveCollection` with
    /// `detach`): a variable that something uses, a collection that holds variables. A variable nothing uses, an empty
    /// collection and a mode are removed plainly, so a use that appears meanwhile is refused (`E_VARIABLE_IN_USE`), not
    /// detached unasked.
    pub detach: bool,
    /// The uses of the target (a collection: of all its variables, aliases among them too): node fields and aliases.
    /// Counted on the whole file, whatever the search of the panel shows.
    pub uses: usize,
    /// The variables of a collection (0 for the others).
    pub variables: usize,
}

/// The field a name is being edited in, with what it comes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameField {
    pub target: ManageTarget,
    /// The name as the field opened: what the text is compared with, so an untouched field never undoes another actor's
    /// rename.
    pub opened: String,
    /// The name it has now, made safe to show (another actor may have renamed it since the field opened).
    pub current: String,
    pub text: String,
    pub check: NameCheck,
    /// The text is no change (it is the name the thing has now or the one the field opened with): saving closes the field
    /// and sends nothing.
    pub unchanged: bool,
    /// The target is the first (default) mode of its collection, which the export does not name.
    pub first: bool,
}

/// The ceilings the view says: the core's limits and the longest name of a mode this view accepts. `default()` reads the
/// core's own `meldframe_check::Limits`, so the numbers cannot drift from the ones the core enforces; it is data, and the
/// crate holds no rule that uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManageLimits {
    pub name_max: usize,
    pub mode_name_max: usize,
    pub collections: usize,
    pub modes: usize,
    pub variables: usize,
}

/// The longest name of a mode this view accepts: the core takes up to `max_name`, a mode is a short label.
pub const MODE_NAME_MAX: usize = 64;

impl Default for ManageLimits {
    fn default() -> Self {
        let l = meldframe_check::Limits::default();
        ManageLimits {
            name_max: l.max_name,
            mode_name_max: MODE_NAME_MAX,
            collections: l.max_collections,
            modes: l.max_modes,
            variables: l.max_variables,
        }
    }
}

/// The Variables manage view: view state with its rules (`Action::Manage`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManageState {
    /// Whether the view is open (`ManageOp::Open` and `Close`).
    #[serde(default)]
    pub open: bool,
    pub limits: ManageLimits,
    /// The collection the person chose; `None` (or one that is gone) shows the first. A front end reads `shown`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub chosen: Option<String>,
    /// The collection shown: the one chosen, else the first of the file; `None` when the file has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub removal: Option<Removal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rename: Option<RenameField>,
    pub drafts: ManageDrafts,
}

/// What a person does in the manage view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ManageOp {
    /// Open the view, with the search of the Variables panel cleared (the view draws the rows of the panel). Nothing, and no
    /// second clearing, when it is open already.
    Open {},
    /// Show a collection. A question or a field that was open ends. An unknown id is refused as `E_COLLECTION_NOT_FOUND`.
    Choose { id: String },
    /// The text of a form that adds something changed; its check follows. Adding is `CreateCollection`, `AddMode` and
    /// `AddVariable` with the name of the check; the draft is emptied once the thing was made.
    SetDraft { form: VariableEntity, text: String },
    /// Ask whether to remove something (`ManageState::removal`). The rename field closes. Refused as
    /// `E_VARIABLE_NOT_FOUND`, `E_COLLECTION_NOT_FOUND` or `E_MODE_NOT_FOUND` for a target that is not there.
    AskRemove { target: ManageTarget },
    /// The question closes and nothing is removed.
    CancelRemove {},
    /// The question closes and the removal goes ahead as `RemoveVariable`, `RemoveCollection` or `RemoveMode` (one undo
    /// step), with `detach` as the question says. Nothing without a question.
    ConfirmRemove {},
    /// Open the rename field on a target, with its name. The question closes. Refused like `AskRemove`.
    StartRename { target: ManageTarget },
    /// The text of the rename field changed; its check follows. Nothing without a field.
    SetRenameText { text: String },
    /// Save the name: closes the field when the name is no change, sends one rename (one undo step) and closes it when the
    /// name can be sent, and keeps the field, with its text, when it cannot (`E_` code of the refusal, `Event::Refused`).
    /// `Event::Renamed` says a rename was made. Nothing without a field.
    SubmitRename {},
    /// Close the rename field.
    CancelRename {},
    /// The view closed: the question, the rename field, the three forms and the choice of a collection are forgotten, so
    /// what was open before does not come back, and a field left open is not refused when its target goes. Nothing is
    /// sent and nothing is refused.
    Close {},
}

impl ManageOp {
    /// Whether the op changes the view only (no op is sent, no undo step).
    pub fn is_view(&self) -> bool {
        !matches!(self, ManageOp::ConfirmRemove {} | ManageOp::SubmitRename {})
    }
}
