//! Which node fields each consumer of the render model renders (the coverage table of E3.1).
//!
//! One row per field of `meldframe_protocol::Node`, by its JSON name. A consumer either renders a
//! field or is marked unsupported with a reason; the tests in `tests/coverage.rs` fail when a field
//! has no row, when a row marked `Rendered` paints nothing different, and when a row marked
//! unsupported does change what is painted. A new node field therefore needs a row (and, for a
//! rendered one, a change in `resolve`) in the same pull request.
//!
//! The canvas column is proven against `resolve`, in this crate. The export column is proven against the
//! export, in the tests of the `export` crate (a feature crate cannot be a dependency of a core one): a field
//! marked `Rendered` there changes the export when it changes, and one marked unsupported does not. A field that
//! one consumer renders and the other does not carries a reason on the side that does not.

use meldframe_protocol::VarType;

/// Whether a consumer renders a node field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Rendered,
    /// Not rendered, and the reason why.
    Unsupported(&'static str),
}

/// The support of one node field by each consumer.
#[derive(Debug, Clone, Copy)]
pub struct FieldSupport {
    pub field: &'static str,
    pub canvas: Support,
    pub export: Support,
    /// The type of variable the field can be bound to (E3.3), or `None` when it takes a literal only. A test
    /// compares these rows with `meldframe_variables::node_bindings` and checks that a bound field paints what
    /// the literal of its resolved value paints.
    pub bindable: Option<VarType>,
}

const FROM_LAYOUT: Support = Support::Unsupported(
    "an input of layout; resolve reads the stored box, so the canvas does not apply it yet",
);
const NOT_DRAWN: Support = Support::Unsupported("the canvas does not draw it yet");

const fn row(field: &'static str, canvas: Support) -> FieldSupport {
    FieldSupport {
        field,
        canvas,
        export: Support::Rendered,
        bindable: None,
    }
}

/// A row for a field that a variable of type `to` can be bound to.
const fn bound(field: &'static str, canvas: Support, to: VarType) -> FieldSupport {
    FieldSupport {
        bindable: Some(to),
        ..row(field, canvas)
    }
}

const R: Support = Support::Rendered;

/// Every field of a node and who renders it.
pub const NODE_FIELDS: &[FieldSupport] = &[
    row("id", R),
    row("name", R),
    row("type", R),
    row("parentId", R),
    row("pos", R),
    row("x", R),
    row("y", R),
    row("w", R),
    row("h", R),
    row("sw", FROM_LAYOUT),
    row("sh", FROM_LAYOUT),
    bound("fill", R, VarType::Color),
    row(
        "fill2",
        Support::Unsupported(
            "only the shader uses it, and the canvas draws a shader as its plain box",
        ),
    ),
    bound("radius", R, VarType::Number),
    row("layout", FROM_LAYOUT),
    row("gap", FROM_LAYOUT),
    row("padding", FROM_LAYOUT),
    row("padT", FROM_LAYOUT),
    row("padR", FROM_LAYOUT),
    row("padB", FROM_LAYOUT),
    row("padL", FROM_LAYOUT),
    row("align", FROM_LAYOUT),
    row("justify", FROM_LAYOUT),
    row("clip", R),
    row("text", R),
    row("fontSize", R),
    FieldSupport {
        export: Support::Unsupported(
            "a shader preset has no CSS form: every preset is exported as one gradient from fill to fill2, and the report says so",
        ),
        ..row(
            "shader",
            Support::Unsupported("the canvas draws a shader as its plain box"),
        )
    },
    row("fontWeight", NOT_DRAWN),
    row("textAlign", NOT_DRAWN),
    row("border", NOT_DRAWN),
    row("shadows", NOT_DRAWN),
    row("marT", FROM_LAYOUT),
    row("marR", FROM_LAYOUT),
    row("marB", FROM_LAYOUT),
    row("marL", FROM_LAYOUT),
    row("absolute", FROM_LAYOUT),
    bound("opacity", R, VarType::Number),
    row("gradient", R),
    row("image", R),
];
