//! The work an import does on the text of a report, counted in bytes: a name that is long and collides again and again
//! is shortened once, not once for each collision. This is the only test of its binary, because the count is global.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::sync::atomic::Ordering;
use variables_import::{ImportOptions, TEXT_WORK, import_css};

#[test]
fn a_long_name_that_20000_names_collide_with_is_cut_a_few_times_not_20000() {
    let mut t = format!(":root {{ --a{}b: #fff;", "_".repeat(100_000));
    // 20,000 different spellings of one name: runs of `-`, `_` and `.` of nine characters, in base 3.
    for k in 1..20_000usize {
        let sep: String = (0..9)
            .map(|d| ['-', '_', '.'][k / 3usize.pow(d) % 3])
            .collect();
        t.push_str(&format!("--a{sep}b:1;"));
    }
    t.push('}');
    TEXT_WORK.store(0, Ordering::SeqCst);
    let o = ImportOptions {
        max_bytes: usize::MAX,
        ..ImportOptions::default()
    };
    let i = import_css(&t, &o).unwrap();
    assert!(i.report.skipped > 15_000);
    let work = TEXT_WORK.load(Ordering::SeqCst);
    assert!(
        work <= 4 * t.len(),
        "{work} bytes of text were cut for an input of {} bytes",
        t.len()
    );
}
