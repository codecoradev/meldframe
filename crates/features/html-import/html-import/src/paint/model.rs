//! The paint of one Node as the protocol holds it, with the context of the box and the addresses that are listed and never followed.

use meldframe_protocol::{Bind, Border, Gradient, ImageFill, Node, Shadow};
use meldframe_text::{Profile, sanitize_clip};

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
    /// `src` as written, without a control or hidden character, cut to 200 characters. A `data:` URL keeps only what
    /// comes before its first comma (the media type), never the payload. An address is identifier-like (the agent
    /// passes it on), so it takes the strict profile of `meldframe-text` (rule U9): no joiner is kept, there is no
    /// emoji in it to protect.
    pub fn new(src: &str) -> Asset {
        let src = src.trim();
        let head = match src.get(..5) {
            Some(h) if h.eq_ignore_ascii_case("data:") => {
                src.split_once(',').map_or(src, |(h, _)| h)
            }
            _ => src,
        };
        Asset {
            src: sanitize_clip(head, Profile::Strict, 200, usize::MAX),
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
