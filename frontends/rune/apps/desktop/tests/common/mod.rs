//! What the headless tests share: the file they draw, and the comparison with a golden.

use meldframe_protocol::Node;
use std::path::PathBuf;

/// A page with a card and a rectangle in it, and a second rectangle beside the card with a rounded corner and a border.
pub fn fixture_nodes() -> Vec<Node> {
    serde_json::from_str(
        r##"[
          {"id": "root", "type": "frame", "name": "Page", "x": 0, "y": 0, "w": 800, "h": 560, "fill": "#f4f4f5"},
          {"id": "f1", "type": "frame", "name": "Card", "parentId": "root", "x": 100, "y": 100, "w": 300, "h": 200, "fill": "#ffffff", "radius": 12},
          {"id": "r1", "type": "rect", "name": "Box", "parentId": "f1", "x": 10, "y": 10, "w": 50, "h": 40, "fill": "#3366cc"},
          {"id": "r2", "type": "rect", "name": "Pill", "parentId": "root", "x": 450, "y": 120, "w": 200, "h": 80, "fill": "#cc3366", "radius": 40}
        ]"##,
    )
    .expect("the fixture parses")
}

/// The golden file `name` of this crate's `tests/golden` directory.
pub fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

/// Compare `bytes` with the golden `name`, byte for byte. `MELDFRAME_UPDATE_GOLDEN=1` writes the file instead, and the change
/// is then read in the diff of the pull request.
pub fn golden(name: &str, bytes: &[u8]) {
    let file = path(name);
    if std::env::var_os("MELDFRAME_UPDATE_GOLDEN").is_some() {
        std::fs::write(&file, bytes).expect("the golden is written");
        return;
    }
    let want = std::fs::read(&file).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; MELDFRAME_UPDATE_GOLDEN=1 writes it",
            file.display()
        )
    });
    assert!(
        want == bytes,
        "{name} differs from its golden; MELDFRAME_UPDATE_GOLDEN=1 writes it, and the diff says what changed"
    );
}
