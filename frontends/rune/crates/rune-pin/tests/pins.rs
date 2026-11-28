//! The revision of Rune is one value in code and the same one in the manifest, the lock and
//! `deny.toml`: a bump that misses one of them fails here.

use std::fs;
use std::path::PathBuf;

use meldframe_rune_pin::{RUNE_REVISION, RUNE_URL, linked_app_spec};

/// The workspace root: `frontends/rune/`.
fn read(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read_to_string(root.join(name)).unwrap_or_else(|e| panic!("cannot read {name}: {e}"))
}

#[test]
fn the_revision_is_a_full_commit_id() {
    assert_eq!(RUNE_REVISION.len(), 40);
    assert!(RUNE_REVISION.bytes().all(|b| b.is_ascii_hexdigit()));
}

/// The `rune` dependency and the eight patches, and every `rev` in the manifest is the constant.
#[test]
fn the_manifest_pins_rune_and_eight_patches_at_one_revision() {
    let manifest = read("Cargo.toml");
    let revs: Vec<&str> = manifest
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && l.contains("rev = \""))
        .collect();
    assert_eq!(revs.len(), 9, "rune and eight patches: {revs:?}");
    let pin = format!("git = \"{RUNE_URL}\", rev = \"{RUNE_REVISION}\"");
    for line in revs {
        assert!(line.contains(&pin), "not at the pinned revision: {line}");
    }
}

#[test]
fn the_lock_resolves_every_rune_package_to_the_pinned_commit() {
    let lock = read("Cargo.lock");
    let sources: Vec<&str> = lock
        .lines()
        .filter(|l| l.starts_with("source = \"git+") && l.contains(RUNE_URL))
        .collect();
    assert!(!sources.is_empty(), "the lock has no Rune package");
    let want = format!("source = \"git+{RUNE_URL}?rev={RUNE_REVISION}#{RUNE_REVISION}\"");
    for line in sources {
        assert_eq!(line, want);
    }
}

#[test]
fn deny_toml_allows_this_revision_and_no_other_git_source() {
    let deny = read("deny.toml");
    let line = deny
        .lines()
        .find(|l| l.trim_start().starts_with("allow-git"))
        .expect("deny.toml has an allow-git list");
    assert!(
        line.contains(&format!("{RUNE_URL}?rev={RUNE_REVISION}")),
        "{line}"
    );
    assert_eq!(line.matches("https://").count(), 1, "{line}");
}

#[test]
fn the_crate_links_the_pinned_rune() {
    assert!(linked_app_spec().contains("AppSpec"));
}
