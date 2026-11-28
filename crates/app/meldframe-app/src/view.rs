//! The view state of the app: the tool, the selection and the panels, none of which is an op or an undo step.

use crate::app::App;
use crate::event::Event;
use crate::parts::panel::{is_open, set_open, toggle_listed};
use crate::parts::selection::known;
use crate::state::Panel;
use crate::text;
use std::collections::{BTreeMap, HashSet};

impl App {
    pub(crate) fn set_tool(&mut self, tool: crate::state::Tool) {
        if self.state.tool != tool {
            self.state.tool = tool;
            self.events.push(Event::ToolChanged { tool });
        }
    }

    pub(crate) fn select(&mut self, ids: Vec<String>) {
        let keep = known(ids, self.replica.nodes());
        if self.state.selection != keep {
            self.state.selection = keep.clone();
            self.events.push(Event::SelectionChanged { ids: keep });
        }
    }

    pub(crate) fn is_open(&self, panel: Panel) -> bool {
        is_open(&self.state.panels, panel)
    }

    pub(crate) fn set_panel(&mut self, panel: Panel, open: bool) {
        if self.is_open(panel) != open {
            set_open(&mut self.state.panels, panel, open);
            self.events.push(Event::PanelChanged { panel, open });
        }
    }

    /// The snapshot's attribution joins `last_actor`: a node the file has, an actor with something left of it after the
    /// strict clean, and never over a change seen live.
    pub(crate) fn attribute(&mut self, actors: BTreeMap<String, String>) {
        let nodes: HashSet<&str> = self.replica.nodes().iter().map(|n| n.id.as_str()).collect();
        let mut taken = Vec::new();
        for (id, raw) in actors {
            if !nodes.contains(id.as_str()) || self.state.last_actor.contains_key(&id) {
                continue;
            }
            if let Some(actor) = text::actor(&raw) {
                taken.push((id, actor));
            }
        }
        if !taken.is_empty() {
            self.state.last_actor.extend(taken);
            self.events.push(Event::AttributionChanged {});
        }
    }

    pub(crate) fn toggle_collapsed(&mut self, id: String) {
        let known = self.replica.nodes().iter().any(|n| n.id == id);
        if !known && !self.state.panels.collapsed.contains(&id) {
            return;
        }
        let collapsed = toggle_listed(&mut self.state.panels.collapsed, &id);
        self.events.push(Event::CollapsedChanged { id, collapsed });
    }
}
