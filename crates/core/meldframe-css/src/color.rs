//! CSS colours to the hex the model stores. A colour that is already hex is kept as written; every other form is
//! converted to sRGB and rounded to 8 bits, and the answer says whether the conversion had to clip a colour that sRGB
//! cannot show (`oklch` and `oklab` reach past it). Nothing here evaluates anything: it reads numbers.

use crate::number::number_exp;

/// A colour as the model stores it, and how it got there.
#[derive(Debug, Clone, PartialEq)]
pub struct Hex {
    pub hex: String,
    /// The colour was outside sRGB and was clamped channel by channel.
    pub clipped: bool,
}

fn exact(hex: String) -> Hex {
    Hex {
        hex,
        clipped: false,
    }
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
pub fn is_hex(s: &str) -> bool {
    s.strip_prefix('#').is_some_and(|h| {
        matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

const NAMED: [(&str, &str); 19] = [
    ("transparent", "#00000000"),
    ("black", "#000000"),
    ("silver", "#c0c0c0"),
    ("gray", "#808080"),
    ("grey", "#808080"),
    ("white", "#ffffff"),
    ("maroon", "#800000"),
    ("red", "#ff0000"),
    ("purple", "#800080"),
    ("fuchsia", "#ff00ff"),
    ("green", "#008000"),
    ("lime", "#00ff00"),
    ("olive", "#808000"),
    ("yellow", "#ffff00"),
    ("navy", "#000080"),
    ("blue", "#0000ff"),
    ("teal", "#008080"),
    ("aqua", "#00ffff"),
    ("orange", "#ffa500"),
];

/// The hex of a colour keyword the table knows.
pub fn named(s: &str) -> Option<Hex> {
    let k = s.to_ascii_lowercase();
    NAMED
        .iter()
        .find(|(n, _)| *n == k)
        .map(|(_, h)| exact((*h).to_string()))
}

fn byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn pack(r: f64, g: f64, b: f64, a: f64, clipped: bool) -> Hex {
    let (r, g, b, a) = (byte(r), byte(g), byte(b), byte(a));
    let hex = if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    };
    Hex { hex, clipped }
}

/// One number of a colour function: a plain number, or a percentage that is `pct_of` at 100%. `none` is 0.
fn num(s: &str, pct_of: f64) -> Option<f64> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    let v = match s.strip_suffix('%') {
        Some(p) => number_exp(p)? / 100.0 * pct_of,
        None => number_exp(s)?,
    };
    v.is_finite().then_some(v)
}

/// A saturation or a lightness: a percentage, or a bare number that is in percent points (`hsl(120 50 50)`), as a
/// fraction.
fn point(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    number_exp(s.strip_suffix('%').unwrap_or(s)).map(|v| v / 100.0)
}

/// The largest hue read, in the unit it is written in: beyond it the angle is not a colour but an accident.
const MAX_HUE: f64 = 1.0e6;

/// A hue in degrees: a number, or a number with `deg`, `rad`, `grad` or `turn`.
fn hue(s: &str) -> Option<f64> {
    let s = s.trim().to_ascii_lowercase();
    let (n, unit) = if let Some(n) = s.strip_suffix("deg") {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix("grad") {
        (n, 0.9)
    } else if let Some(n) = s.strip_suffix("rad") {
        (n, 180.0 / std::f64::consts::PI)
    } else if let Some(n) = s.strip_suffix("turn") {
        (n, 360.0)
    } else if s == "none" {
        ("0", 1.0)
    } else {
        (s.as_str(), 1.0)
    };
    let v = number_exp(n)?;
    (v.abs() <= MAX_HUE).then(|| (v * unit).rem_euclid(360.0))
}

/// The arguments of `name(...)` split on commas or spaces, and the alpha after a comma or a slash.
fn args(body: &str) -> Option<(Vec<&str>, Option<&str>)> {
    let (main, alpha) = match body.split_once('/') {
        Some((m, a)) => (m, Some(a.trim())),
        None => (body, None),
    };
    let parts: Vec<&str> = if main.contains(',') {
        main.split(',').map(str::trim).collect()
    } else {
        main.split_whitespace().collect()
    };
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    if alpha.is_none() && main.contains(',') && parts.len() == 4 {
        return Some((parts[..3].to_vec(), Some(parts[3])));
    }
    if alpha.is_none() && !main.contains(',') && parts.len() == 4 {
        return None;
    }
    Some((parts, alpha))
}

fn alpha(a: Option<&str>) -> Option<f64> {
    a.map_or(Some(1.0), |a| num(a, 1.0))
        .map(|v| v.clamp(0.0, 1.0))
}

fn srgb_encode(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// The encoding of a linear channel that keeps its sign, so that a channel below zero is measured as a colour does.
fn encode_signed(c: f64) -> f64 {
    if c < 0.0 {
        -srgb_encode(-c)
    } else {
        srgb_encode(c)
    }
}

/// Oklab to sRGB, clamped. The flag is set when a channel, as encoded, was more than half a step of 8 bits outside 0 to
/// 1. `None` when the arithmetic did not stay finite (a lightness of 1e103 overflows), which is not a colour.
fn oklab(l: f64, a: f64, b: f64, alpha: f64) -> Option<Hex> {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let (l, m, s) = (l_.powi(3), m_.powi(3), s_.powi(3));
    let lin = [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ];
    if lin.iter().any(|c| !c.is_finite()) {
        return None;
    }
    let enc = lin.map(encode_signed);
    let eps = 0.5 / 255.0;
    let clipped = enc.iter().any(|c| *c < -eps || *c > 1.0 + eps);
    Some(pack(enc[0], enc[1], enc[2], alpha, clipped))
}

fn hsl(h: f64, s: f64, l: f64, alpha: f64) -> Hex {
    let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    let k = |n: f64| (n + h / 30.0).rem_euclid(12.0);
    let f = |n: f64| {
        let a = s * l.min(1.0 - l);
        l - a * (k(n) - 3.0).min(9.0 - k(n)).clamp(-1.0, 1.0)
    };
    pack(f(0.0), f(8.0), f(4.0), alpha, false)
}

/// A colour function, `rgb(...)`, `rgba(...)`, `hsl(...)`, `hsla(...)`, `oklch(...)` or `oklab(...)`, as hex. `None`
/// when `s` is not one of them or a part does not read as a number.
pub fn function(s: &str) -> Option<Hex> {
    let s = s.trim();
    let open = s.find('(')?;
    let body = s[open + 1..].strip_suffix(')')?;
    if body.contains('(') {
        return None;
    }
    let name = s[..open].trim().to_ascii_lowercase();
    let (p, a) = args(body)?;
    let a = alpha(a)?;
    match (name.as_str(), p.len()) {
        ("rgb" | "rgba", 3) => {
            let c = |x: &str| num(x, 255.0).map(|v| v / 255.0);
            Some(pack(c(p[0])?, c(p[1])?, c(p[2])?, a, false))
        }
        ("hsl" | "hsla", 3) => Some(hsl(hue(p[0])?, point(p[1])?, point(p[2])?, a)),
        ("oklch", 3) => {
            // CSS clamps a negative chroma to 0: the colour is grey, not the opposite hue.
            let (l, c, h) = (num(p[0], 1.0)?, num(p[1], 0.4)?.max(0.0), hue(p[2])?);
            let h = h.to_radians();
            oklab(l, c * h.cos(), c * h.sin(), a)
        }
        ("oklab", 3) => oklab(num(p[0], 1.0)?, num(p[1], 0.4)?, num(p[2], 0.4)?, a),
        _ => None,
    }
}

/// shadcn's channel form, `0 0% 100%` or `222.2 84% 4.9% / 0.5`: three bare HSL channels, as the v3 themes write them
/// for `hsl(var(--x))`.
pub fn channels(s: &str) -> Option<Hex> {
    let (main, a) = match s.split_once('/') {
        Some((m, a)) => (m, Some(a.trim())),
        None => (s, None),
    };
    let p: Vec<&str> = main.split_whitespace().collect();
    if p.len() != 3 || !p[1].ends_with('%') || !p[2].ends_with('%') {
        return None;
    }
    if p[0].parse::<f64>().is_err() && !p[0].ends_with("deg") {
        return None;
    }
    Some(hsl(hue(p[0])?, point(p[1])?, point(p[2])?, alpha(a)?))
}
