//! The scene the canvas draws, kept for as long as nothing it is made from has changed, and the two numbers a shell reads
//! to know that something did.
//!
//! A front end that calls the app layer as a library (a native shell) redraws when a number moved and not before: resolving the
//! layout on every frame is the one thing it must not do. `App::scene_epoch` names the painted boxes, `App::hit_index_epoch`
//! names the index of what is under a point. Both are plain integers that go up and never come back; two equal values mean
//! the same boxes, and the order of two different ones says only that the later one is newer.
//!
//! What the scene is made from: the revision of the document (`Replica::revision`: nodes and variables with the pending
//! ops), the measurer that answered, the `layout_on_canvas` flag and the modes picked in `AppState::modes` (they change the
//! painted colours). There is no camera here: the scene is in world space, and pan and zoom belong to the shell, so a camera
//! change moves neither epoch and a shell that draws through a camera needs no new scene for it.

use crate::app::App;
use meldframe_render::PaintedBox;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

/// Everything the painted boxes depend on: the revision, the measurer generation, the flag and the modes picked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    revision: (u64, u64),
    generation: u64,
    enabled: bool,
    modes: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
struct Held {
    key: Option<Key>,
    epoch: u64,
    /// The boxes for `key`, worked out on the first read after the key changed (an epoch can be asked for without them).
    boxes: Option<Vec<PaintedBox>>,
}

/// The painted boxes of the last read, with the epoch they belong to.
#[derive(Debug, Default)]
pub(crate) struct SceneCache {
    held: RefCell<Held>,
    /// How many times the scene was worked out (a test reads it through `App::scene_resolves`).
    resolves: Cell<u64>,
}

impl SceneCache {
    pub(crate) fn resolves(&self) -> u64 {
        self.resolves.get()
    }
}

impl App {
    fn scene_key(&self) -> Key {
        Key {
            revision: self.replica.revision(),
            generation: self.canvas.generation(),
            enabled: self.canvas.enabled,
            modes: self.state.modes.clone(),
        }
    }

    /// Moves the epoch when the key is not the one held. The modes are compared in place so that a call that changes nothing
    /// allocates nothing.
    fn touch_scene(&self, held: &mut Held) {
        let same = held.key.as_ref().is_some_and(|k| {
            k.revision == self.replica.revision()
                && k.generation == self.canvas.generation()
                && k.enabled == self.canvas.enabled
                && k.modes == self.state.modes
        });
        if !same {
            held.key = Some(self.scene_key());
            held.epoch += 1;
            held.boxes = None;
        }
    }

    /// The number that names the scene: it goes up whenever [`App::scene`] would give other boxes (the document, the
    /// measurer, the `layout_on_canvas` flag or the modes changed) and stays while it would give the same ones. A shell redraws
    /// when it moves. Asking costs a comparison, not a layout.
    pub fn scene_epoch(&self) -> u64 {
        match self.scene_cache.held.try_borrow_mut() {
            Ok(mut held) => {
                self.touch_scene(&mut held);
                held.epoch
            }
            // Inside a `with_scene` read: the app cannot have changed under a shared borrow, so the epoch held is current.
            Err(_) => self.scene_cache.held.borrow().epoch,
        }
    }

    /// Reads the boxes the canvas paints without copying them, with the epoch they are for. The scene is worked out only
    /// when the epoch moved since the last read.
    pub fn with_scene<R>(&self, read: impl FnOnce(u64, &[PaintedBox]) -> R) -> R {
        {
            let mut held = self.scene_cache.held.borrow_mut();
            self.touch_scene(&mut held);
            if held.boxes.is_none() {
                let resolves = &self.scene_cache.resolves;
                resolves.set(resolves.get() + 1);
                held.boxes = Some(self.canvas.scene(self.nodes(), self.vars(), &self.choice()));
            }
        }
        // A shared borrow from here, so that `read` may ask the epoch or the scene again.
        let held = self.scene_cache.held.borrow();
        read(held.epoch, held.boxes.as_deref().expect("just filled"))
    }

    /// How many times the app worked the scene out (layout included while the flag is on). Two reads with the same
    /// [`App::scene_epoch`] between them count once.
    pub fn scene_resolves(&self) -> u64 {
        self.scene_cache.resolves()
    }
}
