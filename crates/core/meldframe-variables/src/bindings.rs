use meldframe_protocol::{Bind, Node, NodePatch, VarType, Variables};

/// A node field that is bound to a variable: the field's JSON name, the variable's id and the type the
/// variable must have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding<'a> {
    pub field: &'static str,
    pub var: &'a str,
    pub expects: VarType,
}

fn push<'a, T>(out: &mut Vec<Binding<'a>>, field: &'static str, v: &'a Bind<T>, expects: VarType) {
    if let Bind::Var(r) = v {
        out.push(Binding {
            field,
            var: &r.var,
            expects,
        });
    }
}

/// The bound fields of a node. `fill` takes a colour variable; `radius` and `opacity` take a number variable.
pub fn node_bindings(n: &Node) -> Vec<Binding<'_>> {
    let mut out = Vec::new();
    push(&mut out, "fill", &n.fill, VarType::Color);
    push(&mut out, "radius", &n.radius, VarType::Number);
    push(&mut out, "opacity", &n.opacity, VarType::Number);
    out
}

/// The fields a patch binds.
pub fn patch_bindings(p: &NodePatch) -> Vec<Binding<'_>> {
    let mut out = Vec::new();
    if let Some(v) = &p.fill {
        push(&mut out, "fill", v, VarType::Color);
    }
    if let Some(v) = &p.radius {
        push(&mut out, "radius", v, VarType::Number);
    }
    if let Some(v) = &p.opacity {
        push(&mut out, "opacity", v, VarType::Number);
    }
    out
}

/// What uses variable `id`: the ids of the variables that alias it (in any mode) and of the nodes whose
/// fields are bound to it, as `(aliases, nodes)`.
pub fn users(nodes: &[Node], vars: &Variables, id: &str) -> (Vec<String>, Vec<String>) {
    (aliases_of(vars, id), nodes_bound_to(nodes, id))
}

fn aliases_of(vars: &Variables, id: &str) -> Vec<String> {
    vars.variables
        .iter()
        .filter(|v| v.values.values().any(|b| b.var_id() == Some(id)))
        .map(|v| v.id.clone())
        .collect()
}

fn nodes_bound_to(nodes: &[Node], id: &str) -> Vec<String> {
    nodes
        .iter()
        .filter(|n| node_bindings(n).iter().any(|b| b.var == id))
        .map(|n| n.id.clone())
        .collect()
}
