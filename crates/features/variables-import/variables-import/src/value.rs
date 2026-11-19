//! What one custom property's value is: a colour, a number, an alias to another property, or something the model
//! cannot hold (and why). Text in, a description out; nothing is evaluated.

use crate::color;

/// What a value says.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    /// A colour, as hex; `how` says what was done to get it (nothing for a hex colour).
    Color {
        hex: String,
        how: Option<Conversion>,
    },
    Number {
        value: f64,
        how: Option<Conversion>,
    },
    /// `var(--target)`; `fallback` when it had a fallback value, which is not kept.
    Alias {
        target: String,
        fallback: bool,
    },
    /// Not representable: a stable reason code and a message.
    Skip(&'static str, String),
}

/// A change the import made to the text of a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conversion {
    /// `rgb()`, `hsl()`, `oklch()`, `oklab()`, a keyword or a channel triplet, to hex.
    Colour,
    /// A colour that sRGB cannot show, clamped.
    Clipped,
    /// `rem` times 16.
    Rem,
}

/// The longest value text read, in bytes; a longer one is skipped, so a hostile file cannot make a value that costs more.
pub const MAX_VALUE: usize = 1_000;

fn skip(code: &'static str, msg: impl Into<String>) -> Val {
    Val::Skip(code, msg.into())
}

/// Whether `s` has a CSS function call of this name (`calc(`, `color-mix(`...), as a word.
fn has_fn(s: &str, name: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.match_indices(name).any(|(i, _)| {
        l[i + name.len()..].starts_with('(')
            && !l[..i].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
    })
}

/// `var(--x)` or `var(--x, fallback)` and nothing else.
fn alias(s: &str) -> Option<Val> {
    let body = s
        .strip_prefix("var(")
        .or_else(|| s.strip_prefix("VAR("))?
        .strip_suffix(')')?;
    let (name, fallback) = match body.split_once(',') {
        Some((n, f)) => (n.trim(), !f.trim().is_empty()),
        None => (body.trim(), false),
    };
    let ok = name.starts_with("--")
        && name.len() > 2
        && !name.contains(|c: char| c.is_whitespace() || matches!(c, '(' | ')'));
    ok.then(|| Val::Alias {
        target: name.to_string(),
        fallback,
    })
}

fn number(s: &str) -> Option<Val> {
    let l = s.to_ascii_lowercase();
    let (n, rem) = if let Some(n) = l.strip_suffix("rem") {
        (n, true)
    } else if let Some(n) = l.strip_suffix("px") {
        (n, false)
    } else {
        (l.as_str(), false)
    };
    let digits = n.trim();
    if digits.is_empty()
        || !digits
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+'))
    {
        return None;
    }
    let v = digits.parse::<f64>().ok().filter(|v| v.is_finite())?;
    Some(if rem {
        Val::Number {
            value: v * 16.0,
            how: Some(Conversion::Rem),
        }
    } else {
        Val::Number {
            value: v,
            how: None,
        }
    })
}

/// Describe the value text of the custom property `name` (only used to word a reason).
pub fn classify(name: &str, raw: &str) -> Val {
    let s = raw.trim();
    if s.is_empty() {
        return skip("empty_value", "the value is empty");
    }
    if s.len() > MAX_VALUE {
        return skip(
            "value_too_long",
            format!("the value is over {MAX_VALUE} bytes"),
        );
    }
    if let Some(a) = alias(s) {
        return a;
    }
    if has_fn(s, "var") {
        return skip(
            "computed_value",
            "the value uses var() inside another value, which is not evaluated",
        );
    }
    if has_fn(s, "url") || has_fn(s, "image-set") {
        return skip(
            "url_skipped",
            "url() is never followed, and an image is not a variable type",
        );
    }
    for f in [
        "calc",
        "clamp",
        "min",
        "max",
        "color-mix",
        "light-dark",
        "env",
        "attr",
    ] {
        if has_fn(s, f) {
            return skip(
                "computed_value",
                format!("{f}() is computed, and a computed value is not evaluated"),
            );
        }
    }
    if color::is_hex(s) {
        return Val::Color {
            hex: s.to_string(),
            how: None,
        };
    }
    if let Some(h) = color::named(s) {
        return Val::Color {
            hex: h.hex,
            how: Some(Conversion::Colour),
        };
    }
    let head = s
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let whole = s.ends_with(')') && s.contains('(') && !head.contains(char::is_whitespace);
    let colour_fn = [
        "rgb", "rgba", "hsl", "hsla", "oklch", "oklab", "color", "lab", "lch", "hwb",
    ];
    if whole && colour_fn.contains(&head.as_str()) {
        if let Some(h) = color::function(s) {
            let how = if h.clipped {
                Conversion::Clipped
            } else {
                Conversion::Colour
            };
            return Val::Color {
                hex: h.hex,
                how: Some(how),
            };
        }
        return skip(
            "unsupported_color",
            format!(
                "{head}() is not a colour form the import converts (hex, rgb, hsl, oklch, oklab) or could not be read"
            ),
        );
    }
    if let Some(h) = color::channels(s) {
        return Val::Color {
            hex: h.hex,
            how: Some(Conversion::Colour),
        };
    }
    if let Some(n) = number(s) {
        return n;
    }
    let kind = [
        "shadow", "font", "ease", "animate", "text", "tracking", "leading",
    ]
    .iter()
    .find(|p| name.starts_with(*p));
    skip(
        "unsupported_type",
        match kind {
            Some(k) => format!("a value of the {k} family ({s:?}) has no variable type yet"),
            None => format!(
                "{s:?} is neither a colour nor a number: strings, shadows and lengths with other units have no variable type yet"
            ),
        },
    )
}
