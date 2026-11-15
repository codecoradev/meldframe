//! `Room::open` replays the log in pages, so an adapter never has to return a whole long tail at once.

mod common;

use common::{add, admit, op};
use meldframe_host::{
    Appended, MemoryStore, NewOp, REPLAY_PAGE, Room, Snapshot, Store, StoreError, StoredOp,
};
use std::cell::{Cell, RefCell};

/// A store that records every `ops_after` call it gets.
struct Recording {
    inner: MemoryStore,
    calls: RefCell<Vec<(u64, usize)>>,
    served: Cell<usize>,
}

impl Store for Recording {
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        self.inner.append(room, op)
    }
    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError> {
        self.inner.find(room, actor, key)
    }
    fn latest_seq(&self, room: &str) -> Result<u64, StoreError> {
        self.inner.latest_seq(room)
    }
    fn ops_after(&self, room: &str, seq: u64, limit: usize) -> Result<Vec<StoredOp>, StoreError> {
        self.calls.borrow_mut().push((seq, limit));
        let page = self.inner.ops_after(room, seq, limit)?;
        self.served.set(self.served.get() + page.len());
        Ok(page)
    }
    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        self.inner.save_snapshot(room, snapshot)
    }
    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError> {
        self.inner.latest_snapshot(room)
    }
}

#[test]
fn open_replays_a_long_log_in_bounded_pages_and_gets_the_same_tree() {
    let total = REPLAY_PAGE * 2 + 5;
    let mut room = Room::open("ws_1", "file_1", MemoryStore::new())
        .unwrap()
        .with_snapshot_every(u64::MAX);
    for i in 0..total {
        let adm = admit("user:a", op(&format!("k{i}"), add(&format!("n{i}"))));
        room.accept(adm).unwrap();
    }
    let want: Vec<String> = room.nodes().iter().map(|n| n.id.clone()).collect();
    let store = Recording {
        inner: room.into_store(),
        calls: RefCell::new(Vec::new()),
        served: Cell::new(0),
    };
    let reopened = Room::open("ws_1", "file_1", store).unwrap();
    let got: Vec<String> = reopened.nodes().iter().map(|n| n.id.clone()).collect();
    assert_eq!(got, want);
    assert_eq!(reopened.seq(), total as u64);
    let calls = reopened.store().calls.borrow();
    assert!(calls.len() >= 3, "expected several pages, got {calls:?}");
    assert!(calls.iter().all(|&(_, limit)| limit == REPLAY_PAGE));
    assert_eq!(reopened.store().served.get(), total);
}
