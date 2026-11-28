//! Helpers of the pointer tests (`layout_pointer.rs`, `layout_pointer_view.rs`): events sent to an app, and what they made.
#![allow(dead_code)]

use meldframe_app::{
    Action, App, CameraOp, Effect, Event, Modifiers, PointerButton, PointerInput, PointerPhase,
};
use meldframe_protocol::{Command, NodePatch};

pub fn input(phase: PointerPhase, x: f64, y: f64) -> PointerInput {
    PointerInput {
        phase,
        x,
        y,
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        touch: false,
        space: false,
        at_ms: None,
    }
}

pub fn send(a: &mut App, i: PointerInput) -> Vec<Effect> {
    a.dispatch(Action::Pointer { input: i })
}

pub fn press(a: &mut App, x: f64, y: f64) {
    send(a, input(PointerPhase::Down, x, y));
}

pub fn moved(a: &mut App, x: f64, y: f64) {
    send(a, input(PointerPhase::Move, x, y));
}

/// Lets go; returns the commands that were sent.
pub fn release(a: &mut App, x: f64, y: f64) -> Vec<Command> {
    let fx = send(a, input(PointerPhase::Up, x, y));
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

pub fn click(a: &mut App, x: f64, y: f64) {
    press(a, x, y);
    release(a, x, y);
}

pub fn selection(a: &App) -> Vec<String> {
    a.state().selection.clone()
}

pub fn changed(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::DocumentChanged { .. }))
        .count()
}

pub fn camera(a: &mut App, op: CameraOp) {
    a.dispatch(Action::Camera { op });
}

/// A change of a node the app does not have diverges it: it is halted.
pub fn halt(a: &mut App) {
    a.dispatch(Action::Frame {
        now_ms: 0,
        frame: meldframe_app::Frame::Event {
            seq: 1,
            actor: "agent:a".into(),
            key: Some("k".into()),
            command: Command::UpdateNode {
                id: "ghost".into(),
                patch: NodePatch::default(),
            },
        },
    });
}
