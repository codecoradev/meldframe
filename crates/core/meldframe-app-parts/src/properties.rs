//! The editable values of a node as the properties panel shows them.

use meldframe_app_model::{BindField, FieldBinding, Properties};
use meldframe_protocol::{Node, Variables};
use meldframe_render::Fields;
use meldframe_variables::{ModeChoice, node_bindings};

/// The editable values of a node as they are painted: a bound field shows what the render step resolves it to
/// (`Fields` is the one place that does it, so the panel and the canvas agree, unresolved defaults included).
pub fn properties(vars: &Variables, choice: &ModeChoice, n: &Node) -> Properties {
    let fields = Fields::new(vars, choice);
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
            let found = vars.variables.iter().find(|v| v.id == variable);
            Some(FieldBinding {
                field,
                name: found.map_or_else(|| variable.clone(), |v| v.name.clone()),
                missing: found.is_none(),
                variable,
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
