//! What an element declares and the mapping does not read: listed on the node, never dropped and never guessed
//! (`docs/import-html.md` sections 3.2 to 3.5). The properties the mapping reads say so in their own readers (a value
//! it cannot take is refused there); the paint stage answers for its own (`paint::handles`); the rest is refused here
//! as `refused_not_modelled`, and a `grid-*` property as `refused_grid`.

use super::item::Notes;
use crate::css::{Source, Style};
use crate::paint;
use html_import_tree::Element;

/// What a node is, for what is left of its style.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// A frame or a rect: what it declares.
    Frame,
    /// A text of an element: what it declares, and the font it inherits.
    Text,
    /// A text of the lines inside an element (the element's own properties are on its frame): the inherited font only.
    Anon,
    /// An inline element joined into a text: all it declares, but the three the text compares.
    Joined,
}

/// The properties the mapping reads (a value it cannot read is refused in the reader).
const READ: &[&str] = &[
    "display",
    "flex-direction",
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
    "flex-grow",
    "flex-basis",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "position",
    "left",
    "top",
    "right",
    "bottom",
    "overflow-x",
    "overflow-y",
    "font-size",
    "font-weight",
    "color",
    "text-align",
    "white-space",
    "text-transform",
    "box-sizing",
    "visibility",
];

/// What a joined inline element shares with the text it is in (the rest of it is lost).
const JOINED: &[&str] = &[
    "display",
    "color",
    "font-size",
    "font-weight",
    "white-space",
    "text-transform",
];

/// The properties of a text that have no field in the model; they inherit, so a text lists them where they land.
const FONT: &[&str] = &[
    "font-family",
    "font-style",
    "letter-spacing",
    "line-height",
    "text-indent",
    "word-break",
];

/// A value that changes nothing.
fn initial(v: &str) -> bool {
    matches!(
        v.to_ascii_lowercase().as_str(),
        "none" | "normal" | "auto" | "initial" | "unset" | "0" | "0px" | "static" | "visible"
    )
}

pub(crate) fn sweep(el: &Element, st: &Style, role: Role, notes: &mut Notes) {
    if role != Role::Anon {
        for (name, p) in st.declared() {
            if !matches!(p.source, Source::Author | Source::Inline) {
                continue;
            }
            let v = p.value.trim();
            let skip = match role {
                Role::Joined => JOINED.contains(&name),
                _ => {
                    let flex = match name {
                        "flex-wrap" => !v.eq_ignore_ascii_case("nowrap"),
                        "flex-shrink" => !matches!(v, "0" | "1"),
                        _ => false,
                    };
                    if flex {
                        let code = if name == "flex-wrap" {
                            "refused_not_modelled"
                        } else {
                            "refused_value"
                        };
                        notes.refuse(name, v, code);
                    }
                    READ.contains(&name)
                        || matches!(name, "flex-wrap" | "flex-shrink")
                        || paint::handles(name)
                        || FONT.contains(&name)
                }
            };
            if skip || initial(v) {
                continue;
            }
            let code = if name.starts_with("grid") {
                "refused_grid"
            } else {
                "refused_not_modelled"
            };
            notes.refuse(name, v, code);
        }
        for n in &st.notes {
            notes.refuse(&n.property, &n.value, n.code);
        }
        if let Some(b) = &st.before {
            notes.refuse(
                "::before",
                b.get("content").map_or("", |c| &c.value),
                "refused_generated_content",
            );
        }
        // The `::after` box that carries a border is the export's own form, which the paint stage reads.
        if let Some(a) = &st.after
            && !a.keys().any(|k| k.starts_with("border"))
        {
            notes.refuse(
                "::after",
                a.get("content").map_or("", |c| &c.value),
                "refused_generated_content",
            );
        }
        if matches!(el.tag.as_str(), "td" | "th") {
            for attr in ["colspan", "rowspan"] {
                if let Some(v) = el.attr(attr).filter(|v| v.trim() != "1") {
                    notes.refuse(attr, v, "refused_not_modelled");
                }
            }
        }
    }
    if matches!(role, Role::Text | Role::Anon) {
        for name in FONT {
            if let Some(v) = st
                .get(name)
                .map(str::trim)
                .filter(|v| !initial(v) && *v != "0")
            {
                notes.refuse(name, v, "refused_not_modelled");
            }
        }
    }
}
