//! The Variables manage actions: make a collection, a mode or a variable with ids the app mints, and
//! remove them, letting go of what uses them first when the person says so. The commands are worked out in
//! `meldframe_app_parts::manage`; every one goes through the replica as any other change (and so to the gate), and a list
//! of them is one undo step.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use meldframe_app_model::VariableEntity;
use meldframe_app_parts::manage::{self, Target};
use meldframe_app_rules::modes::check_group;
use meldframe_protocol::{Command, VarType, Variables};
use meldframe_sync::SubmitError;

impl App {
    /// An id nobody uses yet in `taken`'s table: the prefix, a letter for the kind and the counter that node ids use.
    fn mint(&mut self, kind: char, taken: impl Fn(&Variables, &str) -> bool) -> String {
        loop {
            self.next_id += 1;
            let id = format!("{}-{kind}{}", self.prefix, self.next_id);
            if !taken(self.replica.vars(), &id) {
                return id;
            }
        }
    }

    /// Apply the commands as one undo step, after the whole list was checked on what the ones before it make of the
    /// file and found to fit the queue of ops: a list that would fail half way, or would not fit, applies nothing.
    fn apply_group(&mut self, commands: &[Command]) -> Result<(), SubmitError> {
        self.check_room(commands)?;
        check_group(self.replica.nodes(), self.replica.vars(), commands)
            .map_err(SubmitError::Invalid)?;
        self.replica.begin_group();
        let done = commands.iter().try_for_each(|c| self.replica.apply(c));
        self.replica.end_group();
        done
    }

    /// Apply one command that makes `entity` with `id`, and say so.
    fn add_entity(
        &mut self,
        command: Command,
        entity: VariableEntity,
        id: String,
    ) -> Result<(), SubmitError> {
        self.apply_group(&[command])?;
        self.events.push(Event::EntityAdded { entity, id });
        Ok(())
    }

    pub(crate) fn create_collection(&mut self, name: &str) -> Result<(), SubmitError> {
        let id = self.mint('c', |v, id| v.collection(id).is_some());
        let mode = self.mint('m', |_, _| false);
        let command = manage::new_collection(&id, &mode, name);
        self.add_entity(command, VariableEntity::Collection, id)
    }

    pub(crate) fn add_mode(
        &mut self,
        collection: &str,
        name: &str,
        copy_from: Option<String>,
    ) -> Result<(), SubmitError> {
        let id = self.mint('m', |v, id| {
            v.collection(collection)
                .is_some_and(|c| c.modes.iter().any(|m| m.id == id))
        });
        let command = manage::new_mode(collection, &id, name, copy_from);
        self.add_entity(command, VariableEntity::Mode, id)
    }

    pub(crate) fn remove_mode(
        &mut self,
        collection: String,
        mode: String,
    ) -> Result<(), SubmitError> {
        self.apply_group(&[Command::RemoveMode { collection, mode }])
    }

    pub(crate) fn add_variable(
        &mut self,
        collection: &str,
        name: &str,
        kind: VarType,
    ) -> Result<(), SubmitError> {
        let id = self.mint('v', |v, id| v.variable(id).is_some());
        match manage::new_variable(self.vars(), &id, collection, name, kind) {
            Ok(command) => self.add_entity(command, VariableEntity::Variable, id),
            Err(code) => refuse(&mut self.events, code),
        }
    }

    pub(crate) fn remove_entity(
        &mut self,
        target: Target<'_>,
        detach: bool,
    ) -> Result<(), SubmitError> {
        match manage::removal(self.nodes(), self.vars(), target, detach) {
            Ok(commands) => self.apply_group(&commands),
            Err(code) => refuse(&mut self.events, code),
        }
    }
}
