//! The window rendered to a PNG by Rune's own headless renderer (`--snap`), stored as a golden and byte stable across two runs
//! (E8.2.2). The pixels depend on the GPU path, so the golden is the one of a software adapter, SwiftShader's Vulkan, and the
//! test compares only when `VK_ICD_FILENAMES` names it. Anywhere else it prints SKIPPED and passes, and
//! `MELDFRAME_REQUIRE_GPU=1` makes that a failure, as `MELDFRAME_REQUIRE_PG` does for the Postgres tests.
//!
//! On the Linux machine of this work: `VK_ICD_FILENAMES=<a vk_swiftshader_icd.json> cargo test -p meldframe-desktop --test render`,
//! with `MELDFRAME_UPDATE_GOLDEN=1` to write the golden. A Mac renders with Metal and its pixels differ: the PNG there is a
//! measurement for E8.2.3 and not a golden.

use meldframe_desktop::{State, WINDOW_SIZE, spec};
use std::path::Path;

mod common;
use common::fixture_nodes;

fn fixture() -> State {
    State::with_nodes(fixture_nodes(), true)
}

/// Render the window headless to `out` at scale 1 with the bundled font only.
fn render(out: &Path) {
    rune::text::set_system_fonts(false);
    let (w, h) = WINDOW_SIZE;
    let argv = [
        "--snap".into(),
        out.display().to_string(),
        "--size".into(),
        format!("{}x{}", w as u32, h as u32),
        "--scale".into(),
        "1".into(),
    ];
    rune::App::new(spec(fixture))
        .args(argv)
        .try_run()
        .expect("the headless render runs");
}

#[test]
fn the_window_renders_to_a_png_golden_that_is_stable_across_two_runs() {
    let software = std::env::var("VK_ICD_FILENAMES").is_ok_and(|v| v.contains("swiftshader"));
    if !software {
        assert!(
            std::env::var_os("MELDFRAME_REQUIRE_GPU").is_none(),
            "MELDFRAME_REQUIRE_GPU is set and VK_ICD_FILENAMES names no SwiftShader ICD"
        );
        eprintln!(
            "SKIPPED: VK_ICD_FILENAMES does not name SwiftShader's Vulkan; the PNG golden is that adapter's"
        );
        return;
    }
    let dir = std::env::temp_dir().join(format!("meldframe-desktop-render-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let (first, second) = (dir.join("first.png"), dir.join("second.png"));
    render(&first);
    render(&second);
    let (a, b) = (
        std::fs::read(&first).expect("first png"),
        std::fs::read(&second).expect("second png"),
    );
    std::fs::remove_dir_all(&dir).ok();
    assert!(a.starts_with(b"\x89PNG"), "the first run wrote a PNG");
    assert!(a == b, "two runs give the same bytes");
    common::golden("window.swiftshader.png", &a);
}
