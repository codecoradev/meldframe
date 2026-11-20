//! The plain data of the app layer: [`Action`] in, [`Effect`] and [`Event`] out, and the [`AppState`] a front
//! end reads. Every type is serialisable data with no UI toolkit (ADR-0008, standard rule U1). The state
//! machine that uses them is `meldframe-app`, which re-exports all of this; this crate only keeps that one
//! under the size ceiling and holds no logic.

pub mod action;
pub mod effect;
pub mod event;
pub mod state;
pub mod variables;

pub use action::{Action, CreateKind, Frame, FramePreset, Property};
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
