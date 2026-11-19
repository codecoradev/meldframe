//! The Variables panel (E3.4): the rows it lists, its search, and the actions that edit a value or bind a field.
//! Every rule is here; a front end draws `AppState::variables` and sends the actions. The checks of a value, an
//! alias or a binding are the ones of `meldframe-variables`, run when the command is applied.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use meldframe_app_model::{
    BindField, BindOption, CollectionRow, ModeValue, VariableRow, VariableValue, VariablesPanel,
};
use meldframe_protocol::{
    Bind, Collection, Command, Node, NodeKind, NodePatch, Value, VarType, Variable, Variables,
};
use meldframe_render::Fields;
use meldframe_sync::SubmitError;
use meldframe_variables::{ModeChoice, Resolver, node_bindings};
use std::collections::HashMap;

/// The most variables the panel lists at once; `VariablesPanel::matched` says how many matched.
const MAX_LISTED: usize = 200;

/// Whether a field of a node of this kind can hold a binding the panel offers: a text node has no corners.
fn bindable(kind: NodeKind, field: BindField) -> bool {
    !(kind == NodeKind::Text && field == BindField::Radius)
}

fn field_name(field: BindField) -> &'static str {
    match field {
        BindField::Fill => "fill",
        BindField::Radius => "radius",
        BindField::Opacity => "opacity",
    }
}

fn number_text(v: f64) -> String {
    format!("{v}")
}

fn value_text(v: &Value) -> String {
    match v {
        Value::Color(css) => css.clone(),
        Value::Number(n) => number_text(*n),
    }
}

impl App {
    /// Search the panel. View state: no op, no undo step.
    pub(crate) fn set_variables_query(&mut self, query: String) {
        let query = query.trim().to_string();
        if self.state.variables.query != query {
            self.events.push(Event::VariablesQueryChanged {
                query: query.clone(),
            });
            self.state.variables.query = query;
        }
    }

    /// Bring `AppState::variables` in line with the document, the modes, the selection and the search. Nothing is
    /// listed while the panel is closed.
    pub(crate) fn refresh_variables(&mut self) {
        let query = self.state.variables.query.clone();
        let view = if self.state.panels.variables {
            self.variables_view(&query)
        } else {
            VariablesPanel {
                query,
                ..Default::default()
            }
        };
        if self.state.variables != view {
            self.state.variables = view;
        }
    }

