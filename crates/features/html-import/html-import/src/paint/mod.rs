//! Paint (E4.4, `docs/import-html.md` section 3.6): the fields of a Node that are only seen, read from the computed
//! style of one element. [`read`] is a pure function of that style: no tree, no mapping, no other stage. The
//! structure stage calls it once for each Node it makes and [`Paint::apply`] puts the answer on the Node.
//!
//! What is read, and what the model holds (`meldframe-protocol`):
//!
//! - `background-color` and the image layers of `background-image`: `fill`, one linear or radial `gradient` and one
//!   `image`. The image is read only in the form the export writes (an asset id in the url, with the size, position
//!   and repeat the export writes), every other `url()` is a placeholder: nothing is fetched, the text is kept in
//!   [`Paint::assets`];
//! - `border` (four equal solid sides, which take room: [`Paint::border_room`]) and the border of the export, the
//!   `::after` box, with its position;
//! - `border-radius` (four equal corners; `50%` is an ellipse or half the smaller side);
//! - `box-shadow` (outer shadows, the four topmost);
//! - `opacity`.
//!
//! Whatever else of these properties is declared is refused, never guessed, and goes to the report with the byte of
//! the element: kind `Refused` for a property that is lost, `Listed` for a value that was changed to the nearest the
//! model holds, `Cut` when a bound stopped the reading. The properties of the style stage (`Style::notes`: a
//! shorthand that was not read, a `var()` that did not resolve) are not repeated here. [`handles`] says which
//! property names this stage has an answer for, so that another stage does not list them again.

mod fill;
mod lex;

use crate::css::{EntryKind, Report, Style};
use meldframe_protocol::{Bind, Border, Gradient, ImageFill, Node, Shadow};

/// What the reader needs to know of the box besides its style: the size the author gave it in pixels (the border
/// box, as the model's size is) and whether the element has anything inside it. Both only decide a few corner cases;
/// `Ctx::default()` is a box of unknown size with nothing in it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ctx {
    /// The authored `width` in pixels, `None` when it is not a pixel size.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// The element holds a child node or any text. A box with `border-radius: 50%` that holds nothing is an ellipse;
    /// one that holds something is a frame, which has no ellipse form.
    pub has_children: bool,
}

/// A `src` that is kept for the agent and never followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub src: String,
}

impl Asset {
    /// `src` as written, without hidden characters, cut to 200 characters. A `data:` URL keeps only what comes
    /// before its first comma (the media type), never the payload.
    pub fn new(src: &str) -> Asset {
        let src = src.trim();
        let head = match src.get(..5) {
            Some(h) if h.eq_ignore_ascii_case("data:") => {
                src.split_once(',').map_or(src, |(h, _)| h)
            }
            _ => src,
        };
        Asset {
            src: head.chars().filter(|c| !c.is_control()).take(200).collect(),
        }
    }
}

/// The paint of one Node as the protocol holds it. `Paint::default()` paints nothing: `fill: none` (the Node's own
/// default is a grey), no radius, no border, no shadow, full opacity.
#[derive(Debug, Clone, PartialEq)]
pub struct Paint {
    /// A colour (hex, eight digits with an alpha) or the word `none`.
    pub fill: Bind<String>,
    pub gradient: Option<Gradient>,
    pub image: Option<ImageFill>,
    pub radius: Bind<f64>,
    /// The radius is `50%` on a box that holds nothing: the Node is an `ellipse`. `radius` stays 0 then.
    pub ellipse: bool,
    pub border: Border,
    /// What the element's own CSS `border` takes from the box, in pixels: the model's border is paint only, so the
    /// structure stage adds this to the padding and, under `content-box`, to the size. 0 for the export's border
    /// (the `::after` box takes no room) and when there is no border.
    pub border_room: f64,
    /// In the model's order: the first is the bottom one, so the last of the CSS list.
    pub shadows: Vec<Shadow>,
    pub opacity: Bind<f64>,
    /// The `url()`s that were not carried, for the result's `assets`.
    pub assets: Vec<Asset>,
}

impl Default for Paint {
    fn default() -> Self {
        Paint {
            fill: Bind::Lit("none".into()),
            gradient: None,
            image: None,
            radius: Bind::Lit(0.0),
            ellipse: false,
            border: Border::default(),
            border_room: 0.0,
            shadows: Vec::new(),
            opacity: Bind::Lit(1.0),
            assets: Vec::new(),
        }
    }
}

impl Paint {
    /// The paint of a box that stands for something that was not read (an `img`, an `svg`): nothing painted, and
    /// `src` (when there is one) kept under `assets`.
    pub fn placeholder(src: Option<&str>) -> Paint {
        Paint {
            assets: src.map(Asset::new).into_iter().collect(),
            ..Paint::default()
        }
    }

    /// Does the box paint something of its own (a colour, a gradient, an image, a border or a shadow), so that an
    /// element that holds only text is a frame around a text and not a text? A radius or an opacity alone is not.
    pub fn decorates(&self) -> bool {
        self.fill.lit().is_none_or(|f| f != "none")
            || self.gradient.is_some()
            || self.image.is_some()
            || self.border.width > 0.0
            || self.border_room > 0.0
            || !self.shadows.is_empty()
    }

    /// Put the paint on `node`: every field of it is written, so a field the CSS did not set is the CSS initial value
    /// and not the Node's default. The `ellipse` flag is the kind of the Node and is not written here.
    pub fn apply(&self, node: &mut Node) {
        node.fill = self.fill.clone();
        node.gradient = self.gradient.clone();
        node.image = self.image.clone();
        node.radius = self.radius.clone();
        node.border = self.border.clone();
        node.shadows = self.shadows.clone();
        node.opacity = self.opacity.clone();
    }
}

/// Where the reading says what it left out: the report, the byte of the element, and the work done.
pub(crate) struct Sink<'a> {
    pub report: &'a mut Report,
    pub at: usize,
    pub steps: usize,
}

impl Sink<'_> {
    fn note(&mut self, kind: EntryKind, code: &'static str, prop: &str, value: &str, why: &str) {
        let value: String = value.chars().take(80).collect();
        let msg = format!("{prop}: {value}: {why}");
        self.report.note(kind, code, prop, msg, self.at);
    }

    /// A property that is lost.
    pub fn refuse(&mut self, code: &'static str, prop: &str, value: &str, why: &str) {
        self.note(EntryKind::Refused, code, prop, value, why);
    }

    /// A value that was changed to the nearest the model holds (not a lost property).
    pub fn list(&mut self, code: &'static str, prop: &str, value: &str, why: &str) {
        self.note(EntryKind::Listed, code, prop, value, why);
    }
}

/// Read the paint of one element from its computed style.
///
/// - `ctx`: what the box is (see [`Ctx`]);
/// - `at`: the byte of the document where the element starts, for the report;
/// - `report`: receives what is refused, listed or cut, and the work in `report.work`.
///
/// Never fails and never panics; the work is bounded by the size of the values, which the CSS reader already bounds.
pub fn read(style: &Style, ctx: &Ctx, at: usize, report: &mut Report) -> Paint {
    let mut paint = Paint::default();
    let mut k = Sink {
        report,
        at,
        steps: 0,
    };
    fill::read(style, ctx, &mut paint, &mut k);
    k.report.work += k.steps;
    paint
}
