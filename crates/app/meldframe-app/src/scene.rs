//! The scene the canvas draws, kept for as long as nothing it is made from has changed, and the two numbers a shell reads
//! to know that something did.
//!
//! A front end that calls the app layer as a library (a native shell) redraws when a number moved and not before: resolving the
//! layout on every frame is the one thing it must not do. `App::scene_epoch` names the painted boxes, `App::hit_index_epoch`
//! names the index of what is under a point. Both are plain integers that go up and never come back; two equal values mean
//! the same boxes, and the order of two different ones says only that the later one is newer.
//!
//! What the scene is made from: the revision of the document (`Replica::revision`: nodes and variables with the pending
//! ops), the measurer that answered (only while the flag is on: with it off the boxes are the stored ones and no measurer is
//! asked), the `layout_on_canvas` flag and the modes picked in `AppState::modes` (they change the painted colours). The camera
//! (`AppState::camera`) is not one of them: the scene is in world space and the shell draws it through the camera, so a camera
//! change moves neither epoch and costs no resolve, and a shell that draws through a camera needs no new scene for it.

use crate::app::App;
use crate::view_cache::{Held, Key};
use meldframe_render::PaintedBox;

impl App {
    fn scene_key(&self) -> Key {
        Key {
            revision: self.replica.revision(),
            generation: self.canvas.scene_generation(),
            enabled: self.canvas.enabled,
            modes: self.state.modes.clone(),
        }
    }

    /// Whether the held key is the one the app has now. The modes are compared in place, so a call that finds nothing changed
    /// allocates nothing.
    fn is_current(&self, held: &Held) -> bool {
        held.key.as_ref().is_some_and(|k| {
            k.revision == self.replica.revision()
                && k.generation == self.canvas.scene_generation()
                && k.enabled == self.canvas.enabled
                && k.modes == self.state.modes
        })
    }

    /// Brings the held key and epoch up to date, and the boxes too when `boxes` is set. A mutable borrow is taken only when
    /// something is stale, so a call made inside a [`App::with_scene`] read, where nothing can have changed (the app is
    /// borrowed shared), finds everything current and borrows nothing mutably.
    fn refresh_scene(&self, boxes: bool) {
        let stale = {
            let held = self.scene_cache.held.borrow();
            !self.is_current(&held) || (boxes && held.boxes.is_none())
        };
        if !stale {
            return;
        }
        let mut held = self.scene_cache.held.borrow_mut();
        if !self.is_current(&held) {
            held.key = Some(self.scene_key());
            held.epoch += 1;
            held.boxes = None;
        }
        if boxes && held.boxes.is_none() {
            let resolves = &self.scene_cache.resolves;
            resolves.set(resolves.get() + 1);
            held.boxes = Some(self.canvas.scene(self.nodes(), self.vars(), &self.choice()));
        }
    }

    /// The number that names the scene: it goes up whenever [`App::scene`] would give other boxes (the document, the
    /// measurer while the flag is on, the `layout_on_canvas` flag or the modes changed) and stays while it would give the same
    /// ones. A shell redraws when it moves. Asking costs a comparison, not a layout, and it is safe to ask inside a
    /// [`App::with_scene`] read.
    pub fn scene_epoch(&self) -> u64 {
        self.refresh_scene(false);
        self.scene_cache.held.borrow().epoch
    }

    /// Reads the boxes the canvas paints without copying them, with the epoch they are for. The scene is worked out only
    /// when the epoch moved since the last read. `read` may call [`App::scene_epoch`], [`App::scene`], [`App::with_scene`]
    /// and [`App::hit`] again: nothing can have changed while it runs, so they read what is held.
    pub fn with_scene<R>(&self, read: impl FnOnce(u64, &[PaintedBox]) -> R) -> R {
        self.refresh_scene(true);
        let held = self.scene_cache.held.borrow();
        read(held.epoch, held.boxes.as_deref().expect("just filled"))
    }

    /// How many times the app worked the scene out (layout included while the flag is on). Two reads with the same
    /// [`App::scene_epoch`] between them count once.
    pub fn scene_resolves(&self) -> u64 {
        self.scene_cache.resolves()
    }
}
