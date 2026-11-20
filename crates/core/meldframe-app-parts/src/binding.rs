//! The edit of a variable's value and the bind of a field, the parts that read no `App`: what a value becomes on the
//! variable's type, and the patch that binds a field or lets it go with the value that is painted.

use crate::variables_panel::{bindable, field_name};
use meldframe_app_model::{BindField, VariableValue};
use meldframe_protocol::{Bind, Node, NodePatch, Value, VarType, Variables};
use meldframe_render::Fields;
use meldframe_variables::{ModeChoice, node_bindings};

/// What the value of a variable in one mode becomes, or the code of the refusal.
pub fn variable_bind(kind: VarType, value: VariableValue) -> Result<Bind<Value>, &'static str> {
    Ok(match value {
        VariableValue::Alias { variable: to } => Bind::var(to),
        VariableValue::Literal { text } => match kind {
            VarType::Color => Bind::Lit(Value::Color(text.trim().to_string())),
            VarType::Number => match text.trim().parse::<f64>() {
                Ok(n) => Bind::Lit(Value::Number(n)),
                Err(_) => return Err("invalid_value"),
            },
        },
    })
}

/// The patch that binds a field of a node to a variable, or lets it go with the value that is painted. None when the
/// field is so already (the same variable again, or a field that is a literal); the code of the refusal when the field
/// cannot be bound on this kind of node.
pub fn bind_patch(
    node: &Node,
    field: BindField,
    variable: Option<String>,
    vars: &Variables,
    choice: &ModeChoice,
) -> Result<Option<NodePatch>, &'static str> {
    if !bindable(node.kind, field) {
        return Err("not_bindable");
    }
    let bound = node_bindings(node)
        .into_iter()
        .find(|b| b.field == field_name(field))
        .map(|b| b.var.to_string());
    if bound == variable {
        return Ok(None);
    }
    let mut patch = NodePatch::default();
    match variable {
        Some(v) => match field {
            BindField::Fill => patch.fill = Some(Bind::var(v)),
            BindField::Radius => patch.radius = Some(Bind::var(v)),
            BindField::Opacity => patch.opacity = Some(Bind::var(v)),
        },
        None => {
            let f = Fields::new(vars, choice);
            match field {
                BindField::Fill => patch.fill = Some(Bind::Lit(f.fill_text(&node.fill))),
                BindField::Radius => patch.radius = Some(Bind::Lit(f.radius(&node.radius))),
                BindField::Opacity => patch.opacity = Some(Bind::Lit(f.opacity(&node.opacity))),
            }
        }
    }
    Ok(Some(patch))
}
