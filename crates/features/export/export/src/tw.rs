//! A declaration as a Tailwind v4 utility. Where the contract names a utility it is used; every other declaration is
//! an arbitrary property (`[prop:value]`), which is exact (contract, section 4: arbitrary values everywhere).

use crate::decl::Decl;

/// An arbitrary value: a space is `_`, a literal underscore is `\_`.
fn arb(v: &str) -> String {
    v.replace('_', "\\_").replace(' ', "_")
}

/// The class of one declaration; on `::after` it takes the `after:` variant.
pub(crate) fn class(d: &Decl, pseudo: bool) -> String {
    let u = utility(d.prop, &d.value);
    if pseudo { format!("after:{u}") } else { u }
}

fn utility(prop: &str, value: &str) -> String {
    let v = arb(value);
    let fixed = match (prop, value) {
        ("position", _) => Some(value),
        ("display", "flex") => Some("flex"),
        ("flex-direction", "row") => Some("flex-row"),
        ("flex-direction", "column") => Some("flex-col"),
        ("flex-shrink", "0") => Some("shrink-0"),
        ("overflow", "hidden") => Some("overflow-hidden"),
        ("pointer-events", "none") => Some("pointer-events-none"),
        ("align-self", "stretch") => Some("self-stretch"),
        ("align-items", "flex-start") => Some("items-start"),
        ("align-items", "center") => Some("items-center"),
        ("align-items", "flex-end") => Some("items-end"),
        ("justify-content", "flex-start") => Some("justify-start"),
        ("justify-content", "space-between") => Some("justify-between"),
        ("text-align", "center") => Some("text-center"),
        ("text-align", "right") => Some("text-right"),
        ("white-space", "pre") => Some("whitespace-pre"),
        ("white-space", "pre-wrap") => Some("whitespace-pre-wrap"),
        ("width", "fit-content") => Some("w-fit"),
        ("width", "auto") => Some("w-auto"),
        ("height", "auto") => Some("h-auto"),
        ("content", "\"\"") => Some("content-['']"),
        _ => None,
    };
    if let Some(u) = fixed {
        return u.to_owned();
    }
    let named = match prop {
        "left" => "left-",
        "top" => "top-",
        "inset" => "inset-",
        "width" => "w-",
        "height" => "h-",
        "min-width" => "min-w-",
        "min-height" => "min-h-",
        "gap" => "gap-",
        "padding" => "p-",
        "margin" => "m-",
        "flex" => "flex-",
        "font-size" => "text-[length:",
        "font-weight" => "font-",
        "line-height" => "leading-",
        "color" => "text-[color:",
        "background-color" => "bg-[color:",
        "border-radius" => "rounded-",
        "opacity" => "opacity-",
        "box-shadow" => "shadow-",
        _ => return format!("[{prop}:{v}]"),
    };
    // A prefix that opens its own bracket (`text-[length:`) already has it.
    if named.ends_with(':') {
        format!("{named}{v}]")
    } else {
        format!("{named}[{v}]")
    }
}
