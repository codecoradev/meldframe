//! What a PNG export comes to (and, later, a capture), decided from the scene (finding T13 of the thin UI audit, decision 43): the
//! rectangle (the box with the reach of its paint, `meldframe_render::capture_rect`), the scale, the size in pixels, the file
//! name and the refusal, and what the shell's report of a drawn image has to meet before its bytes go anywhere. Pure functions
//! of the boxes and the numbers: the state and the effects are `png`'s.

use meldframe_app_model::{
    EXPORT_MAX_PIXELS, PNG_MIN_BYTES, PngPlan, RasterRect, RasterSize, SCALES,
};
use meldframe_render::{HitIndex, PaintedBox, capture_bounds_in, capture_rect_in};

/// The scene a plan reads: the painted boxes and the index of them (`HitIndex::from_boxes`), which the app keeps for as long
/// as the boxes stay.
#[derive(Debug, Clone, Copy)]
pub struct Scene<'a> {
    pub boxes: &'a [PaintedBox],
    pub index: &'a HitIndex<'a>,
}

/// Why there is no image. The code is the one a refusal and a `captureFailed` will carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The scale is not one the request may have.
    InvalidScale,
    /// The node is not there, or has no size.
    NodeNotFound,
    /// The image would be over the pixel limit.
    TooLarge,
}

impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Refusal::InvalidScale => "invalid_scale",
            Refusal::NodeNotFound => "node_not_found",
            Refusal::TooLarge => "too_large",
        }
    }
}

/// A scale of the PNG export: one of the steps the section offers ([`SCALES`], 0.5 to 4).
pub fn is_export_scale(scale: f64) -> bool {
    SCALES.contains(&scale)
}

/// The image a plan comes to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Planned {
    pub rect: RasterRect,
    pub width: u32,
    pub height: u32,
}

/// The world rectangle of a node, or of the whole page for none (`None` when it is not there).
pub fn rect_of(scene: Scene, node: Option<&str>) -> Option<RasterRect> {
    let Scene { boxes, index } = scene;
    let r = node.map_or_else(
        || capture_bounds_in(index, boxes),
        |id| capture_rect_in(index, boxes, id),
    )?;
    Some(RasterRect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    })
}

/// The pixels of a length at a scale: rounded up, so no edge is cut, and at least 1, so no image is 0 px (a scale as small as a
/// number gets, or a length that underflows with it, would round to 0).
fn pixels(length: f64, scale: f64) -> f64 {
    (length * scale).ceil().max(1.0)
}

/// The size in pixels of an image of `rect` at `scale`: each side rounded up, so no edge is cut, and at least 1 px.
/// `NodeNotFound` for a rectangle with no width or height, `TooLarge` over `max_pixels` or when a side does not fit 32 bits.
pub fn size_of(rect: &RasterRect, scale: f64, max_pixels: u64) -> Result<(u32, u32), Refusal> {
    if !(rect.w > 0.0 && rect.h > 0.0) {
        return Err(Refusal::NodeNotFound);
    }
    let (w, h) = (pixels(rect.w, scale), pixels(rect.h, scale));
    // The sides are numbers from 1 up (the rectangle and the scale are checked), so the product is one or infinite.
    let max_side = f64::from(u32::MAX);
    if w * h > max_pixels as f64 || w > max_side || h > max_side {
        return Err(Refusal::TooLarge);
    }
    Ok((w as u32, h as u32))
}

/// The plan for `node` (`None`: the page) at `scale`. The scale is checked first, so a bad one is refused before the scene is
/// read at all.
pub fn plan(
    scene: Scene,
    node: Option<&str>,
    scale: f64,
    scale_ok: fn(f64) -> bool,
    max_pixels: u64,
) -> Result<Planned, Refusal> {
    if !scale_ok(scale) {
        return Err(Refusal::InvalidScale);
    }
    let rect = rect_of(scene, node).ok_or(Refusal::NodeNotFound)?;
    let (width, height) = size_of(&rect, scale, max_pixels)?;
    Ok(Planned {
        rect,
        width,
        height,
    })
}

/// The file a PNG of `stem` at `scale` is saved under: the stem (safe as it is: `meldframe-text` made it, rule U9), then
/// `@<scale>x` for every scale but 1, then `.png`.
pub fn file_name(stem: &str, scale: f64) -> String {
    if scale == 1.0 {
        format!("{stem}.png")
    } else {
        format!("{stem}@{scale}x.png")
    }
}

/// What a PNG export of frame `id` would be at `scale`; `stem` is the stem of its files (`ExportTarget::file_stem`).
pub fn png_plan(scene: Scene, id: &str, stem: &str, scale: f64) -> PngPlan {
    let refused = |code: Refusal, asked, fits| PngPlan::Refused {
        code: code.code().to_owned(),
        asked,
        fits,
    };
    if !is_export_scale(scale) {
        return refused(Refusal::InvalidScale, None, None);
    }
    let Some(rect) = rect_of(scene, Some(id)) else {
        return refused(Refusal::NodeNotFound, None, None);
    };
    match size_of(&rect, scale, EXPORT_MAX_PIXELS) {
        Ok((width, height)) => PngPlan::Ready {
            rect,
            scale,
            width,
            height,
            file: file_name(stem, scale),
        },
        Err(Refusal::TooLarge) => {
            let side = |s: f64| RasterSize {
                scale: s,
                width: pixels(rect.w, s) as u32,
                height: pixels(rect.h, s) as u32,
            };
            let fits = SCALES
                .iter()
                .rev()
                .find(|s| size_of(&rect, **s, EXPORT_MAX_PIXELS).is_ok());
            refused(Refusal::TooLarge, Some(side(scale)), fits.map(|s| side(*s)))
        }
        Err(other) => refused(other, None, None),
    }
}

/// What the shell reported of the image it drew, against the plan and the limits: the code to refuse it with, or none when
/// it may go on. `max_bytes` is the cap of the sink (none for a download; a capture's upload has one later).
/// Over a limit is `too_large`; a size that is not the planned one, or bytes that cannot be a PNG, is `render_failed`.
pub fn drawn_refusal(
    drawn: (u32, u32, u64),
    planned: (u32, u32),
    max_pixels: u64,
    max_bytes: Option<u64>,
) -> Option<&'static str> {
    let (width, height, bytes) = drawn;
    if u64::from(width) * u64::from(height) > max_pixels || max_bytes.is_some_and(|m| bytes > m) {
        Some("too_large")
    } else if (width, height) != planned || bytes < PNG_MIN_BYTES {
        Some("render_failed")
    } else {
        None
    }
}
