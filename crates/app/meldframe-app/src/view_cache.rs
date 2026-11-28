//! What the app keeps between dispatches, so that a view is built again only when what it reads has changed.

use crate::parts::panel::PanelKey;
use meldframe_render::PaintedBox;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

/// What the view was built from, and how often it was built. A dispatch that changes none of these (a move, a
/// resize, a timer) leaves the view as it is: the check reads the file but allocates only when something differs.
#[derive(Debug, Default)]
pub(crate) struct PanelCache {
    pub(crate) key: Option<PanelKey>,
    pub(crate) builds: u64,
    /// How many times the bind picker was built (a test seam, as `builds`).
    pub(crate) picker_builds: u64,
}

/// Everything the painted boxes depend on: the revision, the measurer generation, the flag and the modes picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Key {
    pub(crate) revision: (u64, u64),
    pub(crate) generation: u64,
    pub(crate) enabled: bool,
    pub(crate) modes: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub(crate) struct Held {
    pub(crate) key: Option<Key>,
    pub(crate) epoch: u64,
    /// The boxes for `key`, worked out on the first read after the key changed (an epoch can be asked for without them).
    pub(crate) boxes: Option<Vec<PaintedBox>>,
}

/// The painted boxes of the last read, with the epoch they belong to.
#[derive(Debug, Default)]
pub(crate) struct SceneCache {
    pub(crate) held: RefCell<Held>,
    /// How many times the scene was worked out (a test reads it through `App::scene_resolves`).
    pub(crate) resolves: Cell<u64>,
}

impl SceneCache {
    pub(crate) fn resolves(&self) -> u64 {
        self.resolves.get()
    }
}
