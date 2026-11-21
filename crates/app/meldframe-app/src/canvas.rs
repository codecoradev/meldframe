//! How the canvas gets its boxes: the host's text measurer and the `layout_on_canvas` flag.
//!
//! Both are host configuration, like `App::set_max_pending`: the shell of a platform sets them once and when its
//! facts change, and they are neither [`crate::AppState`] (the UI never reads them) nor the file (nobody else
//! sees them). With the flag off, the scene is the stored boxes, `resolve_with`, to the bit. With it on, it is
//! `resolve_laid_out`: hug, fill and auto layout applied, text sized by the measurer the host gave.
//!
//! The measurer is a seam of the portable layer: the browser host injects one backed by Pixi's `CanvasTextMetrics`,
//! a desktop host a native one, and [`StubMeasurer`] stays the default for tests and for hosts that give none.
//! On a native target the measurer must be `Send`, so that a desktop shell can keep the [`App`] behind a mutex;
//! under WASM there is one thread, and a measurer that calls into JavaScript cannot be `Send` (see [`MeasurerBox`]).

use crate::app::App;
use crate::effect::Effect;
use meldframe_layout::{Rect, StubMeasurer, TextMeasurer, compute_rects};
use meldframe_protocol::{Node, Variables};
use meldframe_render::{ModeChoice, PaintedBox, resolve_laid_out, resolve_with};
use std::fmt;

/// The measurer a host hands over. Native hosts need it `Send` (a desktop shell keeps the app behind a mutex);
/// under WASM there is a single thread and a JavaScript-backed measurer cannot be `Send`.
#[cfg(target_arch = "wasm32")]
pub type MeasurerBox = Box<dyn TextMeasurer>;
/// The measurer a host hands over. Native hosts need it `Send` (a desktop shell keeps the app behind a mutex);
/// under WASM there is a single thread and a JavaScript-backed measurer cannot be `Send`.
#[cfg(not(target_arch = "wasm32"))]
pub type MeasurerBox = Box<dyn TextMeasurer + Send>;

/// The flag and the measurer, held by the [`App`].
pub(crate) struct CanvasLayout {
    pub(crate) enabled: bool,
    measurer: MeasurerBox,
}

impl Default for CanvasLayout {
    fn default() -> Self {
        CanvasLayout {
            enabled: false,
            measurer: Box::new(StubMeasurer),
        }
    }
}

impl fmt::Debug for CanvasLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasLayout")
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

impl CanvasLayout {
    /// The boxes of the layout in world space, in drawing order, with the host's measurer (the rules of a drop and a
    /// resize read these).
    pub(crate) fn rects(&self, nodes: &[Node]) -> Vec<(String, Rect)> {
        compute_rects(nodes, self.measurer.as_ref())
    }

    pub(crate) fn scene(
        &self,
        nodes: &[Node],
        vars: &Variables,
        choice: &ModeChoice,
    ) -> Vec<PaintedBox> {
        if self.enabled {
            resolve_laid_out(nodes, vars, choice, self.measurer.as_ref())
        } else {
            resolve_with(nodes, vars, choice)
        }
    }
}

impl App {
    /// Whether the canvas reads laid-out boxes (see the module doc). Off until the host turns it on.
    pub fn layout_on_canvas(&self) -> bool {
        self.canvas.enabled
    }

    /// Turn the laid-out scene on or off. Returns the effect the host carries out: `Effect::Render` with an empty
    /// list (everything) when the flag changed, nothing when it did not. No event is raised, so a front end that reads the
    /// scene on events must read it again itself after this call. The only state it changes is the properties of the
    /// selected node: they show the box the canvas draws (`Properties::position_locked`), so they are brought up to date.
    pub fn set_layout_on_canvas(&mut self, on: bool) -> Vec<Effect> {
        if self.canvas.enabled == on {
            return Vec::new();
        }
        self.canvas.enabled = on;
        self.refresh_properties();
        redraw()
    }

    /// Replace the text measurer (the default is [`StubMeasurer`]). It matters only while the flag is on, so the
    /// redraw is asked for only then. The Export dialog does not use it yet: it keeps measuring as before.
    pub fn set_measurer(&mut self, measurer: MeasurerBox) -> Vec<Effect> {
        self.canvas.measurer = measurer;
        if self.canvas.enabled {
            self.refresh_properties();
            redraw()
        } else {
            Vec::new()
        }
    }
}

fn redraw() -> Vec<Effect> {
    vec![Effect::Render { nodes: Vec::new() }]
}
