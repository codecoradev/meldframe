//! The plain data of the app layer: [`Action`] in, [`Effect`] and [`Event`] out, and the [`AppState`] a front
//! end reads. Every type is serialisable data with no UI toolkit (ADR-0008, standard rule U1). The state
//! machine that uses them is `meldframe-app`, which re-exports all of this; this crate only keeps that one
//! under the size ceiling. It holds few rules, each a plain function of its data. The code is conversions (`From`: what
//! `Feed::view` of `meldframe-activity` gives, a `BatchPage`), `Frame::observe` (a plain mapping of a frame to the
//! feed's calls), the classification of an action (`Action::is_activity`, `Action::only_activity`, `Action::is_assets`, `Action::is_checkpoint`),
//! and the one real rule set, the transitions of the image picker's list (`AssetsState`: which answer is taken, what a
//! page has to look like, the cap on the list). The rules of the other screens are in `meldframe-app-parts` and
//! `meldframe-app-rules`.

pub mod action;
mod action_new;
pub mod activity;
pub mod actor;
pub mod assets;
pub use meldframe_camera as camera;
pub mod checkpoint;
mod classify;
pub mod confirm;
pub mod effect;
pub mod event;
pub mod frame;
pub mod gesture;
pub mod key;
pub mod layers;
pub mod layout;
pub mod manage;
pub mod overlay;
pub mod preset;
pub mod property;
pub mod state;
pub mod text_edit;
pub mod text_field;
pub mod variables;
pub mod variables_io;
pub mod wire;

pub use meldframe_app_export_model::{export, raster};

pub use action::{Action, CreateKind, Frame, FramePreset, Property};
pub use activity::{
    ActivityLane, ActivityRun, ActivityState, BatchHeader, BatchPage, BatchRef, BatchRow,
    BatchStatus, BatchSummary, LaneKind, LaneState, ReviewState, TouchRing,
};
pub use actor::{ActorTone, ActorView};
pub use assets::{ASSET_LIST_MAX, ASSET_PAGE, ASSET_PAGE_MAX, AssetInfo, AssetPage, AssetsState};
pub use checkpoint::{
    Checkpoint, CheckpointKind, CheckpointRequest, CheckpointRow, CheckpointsStatus,
};
pub use confirm::{ActivityOp, Confirm, RightTab};
pub use effect::{Effect, OutOp};
pub use event::{Event, RevertSkip, RevertSkipReason, VariableEntity};
pub use export::{ExportDialog, ExportFinding, ExportFormat, ExportState, ExportTarget};
pub use gesture::{GestureOp, Handle, Point};
pub use key::{KeyFocus, KeyInput, Modifiers};
pub use layers::LayerRow;
pub use layout::{Axis, Direction, LayoutNote, Side, Sides};
pub use manage::{
    MODE_NAME_MAX, ManageDrafts, ManageLimits, ManageOp, ManageState, ManageTarget, NameCheck,
    NameDraft, NameError, NameWarning, Removal, RenameField,
};
pub use meldframe_camera::{Camera, CameraOp, Viewport, Wheel, WheelUnit, ZoomCommand};
pub use overlay::{Overlay, OverlayOp};
pub use raster::{
    CAPTURE_MAX_BYTES, CaptureAsk, EXPORT_MAX_PIXELS, MAX_SCALE, MIN_SCALE, PNG_MIN_BYTES, PngOp,
    PngPlan, PngSink, PngState, RasterRect, RasterSink, RasterSize, SCALES, Secret,
};
pub use state::{
    AppState, Connection, FieldBinding, FillKind, History, Panel, Panels, Properties, Sections,
    Tool,
};
pub use text_edit::{TextEdit, TextEditOp};
pub use text_field::{ShadowPart, TextField, TypedText};
pub use variables::{
    BindField, BindOption, CollectionRow, ModeValue, Picker, PickerRow, TypeHeading, VariableRow,
    VariableValue, VariablesPanel,
};
pub use variables_io::{
    ImportAt, ImportEntry, ImportEntryKind, ImportReport, ImportStatus, VariablesExportDialog,
    VariablesImportResult, VariablesIo,
};
pub use wire::{MAX_SAFE_INT, Wire};
