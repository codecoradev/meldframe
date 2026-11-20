//! The plain data of the app layer: [`Action`] in, [`Effect`] and [`Event`] out, and the [`AppState`] a front
//! end reads. Every type is serialisable data with no UI toolkit (ADR-0008, standard rule U1). The state
//! machine that uses them is `meldframe-app`, which re-exports all of this; this crate only keeps that one
//! under the size ceiling and holds no rules. The only code is conversions (`From`: what `Feed::view` of
//! `meldframe-activity` gives, a `BatchPage`), `Frame::observe` (a plain mapping of a frame to the feed's calls) and
//! the classification of an action (`Action::is_activity`, `Action::only_activity`).

pub mod action;
pub mod activity;
pub mod effect;
pub mod event;
pub mod state;
pub mod variables;

pub use action::{Action, CreateKind, Frame, FramePreset, Property};
pub use activity::{
    ActivityLane, ActivityRun, ActivityState, BatchHeader, BatchPage, BatchRef, BatchRow,
    BatchStatus, BatchSummary, LaneKind, LaneState, ReviewState, TouchRing,
};
pub use effect::{Effect, OutOp};
pub use event::{Event, RevertSkip, RevertSkipReason};
pub use state::{
    AppState, Checkpoint, CheckpointKind, Connection, FieldBinding, History, Panel, Panels,
    Properties, Tool,
};
pub use variables::{
    BindField, BindOption, CollectionRow, ModeValue, TypeHeading, VariableRow, VariableValue,
    VariablesPanel,
};
