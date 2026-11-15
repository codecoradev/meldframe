//! Compile-fail proof that a permit and an admission cannot be forged (ADR-0010). Each case under
//! `tests/ui` is pinned to the exact compiler output in its `.stderr` file, which names the error code,
//! so a case cannot pass by failing for another reason. `pass_*.rs` is the companion that compiles.
//!
//! The `.stderr` files depend on the compiler version; `rust-toolchain.toml` pins it. After a
//! toolchain bump, regenerate them with
//! `TRYBUILD=overwrite cargo test -p meldframe-rooms --test compile_fail` and review the diff.

#[test]
fn permit_and_admission_are_sealed() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass_legal_path.rs");
    t.compile_fail("tests/ui/accept_bare_op.rs");
    t.compile_fail("tests/ui/permit_literal.rs");
    t.compile_fail("tests/ui/permit_new.rs");
    t.compile_fail("tests/ui/permit_clone.rs");
    t.compile_fail("tests/ui/admission_literal.rs");
    t.compile_fail("tests/ui/admission_set_op.rs");
    t.compile_fail("tests/ui/admission_set_permit.rs");
    t.compile_fail("tests/ui/admission_swap_parts.rs");
    t.compile_fail("tests/ui/accept_mixed_parts.rs");
}
