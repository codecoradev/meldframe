//! The editable values of a node as the properties panel shows them.

use crate::property::MIN_STOPS;
use meldframe_app_model::{BindField, FieldBinding, FillKind, Properties};
use meldframe_check::Limits;
use meldframe_protocol::{Bind, GradientKind, Node, NodeKind, Variables};
use meldframe_render::Fields;
use meldframe_variables::{ModeChoice, node_bindings};

/// The type of the fill a node shows: an image is on top, then a gradient, then the colour.
pub fn fill_kind(n: &Node) -> FillKind {
    if n.image.is_some() {
        FillKind::Image
    } else if let Some(g) = &n.gradient {
        match g.kind {
            GradientKind::Linear => FillKind::Linear,
            GradientKind::Radial => FillKind::Radial,
        }
    } else if matches!(&n.fill, Bind::Lit(s) if s == "none") {
        FillKind::None
    } else {
        FillKind::Solid
    }
}

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
        fill_kind: fill_kind(n),
        gradient_allowed: n.kind != NodeKind::Text,
        min_stops: MIN_STOPS,
        max_stops: Limits::default().max_gradient_stops,
        gradient: n.gradient.clone(),
        image: n.image.clone(),
        radius: fields.radius(&n.radius),
        border: n.border.clone(),
        shadows: n.shadows.clone(),
        max_shadows: Limits::default().max_shadows,
        text: n.text.clone(),
        font_size: n.font_size,
        opacity: fields.opacity(&n.opacity),
        clip: n.clip,
        bound,
        bindings,
    }
}
