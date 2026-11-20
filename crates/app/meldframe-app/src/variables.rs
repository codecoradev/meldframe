//! Variables in the app layer (E3.3): the mode a collection is shown in, the resolved values of the properties
//! panel, and the action that applies variable commands. The mode is view state; the rules of resolving, checking
//! and applying are in `meldframe-variables` and `meldframe-check`.

use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use crate::state::{FieldBinding, Properties};
use meldframe_app_model::BindField;
use meldframe_check::{ApplyError, Limits, check_command_with};
use meldframe_protocol::{Command, Node};
use meldframe_render::Fields;
use meldframe_sync::SubmitError;
use meldframe_variables::{ModeChoice, VarError, apply, is_variable_command, node_bindings};

impl App {
    /// The modes the canvas is painted in, from `AppState::modes`.
    pub(crate) fn choice(&self) -> ModeChoice {
        ModeChoice::of(
            self.state
                .modes
                .iter()
                .map(|(c, m)| (c.as_str(), m.as_str())),
        )
    }

    /// Pick the mode of a collection, or its default mode for `None`. Only the view changes: no op is sent, the
    /// undo history stays and the canvas paints again (`Effect::Render` with an empty list: everything).
    pub(crate) fn set_mode(
        &mut self,
        collection: String,
        mode: Option<String>,
        fx: &mut Vec<Effect>,
    ) {
        let found = self.vars().collection(&collection).map(|c| {
            mode.as_ref()
                .map(|m| c.modes.iter().any(|x| x.id == *m))
                .unwrap_or(true)
        });
        let code = match found {
            None => VarError::CollectionNotFound(collection.clone()).code(),
            Some(false) => VarError::ModeNotFound(mode.clone().unwrap_or_default()).code(),
            Some(true) => "",
        };
        if !code.is_empty() {
            self.events.push(Event::Refused { code: code.into() });
            return;
        }
        // The default mode is no pick at all, so the state is empty again when every collection is on it.
        let first = self
            .vars()
            .collection(&collection)
            .and_then(|c| c.modes.first().map(|m| m.id.clone()));
        let pick = mode.filter(|m| Some(m) != first.as_ref());
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
        let gone: Vec<String> = self
            .state
            .modes
            .iter()
            .filter(|(c, m)| {
                !self
                    .vars()
                    .collection(c)
                    .is_some_and(|col| col.modes.iter().any(|x| x.id == **m))
            })
            .map(|(c, _)| c.clone())
            .collect();
        for collection in gone {
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
        let limits = Limits::default();
        let mut vars = self.vars().clone();
        for c in commands {
            if !is_variable_command(c) {
                self.events.push(Event::Refused {
                    code: "not_a_variable_command".into(),
                });
                return Ok(());
            }
            check_command_with(self.replica.nodes(), &vars, c, &limits)
                .map_err(SubmitError::Invalid)?;
            apply(&mut vars, c);
        }
        self.replica.begin_group();
        let done = commands.iter().try_for_each(|c| self.replica.apply(c));
        self.replica.end_group();
        done
    }

    /// The editable values of a node as they are painted: a bound field shows what the render step resolves it to
    /// (`Fields` is the one place that does it, so the panel and the canvas agree, unresolved defaults included).
    pub(crate) fn properties(&self, n: &Node) -> Properties {
        let choice = self.choice();
        let fields = Fields::new(self.vars(), &choice);
        let found = node_bindings(n);
        let bound: Vec<String> = found.iter().map(|b| b.field.to_string()).collect();
        let bindings = found
            .iter()
            .filter_map(|b| {
                let field = match b.field {
                    "fill" => BindField::Fill,
                    "radius" => BindField::Radius,
                    "opacity" => BindField::Opacity,
                    _ => return None,
                };
                let variable = b.var.to_string();
                let name = self
                    .vars()
                    .variables
                    .iter()
                    .find(|v| v.id == variable)?
                    .name
                    .clone();
                Some(FieldBinding {
                    field,
                    variable,
                    name,
                })
            })
            .collect();
        Properties {
            id: n.id.clone(),
            name: n.name.clone(),
            kind: n.kind,
            x: n.x,
            y: n.y,
            w: n.w,
            h: n.h,
            fill: fields.fill_text(&n.fill),
            radius: fields.radius(&n.radius),
            text: n.text.clone(),
            font_size: n.font_size,
            opacity: fields.opacity(&n.opacity),
            clip: n.clip,
            bound,
            bindings,
        }
    }
}

/// A refusal as the app words it: variable errors keep their `E_` code (`docs/error-codes.md`).
pub(crate) fn variable_code(e: &ApplyError) -> Option<&'static str> {
    match e {
        ApplyError::Variable(v) => Some(v.code()),
        _ => None,
    }
}
