//! The editable values of a node as the properties panel shows them.

use crate::layout::{is_layout, layout_notes, sides, uniform};
use crate::panel::selected_node;
use crate::property::MIN_STOPS;
use meldframe_app_model::{BindField, FieldBinding, FillKind, Properties, Sections};
use meldframe_check::Limits;
use meldframe_protocol::{Bind, GradientKind, Node, NodeKind, Variables};
use meldframe_render::Fields;
use meldframe_text::{MAX_LABEL, label};
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

/// The box the canvas draws a node at when it reads laid-out boxes: the offset inside its parent and the size, as
/// `PaintedBox` holds them. The panel shows these instead of the stored values (the host flag `layout_on_canvas`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drawn {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The editable values of a node as they are painted: a bound field shows what the render step resolves it to
/// (`Fields` is the one place that does it, so the panel and the canvas agree, unresolved defaults included).
pub fn properties(
    vars: &Variables,
    choice: &ModeChoice,
    n: &Node,
    parent: Option<&Node>,
    drawn: Option<Drawn>,
) -> Properties {
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
                name: label(
                    found.map_or(variable.as_str(), |v| v.name.as_str()),
                    MAX_LABEL,
                ),
                missing: found.is_none(),
                variable,
            })
        })
        .collect();
    let flows = parent.is_some_and(is_layout);
    let is_text = n.kind == NodeKind::Text;
    let margin = sides(n.margin());
    let at = drawn.unwrap_or(Drawn {
        x: n.x,
        y: n.y,
        w: n.w,
        h: n.h,
    });
    Properties {
        id: n.id.clone(),
        name: label(&n.name, MAX_LABEL),
        kind: n.kind,
        x: at.x,
        y: at.y,
        w: at.w,
        h: at.h,
        fill: fields.fill_text(&n.fill),
        fill_kind: fill_kind(n),
        gradient_allowed: n.kind != NodeKind::Text,
        image_allowed: n.kind != NodeKind::Text,
        min_stops: MIN_STOPS,
        max_stops: Limits::default().max_gradient_stops,
        gradient: n.gradient.clone(),
        image: n.image.clone(),
        radius: fields.radius(&n.radius),
        border: n.border.clone(),
        shadows: n.shadows.clone(),
        max_shadows: Limits::default().max_shadows,
        layout: n.layout,
        gap: n.gap,
        padding: sides(n.pad()),
        padding_uniform: uniform(n.pad()),
        margin,
        sw: n.sw,
        sh: n.sh,
        align: n.align,
        justify: n.justify,
        absolute: n.absolute,
        in_layout: flows,
        position_locked: drawn.is_some() && flows && !n.absolute,
        notes: layout_notes(n, parent),
        sections: Sections {
            radius: matches!(n.kind, NodeKind::Frame | NodeKind::Rect | NodeKind::Shader),
            clip: n.kind == NodeKind::Frame,
            font_size: is_text,
            layout: n.kind == NodeKind::Frame,
            placement: flows
                || n.absolute
                || [margin.top, margin.right, margin.bottom, margin.left]
                    .iter()
                    .any(|v| *v > 0.0),
        },
        text: n.text.clone(),
        font_size: n.font_size,
        font_weight: is_text.then_some(n.font_weight),
        text_align: is_text.then_some(n.text_align),
        opacity: fields.opacity(&n.opacity),
        clip: n.clip,
        bound,
        bindings,
    }
}

/// The properties panel follows the selection: the values of the one node selected, with its parent's layout in view.
/// `drawn` is the box the canvas draws the selected node at when it reads laid-out boxes, `None` for the stored box.
pub fn selected_properties(
    vars: &Variables,
    choice: &ModeChoice,
    nodes: &[Node],
    selection: &[String],
    drawn: Option<Drawn>,
) -> Option<Properties> {
    let n = selected_node(selection, nodes)?;
    let parent = n
        .parent_id
        .as_deref()
        .and_then(|p| nodes.iter().find(|x| x.id == p));
    Some(properties(vars, choice, n, parent, drawn))
}
