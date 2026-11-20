//! The view state of the app: the tool, the selection and the panels. None of it is an op or an undo step.

use crate::app::App;
use crate::event::Event;
use crate::state::Panel;

impl App {
    pub(crate) fn set_tool(&mut self, tool: crate::state::Tool) {
        if self.state.tool != tool {
            self.state.tool = tool;
            self.events.push(Event::ToolChanged { tool });
        }
    }

    pub(crate) fn select(&mut self, ids: Vec<String>) {
        let mut keep: Vec<String> = Vec::with_capacity(ids.len());
        for id in ids {
            if !keep.contains(&id) && self.replica.nodes().iter().any(|n| n.id == id) {
                keep.push(id);
            }
        }
        if self.state.selection != keep {
            self.state.selection = keep.clone();
            self.events.push(Event::SelectionChanged { ids: keep });
        }
    }

    pub(crate) fn is_open(&self, panel: Panel) -> bool {
        match panel {
            Panel::Layers => self.state.panels.layers,
            Panel::Properties => self.state.panels.properties,
            Panel::Variables => self.state.panels.variables,
        }
    }

    pub(crate) fn set_panel(&mut self, panel: Panel, open: bool) {
        if self.is_open(panel) != open {
            match panel {
                Panel::Layers => self.state.panels.layers = open,
                Panel::Properties => self.state.panels.properties = open,
                Panel::Variables => self.state.panels.variables = open,
            }
            self.events.push(Event::PanelChanged { panel, open });
        }
    }

    pub(crate) fn toggle_collapsed(&mut self, id: String) {
        let known = self.replica.nodes().iter().any(|n| n.id == id);
        if !known && !self.state.panels.collapsed.contains(&id) {
            return;
        }
        let list = &mut self.state.panels.collapsed;
        let collapsed = match list.iter().position(|c| *c == id) {
            Some(at) => {
                list.remove(at);
                false
            }
            None => {
                list.push(id.clone());
                true
            }
        };
        self.events.push(Event::CollapsedChanged { id, collapsed });
    }
}
