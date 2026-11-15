// The legal path compiles: the gate makes the admission and the room takes it whole.
use meldframe_host::{Gate, MemoryStore, Op, Principal, Room};
use meldframe_protocol::Command;

fn main() {
    let who = Principal::new("ws_1", "a");
    let op = Op::new("k1", Command::RemoveNode { id: "n1".into() });
    let adm = Gate::with_policy(|_, _| meldframe_module::Policy::Auto)
        .submit(&who, vec![op])
        .unwrap()
        .admitted
        .remove(0);
    assert_eq!((adm.op().key.as_str(), adm.permit().key()), ("k1", "k1"));
    let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    let _ = room.accept(adm);
}
