//! The canvas of the Rune front end (E8.2.2): `App::scene()` drawn as one shape that is rebuilt only when
//! `App::scene_epoch` moves, and a press on it sent to the app layer as a pointer click. The crate keeps no state of the
//! file and decides nothing about a click: what is under the point, and what a click selects, is the app layer's (the pointer
//! machine, which asks `App::hit`; T9 and E8.1.4).
//!
//! What this first slice draws: the fill of each box with its own alpha and its corner radius, and the border of a box, each
//! at the world rectangle of `meldframe_render::world_rects` (the placement `HitIndex` reads, so the rectangles of the painter
//! and of a click are the same ones; only the rectangles, not the clip or the visibility the hit rule also reads). The
//! opacity of a node is multiplied down the tree, as the web canvas does it (Pixi fades each part alone: a box under a faded
//! frame is as faded as both, and overlapping parts do not fade as one image, `docs/fidelity-contract.md`, row `border`).
//!
//! What it does not draw: text, gradients, images, shadows, a `clip` (a Rune shape cannot be cut by a box: a clip would need
//! an element for each clipping frame and no longer one shape), and a camera other than the home one (zoom 1 at the origin).
//! A box a clip would cut is painted whole. Those are the criteria C1 to C3 and the canvas issue of E8.3.2.

use meldframe_app::{Action, App, Modifiers, PointerButton, PointerInput, PointerPhase};
use meldframe_protocol::BorderPosition;
use meldframe_render::{PaintedBox, world_rects};
use meldframe_ui_rune::colours;
use rune::color::{Color, srgb_decode};
use rune::shape::{Path, Shape};
use rune::{El, canvas, el};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// The canvas element of the scene for the epoch it was built at. A Rune `El` shares its shape when it is cloned, so handing
/// it on costs the clone of an element and never a copy of the shape; the only deep copy is the one the build makes.
#[derive(Default)]
pub struct SceneShape {
    held: RefCell<Option<(u64, El)>>,
    builds: Cell<u64>,
}

impl std::fmt::Debug for SceneShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneShape")
            .field("epoch", &self.epoch())
            .field("builds", &self.builds())
            .finish()
    }
}

impl SceneShape {
    /// The canvas element for the app's current scene. The shape is built again only when `App::scene_epoch` is not the one the
    /// held element was built at, so a view that runs for another reason (a hover, a button) costs a comparison and a clone
    /// of the element.
    pub fn element(&self, app: &App) -> El {
        let epoch = app.scene_epoch();
        if let Some((held, el)) = &*self.held.borrow()
            && *held == epoch
        {
            return el.clone();
        }
        let el = canvas(app.with_scene(|_, boxes| paint(boxes)))
            .key("canvas")
            .w_full()
            .h_full();
        self.builds.set(self.builds.get() + 1);
        *self.held.borrow_mut() = Some((epoch, el.clone()));
        el
    }

    /// The epoch of the held element, none before the first build.
    pub fn epoch(&self) -> Option<u64> {
        self.held.borrow().as_ref().map(|(e, _)| *e)
    }

    /// How many times the shape was built.
    pub fn builds(&self) -> u64 {
        self.builds.get()
    }
}

/// The canvas element: the held shape on the page colour, with `on_click` called with the actions of a click at the point
/// of the press (in points from the top left corner of the canvas). It takes the room the shell's column leaves.
pub fn view<S: 'static>(
    app: &App,
    shape: &SceneShape,
    on_click: impl Fn(&mut S, Vec<Action>) + 'static,
) -> El {
    el().key("canvas-host")
        .label("Canvas")
        .fill(colours().bg)
        .grow(1.0)
        .w_full()
        .clip_soft(0.0)
        .on_background_press_tx(move |s: &mut S, tx| {
            if let Some(p) = tx.press_point() {
                on_click(s, click(f64::from(p.x), f64::from(p.y)));
            }
        })
        .child(shape.element(app))
}

