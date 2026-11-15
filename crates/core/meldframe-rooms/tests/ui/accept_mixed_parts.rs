// E0616 (private fields) and E0061 (accept takes one admission): the op of one admission and the
// permit of another cannot be handed to the room.
use meldframe_host::{Gate, MemoryStore, Op, Principal, Room};
use meldframe_protocol::Command;

fn two() -> (meldframe_host::Admission, meldframe_host::Admission) {
    let who = Principal::new("ws_1", "a");
    let ops = vec![
        Op::new("k1", Command::RemoveNode { id: "n1".into() }),
        Op::new("k2", Command::RemoveNode { id: "n1".into() }),
    ];
    let mut v = Gate::with_policy(|_, _| meldframe_module::Policy::Auto)
        .submit(&who, ops)
        .unwrap()
        .admitted;
    (v.remove(0), v.remove(0))
}

fn main() {
    let (a, b) = two();
    let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    let _ = room.accept(a.op, b.permit);
}
