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
use meldframe_render::{HitIndex, ModeChoice, PaintedBox, resolve_laid_out, resolve_with};
use std::cell::{Cell, RefCell};
use std::fmt;

/// The measurer a host hands over. Native hosts need it `Send` (a desktop shell keeps the app behind a mutex);
/// under WASM there is a single thread and a JavaScript-backed measurer cannot be `Send`.
#[cfg(target_arch = "wasm32")]
pub type MeasurerBox = Box<dyn TextMeasurer>;
/// The measurer a host hands over. Native hosts need it `Send` (a desktop shell keeps the app behind a mutex);
/// under WASM there is a single thread and a JavaScript-backed measurer cannot be `Send`.
#[cfg(not(target_arch = "wasm32"))]
pub type MeasurerBox = Box<dyn TextMeasurer + Send>;

/// What the boxes of the layout depend on: the revision of the document (the nodes with the pending ops, see
/// `Replica::revision`) and which measurer answered. Nothing else reaches them: `compute_rects` takes the nodes and the
/// measurer and no variables, so a mode switch changes no box (the painted colours, which do read the mode, are the
/// scene's and not cached here), and the flag only decides whether anyone asks.
type Key = ((u64, u64), u64);

/// The boxes of the layout in world space, in drawing order, with the key they were worked out for.
type Cached = (Key, Vec<(String, Rect)>);

/// What the hit index depends on: the revision of the document, the measurer and the flag (the scene is the stored boxes or
/// the laid-out ones). The mode choice and the variables are not in it: they change what is painted, never where a box is.
type HitKey = ((u64, u64), u64, bool);

/// The index of what is under a point, the key it is for and its epoch (see `crate::scene`). The epoch moves when the key
/// does, whether or not anyone has asked for the index since, so a shell can read it without building anything.
#[derive(Default)]
struct HitCache {
    key: Option<HitKey>,
    epoch: u64,
    index: Option<HitIndex<'static>>,
}

impl HitCache {
    fn touch(&mut self, key: HitKey) {
        if self.key != Some(key) {
            self.key = Some(key);
            self.epoch += 1;
            self.index = None;
        }
    }
}

/// The flag and the measurer, held by the [`App`], and the boxes of the last layout.
pub(crate) struct CanvasLayout {
    pub(crate) enabled: bool,
    measurer: MeasurerBox,
    /// Goes up each time the host gives a measurer (even the same function again: its answers may have changed).
    generation: u64,
    /// The boxes of the layout for the key they were worked out at: a preview asks for them on every pointer move, and
    /// the file has not changed between two of those. Interior, because a preview reads the app and changes nothing.
    cache: RefCell<Option<Cached>>,
    /// How many times the whole file was laid out for `rects` (a test reads it through `App::layout_runs`).
    runs: Cell<u64>,
    /// What is under a point, for the scene of the key it was built for (`App::hit`; the hover asks on every frame).
    hits: RefCell<HitCache>,
    /// How many times the index was built (a test reads it through `App::hit_builds`).
    hit_builds: Cell<u64>,
}

impl Default for CanvasLayout {
    fn default() -> Self {
        CanvasLayout {
            enabled: false,
            measurer: Box::new(StubMeasurer),
            generation: 0,
            cache: RefCell::new(None),
            runs: Cell::new(0),
            hits: RefCell::new(HitCache::default()),
            hit_builds: Cell::new(0),
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
    /// The host's measurer (the stub when it gave none): the canvas lays text out with it and so does the Export dialog.
    pub(crate) fn measurer(&self) -> &dyn TextMeasurer {
        self.measurer.as_ref()
    }

    /// Reads the boxes of the layout in world space, in drawing order, with the host's measurer (the rules of a drop and a
    /// resize read these). They are laid out again only when `revision` (that of the document `nodes` is read from) or the
    /// measurer is not the one of the last call.
    pub(crate) fn with_rects<R>(
        &self,
        revision: (u64, u64),
        nodes: &[Node],
        read: impl FnOnce(&[(String, Rect)]) -> R,
    ) -> R {
        let key = (revision, self.generation);
        let mut cache = self.cache.borrow_mut();
        if cache.as_ref().is_none_or(|(k, _)| *k != key) {
            self.runs.set(self.runs.get() + 1);
            *cache = Some((key, compute_rects(nodes, self.measurer.as_ref())));
        }
        let (_, rects) = cache.as_ref().expect("just filled");
        read(rects)
    }

    pub(crate) fn hit_builds(&self) -> u64 {
        self.hit_builds.get()
    }

    /// The measurer generation as far as the scene and the hit index depend on it: with the flag off the boxes are the stored
    /// ones and no measurer is asked, so a new measurer changes nothing there.
    pub(crate) fn scene_generation(&self) -> u64 {
        if self.enabled { self.generation } else { 0 }
    }

    fn hit_key(&self, revision: (u64, u64)) -> HitKey {
        (revision, self.scene_generation(), self.enabled)
    }

    /// The number that names the index for `revision`, the measurer and the flag. The mode choice and the variables are not in
    /// it, so a mode switch leaves it alone.
    pub(crate) fn hit_epoch(&self, revision: (u64, u64)) -> u64 {
        let mut cache = self.hits.borrow_mut();
        cache.touch(self.hit_key(revision));
        cache.epoch
    }

    /// Reads the index of what is under a point for the scene of `revision`, the measurer and the flag. The scene is worked
    /// out and indexed only when one of them is not the one of the last call, so the questions that follow while the pointer
    /// moves over a file that has not changed read the index and nothing else.
    pub(crate) fn with_hits<R>(
        &self,
        revision: (u64, u64),
        build: impl FnOnce() -> HitIndex<'static>,
        read: impl FnOnce(&HitIndex) -> R,
    ) -> R {
        let mut cache = self.hits.borrow_mut();
        cache.touch(self.hit_key(revision));
        if cache.index.is_none() {
            self.hit_builds.set(self.hit_builds.get() + 1);
            cache.index = Some(build());
        }
        read(cache.index.as_ref().expect("just filled"))
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

    /// How many times the app laid the whole file out to answer a drop, a preview or a resize (the scene is worked out
    /// apart from this). A second question about a file that has not changed, with the same measurer, costs none.
    pub fn layout_runs(&self) -> u64 {
        self.canvas.runs.get()
    }

    /// Replace the text measurer (the default is [`StubMeasurer`]). The canvas lays text out with it while the flag is on,
    /// so the redraw is asked for only then. The Export dialog measures with it whatever the flag is, so a hug text exports
    /// the width the canvas draws: an open dialog is built again (`Event::ExportChanged` when it changed), and nothing is
    /// built while it is closed. No measurer given, the stub measures, as it always did.
    pub fn set_measurer(&mut self, measurer: MeasurerBox) -> Vec<Effect> {
        self.canvas.measurer = measurer;
        self.canvas.generation += 1;
        self.measurer_changed();
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
