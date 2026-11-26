//! What an element declares that the importer does not carry (`docs/import-html.md` section 3.0: refused, never
//! guessed). The mapping reads a fixed set of properties; of the rest, the paint stage answers for its own
//! ([`crate::paint::handles`]), and every other declared one is listed on the node with a stable code, so that a
//! refusal is never silent. Only what the author wrote counts: the browser's defaults and what an element inherits do
//! not. The element of an export (it has `data-node`) is read in the form the export writes it: its `line-height` pin,
//! `word-break: keep-all`, `flex-shrink: 0` and `min-width` are part of that form and not refusals.

use super::notes::Notes;
use crate::css::{Source, Style};
use crate::html::Element;

/// The properties the mapping reads (each may still refuse a value of it, where it reads it).
const READ: &[&str] = &[
    "display",
    "flex-direction",
    "flex-grow",
    "flex-basis",
    "flex-wrap",
    "gap",
    "row-gap",
    "column-gap",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "align-items",
    "align-self",
    "justify-content",
    "width",
    "height",
    "position",
    "top",
    "left",
    "overflow-x",
    "overflow-y",
    "font-size",
    "font-weight",
    "text-align",
    "color",
    "white-space",
    "text-transform",
    "visibility",
    "box-sizing",
    "list-style",
    "list-style-type",
    "list-style-position",
];

/// Values that mean nothing: the property is at its initial value.
fn initial(name: &str, v: &str) -> bool {
    let zero = matches!(v, "0" | "0px");
    match name {
        "font-style" | "letter-spacing" | "word-spacing" => v == "normal" || zero,
        "text-indent" => zero,
        "text-overflow" => v == "clip",
        "text-shadow" | "transform" | "rotate" | "scale" | "translate" | "float" | "max-width"
        | "max-height" | "columns" | "object-fit" => v == "none" || v == "normal" || v == "fill",
        "clear" | "aspect-ratio" | "z-index" | "object-position" => v == "auto" || v == "0 0",
        "order" => v == "0",
        "vertical-align" => v == "baseline",
        "min-width" | "min-height" => zero || v == "auto",
        "flex-shrink" => v == "1",
        "word-break" => v == "normal",
        "line-height" => v == "normal",
        "text-decoration" | "text-decoration-line" => v == "none",
        _ => false,
    }
}

/// `Inter Variable`, the one family of the model (contract section 6, item 1).
fn inter(v: &str) -> bool {
    let first = v.split(',').next().unwrap_or("");
    let first = first.trim().trim_matches(['\'', '"']).trim();
    first.eq_ignore_ascii_case("inter") || first.eq_ignore_ascii_case("inter variable")
}

/// Is the declaration part of the form the export writes (the element has `data-node`)?
fn export_form(name: &str, v: &str) -> bool {
    match name {
        "line-height" => v.ends_with("px"),
        "word-break" => v == "keep-all",
        "flex-shrink" => v == "0",
        "min-width" | "min-height" => true,
        _ => false,
    }
}

/// List the declarations of `s` that are neither read nor answered by the paint stage.
pub(super) fn scan(s: &Style, el: &Element, n: &mut Notes) {
    let exported = el.attr("data-node").is_some();
    for (name, prop) in s.declared() {
        if !matches!(prop.source, Source::Author | Source::Inline) {
            continue;
        }
        if READ.contains(&name) || crate::paint::handles(name) {
            continue;
        }
        let v = prop.value.trim();
        let low = v.to_ascii_lowercase();
        if initial(name, &low) || (exported && export_form(name, &low)) {
            continue;
        }
        if name == "font-family" && inter(v) {
            continue;
        }
        let code = if name.starts_with("grid") {
            "refused_grid"
        } else {
            "refused_not_modelled"
        };
        n.lose(name, v, code);
    }
}
