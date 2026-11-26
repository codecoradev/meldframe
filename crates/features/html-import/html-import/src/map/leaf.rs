//! The boxes that are not a frame with children: a text, and the rect that stands for an image or for content that is
//! not carried; and the fields every box has (place, margin, visibility, clip).

use super::names;
use super::node::{Place, Spec, blank, content_box, place, set_margin, set_size, visible};
use super::notes::Notes;
use super::size::{Axis, Kind, Parent, auto};
use super::text::{Piece, TextStyle};
use super::walk::Walk;
use crate::css::Style;
use crate::html::{Element, ElementId};
use crate::paint::{self, Ctx};
use meldframe_layout::{StubMeasurer, TextMeasurer};
use meldframe_protocol::{Bind, LayoutMode, Node, NodeKind, SizeMode};

/// The text of a `px` attribute (`width="24"`), as a size.
fn attr_px(el: &Element, name: &str) -> Option<f64> {
    let v = el.attr(name)?.trim();
    let v = v.strip_suffix("px").unwrap_or(v).trim();
    v.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0)
}

/// The longest text, in characters, that is measured wrapped.
pub const WRAP_MEASURE_MAX: usize = 4_096;

fn measure(node: &Node, wrap: Option<f64>) -> (f64, f64) {
    StubMeasurer.measure(node, wrap)
}

impl Walk<'_> {
    /// An anonymous text between boxes (or the text of a frame that is not itself a text).
    pub(super) fn run_spec(
        &mut self,
        pieces: &[Piece],
        base: &Style,
        p: &Parent,
        source: &str,
    ) -> Option<Spec> {
        let ts = TextStyle::of(base);
        let text = ts.text(pieces);
        if text.is_empty() {
            return None;
        }
        let mut n = Notes::default();
        ts.report(&mut n);
        let k = Kind {
            nowrap: ts.ws.hugs(),
            ..Kind::default()
        };
        let aw = auto(None, &mut n, p, true, k);
        let ah = auto(None, &mut n, p, false, Kind::default());
        let mut node = text_node(&ts, &text);
        size_text(&mut node, aw, ah, &ts);
        node.name = names::of_text(None, &text);
        Some(Spec {
            node,
            source: format!("{source}/text"),
            notes: n,
            assets: Vec::new(),
            kids: Vec::new(),
            named: false,
        })
    }

    /// An image or another element whose content is not carried: a rect of its size.
    pub(super) fn replaced(&mut self, id: ElementId, p: &Parent, source: String) -> Spec {
        let (el, s) = (self.tree.element(id), self.styles.get(id));
        let mut n = Notes::default();
        // A size in pixels from the CSS or the attribute; one that is not (a percentage, `auto`) is listed as refused, and
        // the box has the model's size.
        let size = |prop: &str, attr: &str, n: &mut Notes| {
            if let Some(l) = s.len(prop).filter(|l| l.px >= 0.0) {
                return l.px;
            }
            let said = s
                .get(prop)
                .filter(|v| !v.trim().eq_ignore_ascii_case("auto"))
                .or_else(|| el.attr(attr).filter(|_| attr_px(el, attr).is_none()));
            if let Some(v) = said {
                n.lose(prop, v, "refused_unreadable");
            }
            attr_px(el, attr).unwrap_or_else(|| {
                // "not set" is only true when the author said nothing: an unreadable size is listed above.
                n.change(prop, said.unwrap_or("not set"), "100px");
                100.0
            })
        };
        let (w, h) = (
            size("width", "width", &mut n),
            size("height", "height", &mut n),
        );
        let ctx = Ctx {
            width: Some(w),
            height: Some(h),
            has_children: false,
        };
        let paint = self.read_paint(s, el, &ctx, &mut n);
        self.leftovers(s, el.attr("data-node").is_some(), &mut n);
        // A rect has no padding.
        for side in [
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ] {
            if s.len(side).is_some_and(|l| l.px != 0.0) {
                n.lose(
                    side,
                    s.get(side).unwrap_or_default(),
                    "refused_not_modelled",
                );
            }
        }
        let mut node = blank(NodeKind::Rect);
        paint.apply(&mut node);
        let at: Place = place(s, &mut n);
        common(&mut node, s, p, &at, &mut n);
        let room = if content_box(s) {
            2.0 * paint.border_room
        } else {
            0.0
        };
        let fixed_axis = |px: f64| Axis {
            mode: SizeMode::Fixed,
            px: Some(px),
        };
        set_size(&mut node, fixed_axis(w), fixed_axis(h), (room, room));
        let mut assets = Vec::new();
        if el.tag == "img" {
            if let Some(src) = el.attr("src") {
                assets.push(("image", paint::Asset::new(src)));
            }
        } else {
            n.lose(&el.tag, "", "element_not_carried");
        }
        assets.extend(paint.assets.iter().cloned().map(|a| ("background", a)));
        node.name = names::of_box(el);
        Spec {
            node,
            source,
            notes: n,
            assets,
            kids: Vec::new(),
            named: names::authored(el).is_some(),
        }
    }
}

