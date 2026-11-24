//! Collections, modes and variables as CSS custom properties (ADR-0019).
//!
//! A variable the frame uses (and every variable its aliases reach) becomes `--<collection>-<name>`, with the slash
//! of the name as a hyphen. The default mode of each collection is in one `:root` rule; every other mode is a rule
//! `:root[data-mode-<collection>="<mode>"]`, and the mode picked in the options is the attribute on `<html>`. An
//! alias is `var(--other)`. All of it sits on the one element, so an alias follows the mode picked for the collection
//! it points at, as `meldframe_variables::resolve` does. A colour is written as the render model paints it (`#rrggbb`, or
//! `#rrggbbaa` when the colour has an alpha below 1); a number is unitless, and the field that reads it adds the unit. [`file`]
//! writes the same names and blocks for every variable of the file, a line each, with the colour as its stored hex,
//! lowercased.

use crate::names::{hex_colour, hex_rgba, num, slug, unique};
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

/// One rule of the variables: the selector and the declarations it holds, `--name:value` each.
pub(crate) struct Block {
    pub selector: String,
    pub decls: Vec<String>,
}

/// How a colour is written. A frame is painted by the render model, so its variables are the colour it paints (`#rrggbb`,
/// or `#rrggbbaa` with an alpha, in the canonical form); a file of variables written for a person keeps the stored hex as
/// it is, and lowercases it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Colours {
    Painted,
    Stored,
}

/// The names, the rules and the attributes of `list`, in the order of the file: the default mode of each collection
/// in the one `:root` block, every other mode in a block of its own. A block of a mode is made even when it is empty;
/// the `:root` block only when it holds a declaration.
pub(crate) fn blocks(
    vars: &Variables,
    list: &[&Variable],
    choice: &ModeChoice,
    fields: &Fields,
    colours: Colours,
    report: &mut Report,
) -> (VarCss, Vec<Block>) {
    let mut out = VarCss::default();
    let mut taken = HashSet::new();
    for v in list {
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
                let cx = Cx {
                    names: &out.names,
                    fields,
                    colours,
                };
                if let Some(text) = value_css(bind, &cx, report, v, &mode.name) {
                    decls.push(format!("{}:{text}", out.names[&v.id]));
                }
            }
            if i == 0 {
                root.extend(decls);
            } else {
                modes.push(Block {
                    selector: format!(":root[{attr}=\"{value}\"]"),
                    decls,
                });
            }
            if i > 0 && choice.get(&col.id) == Some(mode.id.as_str()) {
                out.attrs.push((attr.clone(), value));
            }
        }
    }
    let mut all = Vec::new();
    if !root.is_empty() {
        all.push(Block {
            selector: ":root".into(),
            decls: root,
        });
    }
    all.extend(modes);
    (out, all)
}

/// The variables a frame uses (and what its aliases reach), as one line per rule: what an export document carries.
pub(crate) fn build(
    vars: &Variables,
    used: &HashSet<String>,
    choice: &ModeChoice,
    fields: &Fields,
    report: &mut Report,
) -> VarCss {
    let list = closure(vars, used);
    let (mut out, all) = blocks(vars, &list, choice, fields, Colours::Painted, report);
    out.rules = all
        .iter()
        .map(|b| format!("{}{{{}}}\n", b.selector, b.decls.join(";")))
        .collect();
    out
}

/// Every variable of the file as CSS, one declaration per line (ADR-0019 naming and mode blocks): the default mode of
/// each collection in `:root`, every other mode in `:root[data-mode-<collection>="<mode>"]`. A collection with no
/// variable writes nothing. The colour is the stored hex, lowercased, alpha included.
pub(crate) fn file(vars: &Variables) -> (String, usize, usize, Report) {
    let list: Vec<&Variable> = vars.variables.iter().collect();
    let choice = ModeChoice::default();
    let fields = Fields::new(vars, &choice);
    let mut report = Report::default();
    let (_, all) = blocks(vars, &list, &choice, &fields, Colours::Stored, &mut report);
    let mut css = String::new();
    for b in &all {
        css.push_str(&b.selector);
        css.push_str(" {\n");
        for d in &b.decls {
            css.push_str("  ");
            css.push_str(&d.replacen(':', ": ", 1));
            css.push_str(";\n");
        }
        css.push_str("}\n");
    }
    let collections = vars
        .collections
        .iter()
        .filter(|c| list.iter().any(|v| v.collection == c.id))
        .count();
    (css, collections, all.len(), report)
}

/// What [`build`] needs to write one value.
struct Cx<'a> {
    names: &'a HashMap<String, String>,
    fields: &'a Fields<'a>,
    colours: Colours,
}

fn value_css(
    bind: Option<&Bind<Value>>,
    cx: &Cx<'_>,
    report: &mut Report,
    v: &Variable,
    mode: &str,
) -> Option<String> {
    let Cx {
        names,
        fields,
        colours,
    } = cx;
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
            if *colours == Colours::Stored
                && let Some(text) = hex_colour(css)
            {
                return Some(text.to_ascii_lowercase());
            }
            Some(match fields.fill_rgba(&Bind::Lit(css.clone())) {
                Some((rgb, alpha)) => hex_rgba(rgb, alpha),
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