    fn variables_view(&self, query: &str) -> VariablesPanel {
        let vars = self.vars();
        let choice = self.choice();
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let by_id: HashMap<&str, &Variable> =
            vars.variables.iter().map(|v| (v.id.as_str(), v)).collect();
        let uses = use_counts(self.nodes(), vars);
        let selected = match self.state.selection.as_slice() {
            [id] => self.nodes().iter().find(|n| n.id == *id),
            _ => None,
        };
        let (mut matched, mut listed) = (0, 0);
        let mut collections = Vec::new();
        for col in &vars.collections {
            let modes: Vec<ModeChoice> = col
                .modes
                .iter()
                .map(|m| {
                    let mut c = choice.clone();
                    c.set(&col.id, &m.id);
                    c
                })
                .collect();
            // The indexes are built when the first row of the collection is listed.
            let mut resolvers: Option<Vec<Resolver<'_>>> = None;
            let mut rows = Vec::new();
            for v in vars.variables.iter().filter(|v| v.collection == col.id) {
                let texts = texts_of(col, v, &by_id);
                let hay = format!(
                    "{} {} {}",
                    v.name,
                    col.name,
                    texts
                        .iter()
                        .map(|(t, _)| t.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
                .to_lowercase();
                if !words.iter().all(|w| hay.contains(w)) {
                    continue;
                }
                matched += 1;
                if listed >= MAX_LISTED {
                    continue;
                }
                listed += 1;
                let rs = resolvers
                    .get_or_insert_with(|| modes.iter().map(|c| Resolver::new(vars, c)).collect());
                let values = col
                    .modes
                    .iter()
                    .zip(texts)
                    .zip(rs.iter())
                    .map(|((m, (text, alias)), r)| ModeValue {
                        mode: m.id.clone(),
                        text,
                        alias,
                        resolved: r.value(&v.id).map(|x| value_text(&x)).unwrap_or_default(),
                    })
                    .collect();
                rows.push(VariableRow {
                    id: v.id.clone(),
                    name: v.name.clone(),
                    kind: v.kind,
                    values,
                    used_by: uses.get(v.id.as_str()).copied().unwrap_or(0),
                    binds: selected.map(|n| binds_of(n, v)).unwrap_or_default(),
                });
            }
            if !rows.is_empty() || words.is_empty() {
                let active = choice
                    .get(&col.id)
                    .filter(|m| col.modes.iter().any(|x| x.id == *m))
                    .or_else(|| col.modes.first().map(|m| m.id.as_str()))
                    .unwrap_or_default();
                collections.push(CollectionRow {
                    id: col.id.clone(),
                    name: col.name.clone(),
                    modes: col.modes.clone(),
                    active: active.to_string(),
                    variables: rows,
                });
            }
        }
        VariablesPanel {
            query: query.to_string(),
            total: vars.variables.len(),
            matched,
            collections,
        }
    }

    /// Set the value of a variable in one mode.
    pub(crate) fn set_variable_value(
        &mut self,
        variable: String,
        mode: String,
        value: VariableValue,
    ) -> Result<(), SubmitError> {
        let Some(var) = self.vars().variable(&variable) else {
            return refuse(&mut self.events, "E_VARIABLE_NOT_FOUND");
        };
        let new: Bind<Value> = match value {
            VariableValue::Alias { variable: to } => Bind::var(to),
            VariableValue::Literal { text } => match var.kind {
                VarType::Color => Bind::Lit(Value::Color(text.trim().to_string())),
                VarType::Number => match text.trim().parse::<f64>() {
                    Ok(n) => Bind::Lit(Value::Number(n)),
                    Err(_) => return refuse(&mut self.events, "invalid_value"),
                },
            },
        };
        if var.values.get(&mode) == Some(&new) {
            return Ok(());
        }
        self.replica.apply(&Command::SetVariableValue {
            id: variable,
            mode,
            value: new,
        })
    }

    /// Bind a field of a node to a variable, or let it go with the value that is painted.
    pub(crate) fn bind_property(
        &mut self,
        id: &str,
        field: BindField,
        variable: Option<String>,
    ) -> Result<(), SubmitError> {
        let Some(node) = self.nodes().iter().find(|n| n.id == id) else {
            return refuse(&mut self.events, "not_found");
        };
        if !bindable(node.kind, field) {
            return refuse(&mut self.events, "not_bindable");
        }
        let mut patch = NodePatch::default();
        match variable {
            Some(v) => match field {
                BindField::Fill => patch.fill = Some(Bind::var(v)),
                BindField::Radius => patch.radius = Some(Bind::var(v)),
                BindField::Opacity => patch.opacity = Some(Bind::var(v)),
            },
            None => {
                if !node_bindings(node)
                    .iter()
                    .any(|b| b.field == field_name(field))
                {
                    return Ok(());
                }
                let choice = self.choice();
                let f = Fields::new(self.vars(), &choice);
                match field {
                    BindField::Fill => patch.fill = Some(Bind::Lit(f.fill_text(&node.fill))),
                    BindField::Radius => patch.radius = Some(Bind::Lit(f.radius(&node.radius))),
                    BindField::Opacity => patch.opacity = Some(Bind::Lit(f.opacity(&node.opacity))),
                }
            }
        }
        self.replica.apply(&Command::UpdateNode {
            id: id.to_string(),
            patch,
        })
    }
}

/// The text of each value of `v`, in the order of the modes of `col`: a literal as stored, an alias as the name of
/// its target (its id when the target is missing), with the id of the target.
fn texts_of(
    col: &Collection,
    v: &Variable,
    by_id: &HashMap<&str, &Variable>,
) -> Vec<(String, Option<String>)> {
    col.modes
        .iter()
        .map(|m| match v.values.get(&m.id) {
            Some(Bind::Lit(x)) => (value_text(x), None),
            Some(Bind::Var(r)) => (
                by_id
                    .get(r.var.as_str())
                    .map_or_else(|| r.var.clone(), |t| t.name.clone()),
                Some(r.var.clone()),
            ),
            None => (String::new(), None),
        })
        .collect()
}

/// The fields of `n` that `v` fits, each with whether it is bound to `v` now.
fn binds_of(n: &Node, v: &Variable) -> Vec<BindOption> {
    let fields: &[BindField] = match v.kind {
        VarType::Color => &[BindField::Fill],
        VarType::Number => &[BindField::Radius, BindField::Opacity],
    };
    let bound = node_bindings(n);
    fields
        .iter()
        .filter(|f| bindable(n.kind, **f))
        .map(|f| BindOption {
            field: *f,
            bound: bound
                .iter()
                .any(|b| b.field == field_name(*f) && b.var == v.id),
        })
        .collect()
}

/// How many node fields and aliases use each variable, in one pass over the file.
fn use_counts<'a>(nodes: &'a [Node], vars: &'a Variables) -> HashMap<&'a str, usize> {
    let mut uses: HashMap<&str, usize> = HashMap::new();
    for v in &vars.variables {
        for b in v.values.values() {
            if let Some(id) = b.var_id() {
                *uses.entry(id).or_default() += 1;
            }
        }
    }
    for n in nodes {
        for b in node_bindings(n) {
            *uses.entry(b.var).or_default() += 1;
        }
    }
    uses
}
