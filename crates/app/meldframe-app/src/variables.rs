//! Variables in the app layer (E3.3): the mode a collection is shown in, the resolved values of the properties
//! panel, and the action that applies variable commands. The mode is view state; the rules of resolving, checking
//! and applying are in `meldframe-variables` and `meldframe-check`, and the rules of the pick and of the check of
//! a list of commands are in `meldframe_app_rules::modes`.

use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use meldframe_app_rules::modes::{self, EditRefusal};
use meldframe_protocol::Command;
use meldframe_sync::SubmitError;
use meldframe_variables::ModeChoice;

impl App {
    /// The modes the canvas is painted in, from `AppState::modes`.
    pub(crate) fn choice(&self) -> ModeChoice {
        modes::choice(&self.state.modes)
    }

    /// Pick the mode of a collection, or its default mode for `None`. Only the view changes: no op is sent, the
    /// undo history stays and the canvas paints again (`Effect::Render` with an empty list: everything).
    pub(crate) fn set_mode(
        &mut self,
        collection: String,
        mode: Option<String>,
        fx: &mut Vec<Effect>,
    ) {
        let pick = match modes::pick(self.vars(), &collection, mode) {
            Ok(pick) => pick,
            Err(code) => {
                self.events.push(Event::Refused { code: code.into() });
                return;
            }
        };
        let before = self.state.modes.get(&collection).cloned();
        match &pick {
            Some(m) => self.state.modes.insert(collection.clone(), m.clone()),
            None => self.state.modes.remove(&collection),
        };
        if before != pick {
            self.events.push(Event::ModeChanged {
                collection,
                mode: pick,
            });
            fx.push(Effect::Render { nodes: Vec::new() });
        }
    }

    /// A mode that a remote change removed (or whose collection it removed) goes back to the default.
    pub(crate) fn prune_modes(&mut self) {
        for collection in modes::removed(self.vars(), &self.state.modes) {
            self.state.modes.remove(&collection);
            self.events.push(Event::ModeChanged {
                collection,
                mode: None,
            });
        }
    }

    /// Apply variable commands as one undo step, after every one of them was checked on what the ones before it
    /// make of the variables.
    ///
    /// Every command is checked before any is applied, and the replica applies them under the same rules, so a
    /// failure after the check is not expected; if one came (the checks and the replica disagree), the commands
    /// before it stay applied as the one step and the error is returned.
    pub(crate) fn edit_variables(&mut self, commands: &[Command]) -> Result<(), SubmitError> {
        if commands.is_empty() {
            return Ok(());
        }
        let max = self.replica.max_pending();
        if self.replica.pending_len() + commands.len() > max {
            return Err(SubmitError::OutboxFull { max });
        }
        match modes::check_edit(self.replica.nodes(), self.vars(), commands) {
            Ok(()) => {}
            Err(EditRefusal::NotVariable) => {
                self.events.push(Event::Refused {
                    code: "not_a_variable_command".into(),
                });
                return Ok(());
            }
            Err(EditRefusal::Invalid(e)) => return Err(SubmitError::Invalid(e)),
        }
        self.replica.begin_group();
        let done = commands.iter().try_for_each(|c| self.replica.apply(c));
        self.replica.end_group();
        done
    }
}