pub(super) fn text_node(ts: &TextStyle, text: &str) -> Node {
    let mut node = blank(NodeKind::Text);
    node.text = text.to_owned();
    node.font_size = ts.size;
    node.font_weight = ts.weight;
    node.text_align = ts.align;
    node.fill = Bind::Lit(ts.color.clone());
    node
}

/// The stored size of a text: the authored pixels, else an estimate of the stub measure (the layout ignores it for a
/// hug or fill axis).
pub(super) fn size_text(node: &mut Node, aw: Axis, ah: Axis, ts: &TextStyle) {
    // The stub's wrap is quadratic in the length of a line, so a long text is measured as one line (an estimate
    // either way, and the layout ignores the stored size of a hug or fill axis).
    let short = node.text.chars().count() <= WRAP_MEASURE_MAX;
    let wrap = (!ts.ws.hugs() && short).then_some(aw.px).flatten();
    let (mw, mh) = measure(node, wrap);
    node.sw = aw.mode;
    node.sh = ah.mode;
    node.w = aw.px.unwrap_or(mw);
    node.h = ah.px.unwrap_or(mh);
}

/// `overflow`: hidden and clip clip; auto and scroll clip (scrolling is not modelled); visible does not. The model's
/// default is to clip, so the answer is always written.
pub(super) fn clip(s: &Style, n: &mut Notes) -> bool {
    let get = |p: &str| {
        s.get(p)
            .map_or_else(|| "visible".to_owned(), |v| v.trim().to_ascii_lowercase())
    };
    let (x, y) = (get("overflow-x"), get("overflow-y"));
    let clips = |v: &str| matches!(v, "hidden" | "clip");
    let scrolls = |v: &str| matches!(v, "auto" | "scroll");
    if x != y {
        n.change("overflow", &format!("{x} {y}"), "clip");
    }
    if scrolls(&x) || scrolls(&y) {
        n.change("overflow", &format!("{x} {y}"), "clip");
    }
    clips(&x) || clips(&y) || scrolls(&x) || scrolls(&y)
}

/// The place, the margins and the visibility every box has.
pub(super) fn common(node: &mut Node, s: &Style, p: &Parent, at: &Place, n: &mut Notes) {
    node.x = at.x;
    node.y = at.y;
    node.absolute = at.absolute && p.layout != LayoutMode::None;
    if p.layout != LayoutMode::None && !at.absolute {
        set_margin(node, s, n);
    }
    // A child of a flex container can place itself on the cross axis; the model has that on the parent only.
    if p.flex
        && let Some(v) = s.get("align-self").map(|v| v.trim().to_ascii_lowercase())
        && !matches!(v.as_str(), "auto" | "normal" | "stretch")
    {
        n.lose("align-self", &v, "refused_not_modelled");
    }
    visible(node, s, n);
}
