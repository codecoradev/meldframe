//! The helpers of the tests of the key input (`key_input.rs`, `key_rows.rs`): a key press as an action, and the checks of what it
//! raised.
#![allow(dead_code)]

use super::Loopback;
use meldframe_app::{Action, Event, KeyFocus, KeyInput, Modifiers};

pub const NONE: Modifiers = Modifiers {
    ctrl: false,
    meta: false,
    alt: false,
    shift: false,
};

pub fn press(key: &str, code: &str, modifiers: Modifiers) -> Action {
    Action::Key {
        input: KeyInput {
            key: key.into(),
            code: code.into(),
            modifiers,
            repeat: false,
            composing: false,
            focus: KeyFocus::Page,
        },
    }
}
pub fn key(k: &str) -> Action {
    press(k, "", NONE)
}
pub fn ctrl(k: &str) -> Action {
    press(k, "", Modifiers { ctrl: true, ..NONE })
}
pub fn cmd(k: &str) -> Action {
    press(k, "", Modifiers { meta: true, ..NONE })
}
pub fn focused(mut a: Action, focus: KeyFocus) -> Action {
    if let Action::Key { input } = &mut a {
        input.focus = focus;
    }
    a
}

/// A connected app with the events of connecting taken.
pub fn live() -> Loopback {
    let mut l = Loopback::live();
    l.app.take_events();
    l
}
pub fn handled(events: &[Event]) -> bool {
    events.contains(&Event::KeyHandled {})
}
pub fn changed(events: &[Event]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::DocumentChanged { .. }))
}
pub fn nothing(l: &mut Loopback, a: Action) {
    let fx = l.send(a);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(l.events.is_empty(), "{:?}", l.events);
}
pub fn exists(l: &Loopback, id: &str) -> bool {
    l.app.nodes().iter().any(|n| n.id == id)
}
pub fn drain(l: &mut Loopback) -> Vec<Event> {
    std::mem::take(&mut l.events)
}
