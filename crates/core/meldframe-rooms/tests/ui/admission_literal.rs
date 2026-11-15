// E0451: an admission has private fields, so no struct literal pairs an op with a permit.
use meldframe_host::{Admission, Op, Permit};

fn pair_by_hand(op: Op, permit: Permit) -> Admission {
    Admission { op, permit }
}

fn main() {
    let _ = pair_by_hand;
}
