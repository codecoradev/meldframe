//! Who owns each live stream: the actor that opened it, on the file key it opened it on (which holds the workspace).
//!
//! The hubs know workspaces (by file key) but not
//! actors, so the server keeps this small map. Ids are never reused, so a stale entry cannot grant access
//! to a later stream; entries go with `unsubscribe`, `tick` and a failed subscribe.

use meldframe_stream::StreamId;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct Owners(HashMap<StreamId, (String, String)>);

impl Owners {
    pub(crate) fn set(&mut self, id: StreamId, key: &str, actor: &str) {
        self.0.insert(id, (key.to_string(), actor.to_string()));
    }

    /// `id` was opened on file `key` by `actor`.
    pub(crate) fn owns(&self, id: StreamId, key: &str, actor: &str) -> bool {
        self.0.get(&id).is_some_and(|(k, a)| k == key && a == actor)
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn forget(&mut self, id: StreamId) {
        self.0.remove(&id);
    }
}
