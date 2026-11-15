// E0616: the op of an admission the gate made cannot be replaced.
use meldframe_host::{Gate, Op, Principal};
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
    let (mut a, b) = two();
    a.op = b.op;
}
