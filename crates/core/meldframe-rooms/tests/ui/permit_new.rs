// E0624: the constructor of a permit is private to the gate.
use meldframe_host::Permit;

fn main() {
    let _ = Permit::new("a", "w", "k", meldframe_module::RiskTier::Read);
}
