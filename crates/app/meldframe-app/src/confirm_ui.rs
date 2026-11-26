//! The tab of the right card as state of the app (audit finding T10, issue 323). Choosing the Activity tab asks for the
//! newest batches and the checkpoints once (`activity.rs` does the fetch); a tab that is selected already asks for nothing.

use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use meldframe_app_model::{Action, ActivityOp, RightTab};

impl App {
    /// An op that changes the view only: the tab.
    pub(crate) fn activity_view(&mut self, op: ActivityOp, fx: &mut Vec<Effect>) {
        match op {
            ActivityOp::SetTab { tab } => {
                if self.state.right_tab == tab {
                    return;
                }
                self.state.right_tab = tab;
                self.events.push(Event::RightTabChanged { tab });
                if tab == RightTab::Activity {
                    self.activity_action(Action::OpenActivity {}, fx);
                }
            }
        }
    }
}
