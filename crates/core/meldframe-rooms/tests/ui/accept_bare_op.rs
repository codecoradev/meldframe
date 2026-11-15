// E0308: a write without a permit does not compile, an op is not an admission.
use meldframe_host::{MemoryStore, Op, Room};
use meldframe_protocol::Command;

fn main() {
    let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    let op = Op::new("k1", Command::RemoveNode { id: "n1".into() });
    let _ = room.accept(op);
}