/// The actions of one click at a point of the canvas: the press and the release, as the platform reports them. The app
/// layer's machine decides what they come to.
pub fn click(x: f64, y: f64) -> Vec<Action> {
    [PointerPhase::Down, PointerPhase::Up]
        .into_iter()
        .map(|phase| Action::Pointer {
            input: PointerInput {
                phase,
                x,
                y,
                modifiers: Modifiers::default(),
                button: PointerButton::Primary,
                touch: false,
                space: false,
                at_ms: None,
            },
        })
        .collect()
}

/// The boxes of a scene as one shape, back to front, in one pass. A box is drawn at its world rectangle
/// (`meldframe_render::world_rects`) with the opacity of its own and of every box above it multiplied; a box whose id an
/// earlier box has is not drawn, as the hit rule never finds it.
pub fn paint(boxes: &[PaintedBox]) -> Shape {
    let mut faded: HashMap<&str, f64> = HashMap::with_capacity(boxes.len());
    let mut shape = Shape::new();
    for (b, placed) in boxes.iter().zip(world_rects(boxes)) {
        let Some(r) = placed else { continue };
        let above = b
            .parent
            .as_deref()
            .and_then(|p| faded.get(p))
            .copied()
            .unwrap_or(1.0);
        let opacity = above * b.opacity;
        faded.insert(b.id.as_str(), opacity);
        shape = paint_box(shape, b, (r.x, r.y), opacity);
    }
    shape
}

fn paint_box(mut shape: Shape, b: &PaintedBox, (bx, by): (f64, f64), opacity: f64) -> Shape {
    if let Some((rgb, alpha)) = b.painted_fill() {
        shape = shape.fill_path(
            &rounded(bx, by, b.w, b.h, b.radius),
            colour(rgb, (opacity * alpha) as f32),
        );
    }
    if let Some(border) = b.border {
        let grow = match border.position {
            BorderPosition::Inside => -border.width / 2.0,
            BorderPosition::Center => 0.0,
            BorderPosition::Outside => border.width / 2.0,
        };
        let (x, y, w, h) = (bx - grow, by - grow, b.w + 2.0 * grow, b.h + 2.0 * grow);
        let ring = rounded(x, y, w, h, b.radius + grow);
        let line = colour(border.color, (opacity * border.alpha) as f32);
        shape = shape.stroke_path(&ring, border.width as f32, rune::shape::LineCap::Butt, line);
    }
    shape
}

/// `0xRRGGBB` and an alpha as a Rune colour (linear light, straight alpha).
fn colour(rgb: u32, alpha: f32) -> Color {
    let channel = |shift: u32| srgb_decode(((rgb >> shift) & 0xff) as f32 / 255.0);
    Color {
        r: channel(16),
        g: channel(8),
        b: channel(0),
        a: alpha,
    }
}

/// A rectangle with rounded corners as a closed path. A radius larger than half a side is cut to it.
fn rounded(x: f64, y: f64, w: f64, h: f64, radius: f64) -> Path {
    let r = radius.clamp(0.0, (w.min(h) / 2.0).max(0.0));
    let (x0, y0, x1, y1) = (x as f32, y as f32, (x + w) as f32, (y + h) as f32);
    if r <= 0.0 {
        return Path::new().polygon([[x0, y0], [x1, y0], [x1, y1], [x0, y1]]);
    }
    // A quarter circle as one cubic, with the usual handle length.
    let (r, k) = (r as f32, r as f32 * (1.0 - KAPPA));
    Path::new()
        .move_to([x0 + r, y0])
        .line_to([x1 - r, y0])
        .cubic_to([x1 - k, y0], [x1, y0 + k], [x1, y0 + r])
        .line_to([x1, y1 - r])
        .cubic_to([x1, y1 - k], [x1 - k, y1], [x1 - r, y1])
        .line_to([x0 + r, y1])
        .cubic_to([x0 + k, y1], [x0, y1 - k], [x0, y1 - r])
        .line_to([x0, y0 + r])
        .cubic_to([x0, y0 + k], [x0 + k, y0], [x0 + r, y0])
        .close()
}

/// The handle length of a cubic that draws a quarter circle, as a fraction of the radius.
const KAPPA: f32 = 0.552_284_8;
