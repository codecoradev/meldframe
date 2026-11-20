//! The rules of the properties panel that are a function of the node: what each `Property` does to it, as the patch of one
//! `UpdateNode` (one undo step). The Fill section's rules live here: what a change of type does to `fill`, `gradient`
//! and `image`, and how the stops of a gradient are edited. The checks of a colour, of the stops and of a size are the
//! core's, run when the command is applied.

use crate::properties::fill_kind;
use meldframe_app_model::{FillKind, Property};
use meldframe_check::Limits;
use meldframe_protocol::{
    Bind, Border, DEFAULT_FILL, Gradient, GradientKind, GradientStop, Node, NodeKind, NodePatch,
    Shadow,
};

/// The fewest stops of a gradient (the core refuses fewer as `E_INVALID_GRADIENT`); the most is the core's limit.
pub const MIN_STOPS: usize = 2;

/// The patch one `Property` makes on a node, `None` when there is nothing to change, or the code of a refusal. `painted` is
/// the colour the fill paints now (a bound fill resolved), the first stop of a gradient made from it.
pub fn property_patch(
    node: &Node,
    property: Property,
    painted: &str,
) -> Result<Option<NodePatch>, &'static str> {
    // Only a frame clips: the canvas draws nothing else that way, so other kinds refuse the field.
    if matches!(property, Property::Clip(_)) && node.kind != NodeKind::Frame {
        return Err("invalid");
    }
    let mut patch = NodePatch::default();
    match property {
        Property::Name(v) => patch.name = Some(v),
        Property::Fill(v) => patch.fill = Some(v.into()),
        Property::Radius(v) => patch.radius = Some(v.into()),
        Property::FontSize(v) => patch.font_size = Some(v),
        Property::Opacity(v) => patch.opacity = Some(v.into()),
        Property::Clip(v) => patch.clip = Some(v),
        Property::FillType(kind) => {
            if fill_kind(node) == kind {
                // Already so: nothing to change, and nothing to refuse (also for `Image` on a node that has one).
                return Ok(None);
            }
            match kind {
                // The node has no image (it would be of this type already): there is nothing to show.
                FillKind::Image => return Err("no_image"),
                FillKind::Linear | FillKind::Radial if node.kind == NodeKind::Text => {
                    return Err("gradient_on_text");
                }
                _ => patch = fill_type_patch(node, kind, painted),
            }
        }
        Property::GradientAngle(a) => {
            let Some(mut g) = node.gradient.clone() else {
                return Err("no_gradient");
            };
            if !a.is_finite() {
                return Err("invalid");
            }
            g.angle = a.rem_euclid(360.0);
            patch.gradient = Some(Some(g));
        }
        Property::GradientStop { index, pos, color } => {
            let Some(mut g) = node.gradient.clone() else {
                return Err("no_gradient");
            };
            if index >= g.stops.len() {
                return Err("not_found");
            }
            if !pos.is_finite() {
                return Err("invalid");
            }
            // The stops may not go back: a stop stays between its neighbours.
            let lo = if index == 0 {
                0.0
            } else {
                g.stops[index - 1].pos
            };
            let hi = g.stops.get(index + 1).map_or(1.0, |s| s.pos);
            g.stops[index] = GradientStop {
                pos: pos.clamp(lo, hi),
                color,
            };
            patch.gradient = Some(Some(g));
        }
        Property::AddGradientStop {} => {
            let Some(mut g) = node.gradient.clone() else {
                return Err("no_gradient");
            };
            if g.stops.len() >= Limits::default().max_gradient_stops {
                return Err("limit");
            }
            // The widest gap between two stops (the first one on a tie), and the middle of it.
            let at = (0..g.stops.len().saturating_sub(1))
                .fold((0, -1.0), |best, i| {
                    let gap = g.stops[i + 1].pos - g.stops[i].pos;
                    if gap > best.1 { (i, gap) } else { best }
                })
                .0;
            let (a, b) = (&g.stops[at], &g.stops[at + 1]);
            let stop = GradientStop {
                pos: (a.pos + b.pos) / 2.0,
                color: a.color.clone(),
            };
            g.stops.insert(at + 1, stop);
            patch.gradient = Some(Some(g));
        }
        Property::RemoveGradientStop(index) => {
            let Some(mut g) = node.gradient.clone() else {
                return Err("no_gradient");
            };
            if index >= g.stops.len() {
                return Err("not_found");
            }
            if g.stops.len() <= MIN_STOPS {
                return Err("min_stops");
            }
            g.stops.remove(index);
            patch.gradient = Some(Some(g));
        }
        Property::BorderWidth(width) => {
            patch.border = Some(Border {
                width,
                ..node.border.clone()
            })
        }
        Property::BorderColor(color) => {
            patch.border = Some(Border {
                color,
                ..node.border.clone()
            })
        }
        Property::BorderPosition(position) => {
            patch.border = Some(Border {
                position,
                ..node.border.clone()
            })
        }
        Property::SetShadow { index, shadow } => {
            let mut list = node.shadows.clone();
            let Some(slot) = list.get_mut(index) else {
                return Err("not_found");
            };
            *slot = shadow;
            patch.shadows = Some(list);
        }
        Property::AddShadow {} => {
            let mut list = node.shadows.clone();
            if list.len() >= Limits::default().max_shadows {
                return Err("limit");
            }
            // A soft drop shadow, the one the Shadow board starts from.
            list.push(Shadow {
                x: 0.0,
                y: 4.0,
                blur: 8.0,
                spread: 0.0,
                color: "#00000040".into(),
            });
            patch.shadows = Some(list);
        }
        Property::RemoveShadow(index) => {
            let mut list = node.shadows.clone();
            if index >= list.len() {
                return Err("not_found");
            }
            list.remove(index);
            patch.shadows = Some(list);
        }
    }
    Ok(Some(patch))
}

/// The change a type switch makes to a node that is not of that type yet (`Image` is the caller's: there is none to make).
fn fill_type_patch(n: &Node, kind: FillKind, painted: &str) -> NodePatch {
    let mut p = NodePatch::default();
    match kind {
        FillKind::Image => {}
        FillKind::None => {
            p.fill = Some("none".to_string().into());
            p.gradient = Some(None);
            p.image = Some(None);
        }
        FillKind::Solid => {
            if matches!(&n.fill, Bind::Lit(s) if s == "none") {
                p.fill = Some(DEFAULT_FILL.to_string().into());
            }
            p.gradient = Some(None);
            p.image = Some(None);
        }
        FillKind::Linear | FillKind::Radial => {
            let shape = if kind == FillKind::Linear {
                GradientKind::Linear
            } else {
                GradientKind::Radial
            };
            let g = match &n.gradient {
                Some(g) => Gradient {
                    kind: shape,
                    ..g.clone()
                },
                None => {
                    let start = if painted.starts_with('#') {
                        painted.to_string()
                    } else {
                        DEFAULT_FILL.to_string()
                    };
                    Gradient {
                        kind: shape,
                        angle: 180.0,
                        stops: vec![
                            GradientStop {
                                pos: 0.0,
                                color: start,
                            },
                            GradientStop {
                                pos: 1.0,
                                color: "#ffffff".into(),
                            },
                        ],
                    }
                }
            };
            p.gradient = Some(Some(g));
            p.image = Some(None);
        }
    }
    p
}
