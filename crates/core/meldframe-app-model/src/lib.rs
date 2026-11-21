//! The plain data of the app layer: [`Action`] in, [`Effect`] and [`Event`] out, and the [`AppState`] a front
//! end reads. Every type is serialisable data with no UI toolkit (ADR-0008, standard rule U1). The state
//! machine that uses them is `meldframe-app`, which re-exports all of this; this crate only keeps that one
//! under the size ceiling. It holds few rules, each a plain function of its data. The code is conversions (`From`: what
//! `Feed::view` of `meldframe-activity` gives, a `BatchPage`), `Frame::observe` (a plain mapping of a frame to the
//! feed's calls), the classification of an action (`Action::is_activity`, `Action::only_activity`, `Action::is_assets`),
//! and the one real rule set, the transitions of the image picker's list (`AssetsState`: which answer is taken, what a
//! page has to look like, the cap on the list). The rules of the other screens are in `meldframe-app-parts` and
//! `meldframe-app-rules`.

pub mod action;
pub mod activity;
pub mod assets;
pub mod effect;
pub mod event;
pub mod export;
pub mod frame;
pub mod layout;
pub mod property;
pub mod state;
pub mod variables;

pub use action::{Action, CreateKind, Frame, FramePreset, Property};
pub use activity::{
    ActivityLane, ActivityRun, ActivityState, BatchHeader, BatchPage, BatchRef, BatchRow,
    BatchStatus, BatchSummary, LaneKind, LaneState, ReviewState, TouchRing,
};
pub use assets::{ASSET_LIST_MAX, ASSET_PAGE, ASSET_PAGE_MAX, AssetInfo, AssetPage, AssetsState};
pub use effect::{Effect, OutOp};
pub use event::{Event, RevertSkip, RevertSkipReason, VariableEntity};
pub use export::{ExportDialog, ExportFinding, ExportFormat, ExportState, ExportTarget};
pub use layout::{Axis, Direction, LayoutNote, Side, Sides};
pub use state::{
    AppState, Checkpoint, CheckpointKind, Connection, FieldBinding, FillKind, History, Panel,
    Panels, Properties, Tool,
};
pub use variables::{
    BindField, BindOption, CollectionRow, ModeValue, Picker, PickerRow, TypeHeading, VariableRow,
    VariableValue, VariablesPanel,
};
