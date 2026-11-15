//! The in-memory adapter passes the shared store contract.

use meldframe_host::MemoryStore;

fn make() -> Option<MemoryStore> {
    Some(MemoryStore::new())
}

meldframe_store_contract::store_contract!(make);
