//! A read whose store fails is logged with the outcome the caller gets (`unavailable`), not `ok`.

use meldframe_calls::{CallEntry, CallLog};
use meldframe_host::{MemoryStore, Principal};
use meldframe_http::mcp::Reader;
use meldframe_server::{Backing, Server, SharedServer};
use std::sync::{Arc, Mutex};

/// A call log the test can read: every entry also goes into a shared list.
#[derive(Debug, Default)]
struct Shared(Arc<Mutex<Vec<CallEntry>>>);
impl CallLog for Shared {
    fn record(&mut self, entry: CallEntry) {
        self.0.lock().unwrap().push(entry);
    }
    fn recent(&self, limit: usize) -> Vec<CallEntry> {
        let v = self.0.lock().unwrap();
        v[v.len().saturating_sub(limit)..].to_vec()
    }
}

#[test]
fn a_read_that_fails_in_the_store_is_logged_as_unavailable() {
    let store: Arc<Mutex<MemoryStore>> = Arc::default();
    let entries: Arc<Mutex<Vec<CallEntry>>> = Arc::default();
    let mut server = Server::assemble_in(Vec::new(), Backing::Memory(store.clone()))
        .with_call_log(Box::new(Shared(entries.clone())));
    // Open the file while the store works and keep it open, then break the store: the room is in memory, so
    // the read itself starts, and the query inside it (the recent ops, the applied keys) fails.
    server.open("ws_1", "f").unwrap();
    let shared = SharedServer::new(server);
    let who = Principal::new("ws_1", "agent:a");
    let s = store.clone();
    let _ = std::thread::spawn(move || {
        let _held = s.lock().unwrap();
        panic!("poison the store");
    })
    .join();
    for tool in ["get_tree", "batch_status"] {
        let failed = match tool {
            "get_tree" => shared.tree(&who, "f", 10).is_err(),
            _ => shared.applied(&who, "f", &["k1".into()]).is_err(),
        };
        assert!(failed, "{tool}: the caller gets the failure");
        let last = entries.lock().unwrap().last().cloned().unwrap();
        assert_eq!(
            (last.tool.as_str(), last.outcome.as_str(), last.decision),
            (tool, "unavailable", None),
            "{tool}"
        );
    }
}
