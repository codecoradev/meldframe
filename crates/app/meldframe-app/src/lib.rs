//! UI-neutral state, actions, effects and events.
//!
//! A front end (the web editor now; a desktop or mobile app or a test harness later) does three things
//! only: read [`AppState`], send [`Action`]s, and carry out the [`Effect`]s it is given. Every type here is
//! plain serialisable data with no UI toolkit (ADR-0008, standard rule U1).
//!
//! The [`App`] is a synchronous, deterministic state machine: it does no I/O, reads no clock and draws
//! nothing. Time comes in as a payload of an action; the transport is an effect the shell carries out. The
//! document lives in a [`meldframe_sync::Replica`] inside the app: the confirmed file, plus the person's
//! own pending ops, plus their undo history.
//!
//! An [`Action`] is an input to this layer. It is not a [`meldframe_protocol::Command`]: the command is the
//! change to the file, and the app layer works out the commands from what the person meant.

mod activity;
mod app;
mod assets;
mod canvas;
mod checkpoints;
mod document;
mod export;
mod frames;
mod interact;
mod link;
mod manage;
mod panel;
mod picker;
mod props;
mod retry;
mod review;
mod variables;
mod variables_io;
mod view;

pub(crate) use meldframe_app_model::{action, effect, event, state};

pub use app::App;
pub use canvas::MeasurerBox;
pub use link::{ATTEMPT_MS, SILENT_MS};
pub use meldframe_app_model::{
    ASSET_LIST_MAX, ASSET_PAGE, ASSET_PAGE_MAX, Action, AppState, AssetInfo, AssetPage,
    AssetsState, Axis, BindField, BindOption, Checkpoint, CheckpointKind, CheckpointRequest,
    CheckpointRow, CheckpointsStatus, CollectionRow, Connection, CreateKind, Direction, Effect,
    Event, ExportDialog, ExportFinding, ExportFormat, ExportState, ExportTarget, FieldBinding,
    FillKind, Frame, FramePreset, History, LayoutNote, ModeValue, OutOp, Panel, Panels, Picker,
    PickerRow, Properties, Property, RevertSkip, RevertSkipReason, Side, Sides, Tool, TypeHeading,
    VariableEntity, VariableRow, VariableValue, VariablesPanel,
};
pub use meldframe_app_model::{
    ImportAt, ImportEntry, ImportEntryKind, ImportReport, ImportStatus, VariablesExportDialog,
    VariablesImportResult, VariablesIo,
};
pub use meldframe_app_rules::drop::Slot as DropSlot;
pub use meldframe_app_rules::multi::Preview as DropPreview;
pub use meldframe_layout::{StubMeasurer, TextMeasurer};
pub use meldframe_protocol::ImageFit;
pub use meldframe_sync::LoadError;
