// E0451: the fields of a permit are private, so no struct literal builds one.
use meldframe_host::Permit;
use meldframe_module::RiskTier;

fn main() {
    let _ = Permit {
        actor: "a".into(),
        workspace: "w".into(),
        key: "k".into(),
        tier: RiskTier::Read,
    };
}
