//! Collections, modes and variables as CSS custom properties (ADR-0019).
//!
//! A variable the frame uses (and every variable its aliases reach) becomes `--<collection>-<name>`, with the slash
//! of the name as a hyphen. The default mode of each collection is in one `:root` rule; every other mode is a rule
//! `:root[data-mode-<collection>="<mode>"]`, and the mode picked in the options is the attribute on `<html>`. An
//! alias is `var(--other)`. All of it sits on the one element, so an alias follows the mode picked for the collection
//! it points at, as `meldframe_variables::resolve` does. A colour is written as the render model paints it
//! (`#rrggbb`, the alpha dropped); a number is unitless, and the field that reads it adds the unit.

use crate::names::{hex, num, slug, unique};
use crate::report::Report;
use meldframe_protocol::{Bind, Value, Variable, Variables};
use meldframe_render::{Fields, ModeChoice};
use std::collections::{HashMap, HashSet};

/// The CSS of the variables of one export.
#[derive(Debug, Default)]
pub(crate) struct VarCss {
    /// Variable id to custom property name.
    pub names: HashMap<String, String>,
    /// The `:root` rule and the mode rules, one per line; empty when no variable is used.
    pub rules: String,
    /// The attributes of `<html>` that pick the modes.
    pub attrs: Vec<(String, String)>,
}

/// The variables of `used` and everything they alias, in the order of the file.
fn closure<'a>(vars: &'a Variables, used: &HashSet<String>) -> Vec<&'a Variable> {
    let index: HashMap<&str, &Variable> =
        vars.variables.iter().map(|v| (v.id.as_str(), v)).collect();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut todo: Vec<&str> = used.iter().map(String::as_str).collect();
    while let Some(id) = todo.pop() {
        let Some(v) = index.get(id) else { continue };
        if seen.insert(&v.id) {
            for value in v.values.values() {
                if let Bind::Var(r) = value {
                    todo.push(&r.var);
                }
            }
        }
    }
    vars.variables
        .iter()
        .filter(|v| seen.contains(v.id.as_str()))
        .collect()
}

pub(crate) fn build(
    vars: &Variables,
    used: &HashSet<String>,
    choice: &ModeChoice,
    fields: &Fields,
    report: &mut Report,
) -> VarCss {
    let list = closure(vars, used);
    let mut out = VarCss::default();
    let mut taken = HashSet::new();
    for v in &list {
        let col = vars.collection(&v.collection).map_or("x", |c| &c.name);
        let name = unique(&mut taken, format!("--{}-{}", slug(col), slug(&v.name)));
        out.names.insert(v.id.clone(), name);
    }
    let mut root = Vec::new();
    let mut modes = Vec::new();
    let mut col_names = HashSet::new();
    for col in &vars.collections {
        let members: Vec<&&Variable> = list.iter().filter(|v| v.collection == col.id).collect();
        if members.is_empty() {
            continue;
        }
        let attr = format!("data-mode-{}", unique(&mut col_names, slug(&col.name)));
        let mut mode_names = HashSet::new();
        for (i, mode) in col.modes.iter().enumerate() {
            let value = unique(&mut mode_names, slug(&mode.name));
            let mut decls = Vec::new();
            for v in &members {
                let bind = v.values.get(&mode.id).or_else(|| {
                    let default = col.modes.first()?;
                    v.values.get(&default.id)
                });
                if let Some(text) = value_css(bind, &out.names, fields, report, v, &mode.name) {
                    decls.push(format!("{}:{text}", out.names[&v.id]));
                }
            }
            if i == 0 {
                root.extend(decls);
            } else {
                modes.push(format!(
                    ":root[{attr}=\"{value}\"]{{{}}}\n",
                    decls.join(";")
                ));
            }
            if i > 0 && choice.get(&col.id) == Some(mode.id.as_str()) {
                out.attrs.push((attr.clone(), value));
            }
        }
    }
    if !root.is_empty() {
        out.rules = format!(":root{{{}}}\n{}", root.join(";"), modes.concat());
    }
    out
}

fn value_css(
    bind: Option<&Bind<Value>>,
    names: &HashMap<String, String>,
    fields: &Fields,
    report: &mut Report,
    v: &Variable,
    mode: &str,
) -> Option<String> {
    let Some(bind) = bind else {
        report.warn(
            "missing_value",
            None,
            format!(
                "variable {} has no value in mode {mode} and none in the default mode; `var({})` is not defined there",
                v.name, names[&v.id]
            ),
        );
        return None;
    };
    match bind {
        Bind::Lit(Value::Number(n)) => Some(num(*n)),
        Bind::Lit(Value::Color(css)) => {
            Some(match fields.fill(&Bind::Lit(css.clone())) {
                Some(rgb) => hex(rgb),
                None => {
                    report.warn(
                    "invalid_colour",
                    None,
                    format!("variable {} holds `{css}`, which is not a colour; exported as transparent", v.name),
                );
                    "transparent".into()
                }
            })
        }
        Bind::Var(r) => match names.get(&r.var) {
            Some(n) => Some(format!("var({n})")),
            None => {
                report.warn(
                    "unresolved_alias",
                    None,
                    format!(
                        "variable {} aliases {}, which is not in the file; left out",
                        v.name, r.var
                    ),
                );
                None
            }
        },
    }
}
