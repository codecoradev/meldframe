//! The state machine: one action in, state changes, events and effects out. No I/O, clock or randomness.

use crate::action::Action;
use crate::effect::Effect;
use crate::event::Event;
use crate::state::{AppState, Connection, Panel};

/// Owns the [`AppState`] and applies actions to it.
#[derive(Debug, Default)]
pub struct App {
    state: AppState,
    events: Vec<Event>,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The events raised since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Applies one action and returns the work the platform has to do. The same actions in the same order
    /// always give the same state, events and effects.
    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        let mut effects = Vec::new();
        match action {
            Action::Select { ids } => self.select(dedup(ids)),
            Action::ToggleSelect { id } => {
                let mut ids = self.state.selection.clone();
                match ids.iter().position(|current| *current == id) {
                    Some(at) => {
                        ids.remove(at);
                    }
                    None => ids.push(id),
                }
                self.select(ids);
            }
            Action::ClearSelection {} => self.select(Vec::new()),
            Action::SetTool { tool } => {
                if self.state.tool != tool {
                    self.state.tool = tool;
                    self.events.push(Event::ToolChanged { tool });
                }
            }
            Action::SetPanel { panel, open } => self.set_panel(panel, open),
            Action::TogglePanel { panel } => {
                let open = !self.is_open(panel);
                self.set_panel(panel, open);
            }
            Action::ConnectionChanged { connection } => {
                self.set_connection(connection, &mut effects);
            }
            // Undo, redo, submit and their results are added in the next change.
            _ => {}
        }
        effects
    }

    fn select(&mut self, ids: Vec<String>) {
        if self.state.selection != ids {
            self.state.selection = ids.clone();
            self.events.push(Event::SelectionChanged { ids });
        }
    }

    fn set_connection(&mut self, connection: Connection, effects: &mut Vec<Effect>) {
        if self.state.connection != connection {
            self.state.connection = connection;
            self.events.push(Event::ConnectionChanged { connection });
            if connection == Connection::Disconnected {
                effects.push(Effect::Reconnect {});
            }
        }
    }

    fn is_open(&self, panel: Panel) -> bool {
        match panel {
            Panel::Left => self.state.panels.left,
            Panel::Right => self.state.panels.right,
        }
    }

    fn set_panel(&mut self, panel: Panel, open: bool) {
        if self.is_open(panel) != open {
            match panel {
                Panel::Left => self.state.panels.left = open,
                Panel::Right => self.state.panels.right = open,
            }
            self.events.push(Event::PanelChanged { panel, open });
        }
    }
}

/// Drops repeated ids and keeps the first occurrence of each.
fn dedup(ids: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !seen.contains(&id) {
            seen.push(id);
        }
    }
    seen
}
