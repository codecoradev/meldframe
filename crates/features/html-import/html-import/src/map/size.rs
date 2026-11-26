//! Sizing (`docs/import-html.md` section 3.4): fixed, hug or fill for each axis of a box, from what its parent is and
//! what the CSS says. CSS gives the answer of the browser (a block child stretches, a flex item is as wide as its
//! content, `flex: 1` grows), and the model has the same three words; rule G1 keeps one apart: a fill under a parent
//! that hugs on that axis would be exported as a fixed size, so the box hugs.

use super::notes::Notes;
use crate::css::value::{Len, Unit};
use crate::css::{Source, Style};
use meldframe_protocol::{LayoutMode, SizeMode};

/// What a child needs to know of the frame it is in.
#[derive(Debug, Clone, Copy)]
pub(super) struct Parent {
    pub layout: LayoutMode,
    pub sw: SizeMode,
    pub sh: SizeMode,
    /// A child with an auto cross size stretches (`align-items: stretch`, the default, and block flow).
    pub stretch: bool,
    /// A flex container: `flex-grow` of a child counts.
    pub flex: bool,
    /// A flex or grid container: a child is an item of it, so an inline child is blockified and stretches.
    pub blockified: bool,
    /// The row of a table: its cells share it.
    pub cells: bool,
    /// The fixed size of the parent in pixels, for `100%`.
    pub w: Option<f64>,
    pub h: Option<f64>,
}

impl Parent {
    fn mode(&self, horizontal: bool) -> SizeMode {
        if horizontal { self.sw } else { self.sh }
    }
}

/// What a box is, as far as its size goes.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Kind {
    /// A text that does not wrap: as wide as its line.
    pub nowrap: bool,
    pub absolute: bool,
    /// `display: inline-block`, `inline-flex`: as wide as its content in a block.
    pub inline: bool,
}

/// The size of one axis: the mode, and the pixels when it is fixed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Axis {
    pub mode: SizeMode,
    pub px: Option<f64>,
}

impl Axis {
    fn hug() -> Axis {
        Axis {
            mode: SizeMode::Hug,
            px: None,
        }
    }

    fn fixed(px: f64) -> Axis {
        Axis {
            mode: SizeMode::Fixed,
            px: Some(px),
        }
    }
}

/// `flex-grow` of an element: whether it grows (any positive number does; the model shares equally).
fn grows(s: &Style, n: &mut Notes) -> bool {
    let Some(v) = s.get("flex-grow").map(str::trim) else {
        return false;
    };
    match v.parse::<f64>() {
        Ok(g) if g > 0.0 => {
            if (g - 1.0).abs() > f64::EPSILON {
                n.change("flex-grow", v, "fill with an equal share");
            }
            true
        }
        _ => false,
    }
}

/// An axis the CSS leaves to the layout: what the parent makes of it.
pub(super) fn auto(
    s: Option<&Style>,
    n: &mut Notes,
    p: &Parent,
    horizontal: bool,
    k: Kind,
) -> Axis {
    if k.absolute || p.layout == LayoutMode::None || (horizontal && k.nowrap) {
        return Axis::hug();
    }
    let main = (p.layout == LayoutMode::Row) == horizontal;
    let mode = if main {
        let grow = p.flex && s.is_some_and(|s| grows(s, n));
        if p.cells || grow {
            SizeMode::Fill
        } else if let (true, Some(l)) = (
            p.flex,
            s.and_then(|s| s.len("flex-basis")).filter(|l| l.px >= 0.0),
        ) {
            // A basis with no growing: a fixed size.
            return Axis::fixed(l.px);
        } else {
            SizeMode::Hug
        }
    } else {
        let own = s
            .and_then(|s| s.get("align-self"))
            .map(|v| v.trim().to_ascii_lowercase());
        let stretch = match own.as_deref() {
            Some("stretch") => true,
            Some("auto" | "normal") | None => p.stretch,
            Some(_) => false,
        };
        if horizontal && k.inline {
            SizeMode::Hug
        } else if stretch {
            SizeMode::Fill
        } else {
            SizeMode::Hug
        }
    };
    // Rule G1.
    if mode == SizeMode::Fill && p.mode(horizontal) == SizeMode::Hug {
        Axis::hug()
    } else {
        Axis { mode, px: None }
    }
}

/// A length that was not written in pixels is converted and says so.
pub(super) fn converted(s: &Style, n: &mut Notes, prop: &str, raw: &str, l: Len) {
    // A length of the browser's own defaults table (the `1em` of a paragraph margin) is not the author's to be told of.
    let default = s
        .props
        .get(prop)
        .is_some_and(|p| p.source == Source::UserAgent);
    if l.unit != Unit::Px && !default {
        n.change(prop, raw, &format!("{}px", l.px));
    }
}

/// `width` or `height` of an element.
pub(super) fn axis(s: &Style, n: &mut Notes, p: &Parent, horizontal: bool, k: Kind) -> Axis {
    let prop = if horizontal { "width" } else { "height" };
    let Some(raw) = s.get(prop).map(str::trim) else {
        return auto(Some(s), n, p, horizontal, k);
    };
    let low = raw.to_ascii_lowercase();
    match low.as_str() {
        "auto" => auto(Some(s), n, p, horizontal, k),
        "fit-content" | "max-content" => Axis::hug(),
        "min-content" => {
            n.change(prop, raw, "hug");
            Axis::hug()
        }
        v if v.ends_with('%') => {
            let whole = v == "100%";
            let known = if horizontal { p.w } else { p.h };
            if whole
                && p.layout != LayoutMode::None
                && p.mode(horizontal) != SizeMode::Hug
                && !k.absolute
            {
                n.change(prop, raw, "fill");
                Axis {
                    mode: SizeMode::Fill,
                    px: None,
                }
            } else if let (true, Some(px)) = (whole, known) {
                n.change(prop, raw, &format!("{px}px"));
                Axis::fixed(px)
            } else {
                n.lose(prop, raw, "refused_percent");
                auto(Some(s), n, p, horizontal, k)
            }
        }
        _ => match s.len(prop).filter(|l| l.px >= 0.0) {
            Some(l) => {
                converted(s, n, prop, raw, l);
                Axis::fixed(l.px)
            }
            None => {
                n.lose(prop, raw, "refused_unreadable");
                auto(Some(s), n, p, horizontal, k)
            }
        },
    }
}
