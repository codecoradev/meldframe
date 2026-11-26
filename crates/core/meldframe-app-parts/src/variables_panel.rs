//! The view of the Variables panel (E3.4), built from the variables, the nodes, the modes, the selection and the
//! search: a pure function with no state. The cache that decides when to build it again, and the actions that edit a
//! value or bind a field, stay in `meldframe-app`.

use meldframe_app_model::{
    BindField, BindOption, CollectionRow, ModeValue, TypeHeading, VariableRow, VariablesPanel,
};
use meldframe_protocol::{
    Bind, Collection, Mode, Node, NodeKind, Value, VarType, Variable, Variables,
};
use meldframe_text::{MAX_LABEL, label};
use meldframe_variables::{ModeChoice, Resolver, node_bindings};
use std::collections::HashMap;

/// The most variables the panel lists at once; `VariablesPanel::matched` says how many matched.
const MAX_LISTED: usize = 200;

/// Whether a field of a node of this kind can hold a binding the panel offers: text and an ellipse have no corners to set.
pub fn bindable(kind: NodeKind, field: BindField) -> bool {
    // The canvas draws no corners on text, and an ellipse takes its radius from its size.
    !(matches!(kind, NodeKind::Text | NodeKind::Ellipse) && field == BindField::Radius)
}

pub fn field_name(field: BindField) -> &'static str {
    match field {
        BindField::Fill => "fill",
        BindField::Radius => "radius",
        BindField::Opacity => "opacity",
    }
}

fn number_text(v: f64) -> String {
    format!("{v}")
}

pub fn value_text(v: &Value) -> String {
    match v {
        Value::Color(css) => css.clone(),
        Value::Number(n) => number_text(*n),
    }
}

pub fn bound_fields(nodes: &[Node]) -> Vec<(String, &'static str, String)> {
    nodes
        .iter()
        .flat_map(|n| {
            node_bindings(n)
                .into_iter()
                .map(|b| (n.id.clone(), b.field, b.var.to_string()))
        })
        .collect()
}

/// The panel the variables of the document make, as the person sees it with `choice` and `selected` and the search `query`.
pub fn variables_view(
    vars: &Variables,
    nodes: &[Node],
    choice: &ModeChoice,
    selected: Option<&Node>,
    query: &str,
) -> VariablesPanel {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let by_id: HashMap<&str, &Variable> =
        vars.variables.iter().map(|v| (v.id.as_str(), v)).collect();
    let uses = use_counts(nodes, vars);
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
        let (mut in_col, mut rows) = (0, Vec::new());
        for v in vars.variables.iter().filter(|v| v.collection == col.id) {
            // The words are looked for in the texts only when there are words; a row is built only when listed.
            let mut texts = None;
            if !words.is_empty() {
                let t = texts_of(col, v, &by_id);
                let hay = format!(
                    "{} {} {}",
                    v.name,
                    col.name,
                    t.iter()
                        .map(|(x, _)| x.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
                .to_lowercase();
                if !words.iter().all(|w| hay.contains(w)) {
                    continue;
                }
                texts = Some(t);
            }
            matched += 1;
            in_col += 1;
            if listed >= MAX_LISTED {
                continue;
            }
            listed += 1;
            let texts = texts.unwrap_or_else(|| texts_of(col, v, &by_id));
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
                name: label(&v.name, MAX_LABEL),
                kind: v.kind,
                values,
                used_by: uses.get(v.id.as_str()).copied().unwrap_or(0),
                binds: selected.map(|n| binds_of(n, v)).unwrap_or_default(),
                heading: None,
            });
        }
        group_by_type(&mut rows);
        if in_col > 0 || words.is_empty() {
            let active = choice
                .get(&col.id)
                .filter(|m| col.modes.iter().any(|x| x.id == *m))
                .or_else(|| col.modes.first().map(|m| m.id.as_str()))
                .unwrap_or_default();
            collections.push(CollectionRow {
                id: col.id.clone(),
                name: label(&col.name, MAX_LABEL),
                modes: col.modes.iter().map(shown_mode).collect(),
                active: active.to_string(),
                matched: in_col,
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

/// A mode as the panel lists it: its name made safe to show; the id is never cleaned.
fn shown_mode(m: &Mode) -> Mode {
    Mode {
        id: m.id.clone(),
        name: label(&m.name, MAX_LABEL),
    }
}

/// List the rows by type, colours first (the order inside a type stays the file's), and put the heading on the first row
/// of each type.
fn group_by_type(rows: &mut [VariableRow]) {
    rows.sort_by_key(|r| matches!(r.kind, VarType::Number));
    let mut at = 0;
    while at < rows.len() {
        let kind = rows[at].kind;
        let count = rows[at..].iter().take_while(|r| r.kind == kind).count();
        rows[at].heading = Some(TypeHeading { kind, count });
        at += count;
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
                label(
                    by_id
                        .get(r.var.as_str())
                        .map_or(r.var.as_str(), |t| &t.name),
                    MAX_LABEL,
                ),
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

/// How many node fields and aliases use each variable, by id, on the whole file.
pub fn use_counts<'a>(nodes: &'a [Node], vars: &'a Variables) -> HashMap<&'a str, usize> {
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
