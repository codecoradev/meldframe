//! The bounds of the CSS reader: each limit cuts what it says and reports it, and the work grows with the text and
//! no faster, whatever the text is made of.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::{EntryKind, Limits, MAX_ENTRIES, Report, read};

fn limits(f: impl FnOnce(&mut Limits)) -> Limits {
    let mut l = Limits::default();
    f(&mut l);
    l
}

#[test]
fn the_defaults_are_the_numbers_of_adr_0024() {
    let l = Limits::default();
    assert_eq!((l.max_rules, l.max_value_bytes), (5_000, 1_000));
    assert_eq!(
        (
            l.max_decls,
            l.max_selector_bytes,
            l.max_depth,
            l.max_selectors
        ),
        (50_000, 4_096, 16, 10_000)
    );
}

#[test]
fn a_selector_list_of_exactly_the_limit_is_read_and_one_byte_more_is_not() {
    let ok = "a".repeat(4_096);
    let (s, r) = sheet(&format!("{ok}{{x:1}}"));
    assert_eq!(s.rules.len(), 1);
    assert!(r.entries.is_empty());
    let (s, r) = sheet(&format!("{ok}a{{x:1}} b{{y:2}}"));
    assert_eq!(dump(&s), ["b{y:2}"]);
    assert_eq!(count(&r, "selector_too_long"), 1);
    // Blanks around a list of the limit are not counted.
    let (s, _) = sheet(&format!("  {ok}  \n/**/{{x:1}}"));
    assert_eq!(s.rules.len(), 1);
}

#[test]
fn the_selector_limit_counts_over_texts_and_cuts_the_list_it_falls_in() {
    let l = limits(|l| l.max_selectors = 3);
    let mut r = Report::default();
    let s = read(["a,b{x:1}", "c,d{y:2}", "e{z:3}"], &l, &mut r);
    assert_eq!(dump(&s), ["a, b{x:1}", "c{y:2}"]);
    assert_eq!(count(&r, "selector_limit"), 1);
}

#[test]
fn rules_past_the_limit_are_not_read_and_the_read_stops_across_texts() {
    let mut r = Report::default();
    let l = limits(|l| l.max_rules = 3);
    let s = read(["a{x:1} b{x:1} c{x:1} d{x:1} e{x:1}", "f{x:1}"], &l, &mut r);
    assert_eq!(dump(&s), ["a{x:1}", "b{x:1}", "c{x:1}"]);
    assert_eq!(count(&r, "rule_limit"), 1);
    assert_eq!(r.entries[0].kind, EntryKind::Cut);
    // The default is 5,000.
    let many = "a{x:1}".repeat(5_001);
    let (s, r) = sheet(&many);
    assert_eq!(s.rules.len(), 5_000);
    assert_eq!(count(&r, "rule_limit"), 1);
}

#[test]
fn declarations_past_the_limit_are_left_out_and_counted_once() {
    let l = limits(|l| l.max_decls = 4);
    let (s, r) = sheet_with("a{x:1;y:2;z:3} b{x:1;y:2;z:3}", &l);
    assert_eq!(dump(&s), ["a{x:1; y:2; z:3}", "b{x:1}"]);
    assert_eq!(count(&r, "declaration_limit"), 1);
    // A `style` attribute has its own count.
    let mut r = Report::default();
    let d = html_import::css::read_declarations("a:1;b:2;c:3", 0, &l, &mut r);
    assert_eq!(d.len(), 3);
}

#[test]
fn a_value_over_the_limit_is_left_out_with_its_name_and_one_at_the_limit_is_kept() {
    let at = format!("a{{x:{};y:2}}", "v".repeat(1_000));
    let over = format!("a{{x:{};y:2;z:{}}}", "v".repeat(1_001), "w".repeat(100_000));
    let (s, r) = sheet(&at);
    assert_eq!(s.rules[0].decls.len(), 2);
    assert!(r.entries.is_empty());
    let (s, r) = sheet(&over);
    assert_eq!(dump(&s), ["a{y:2}"]);
    assert_eq!(names(&r, "value_too_long"), ["x", "z"]);
    // The piece that was read and not kept costs nothing in the report.
    assert!(
        r.entries
            .iter()
            .all(|e| e.name.len() <= 80 && e.message.len() <= 240)
    );
}

#[test]
fn a_selector_list_over_the_limit_skips_its_rule_and_the_next_rule_is_read() {
    let sel = vec!["a"; 3_000].join(",");
    let (s, r) = sheet(&format!("{sel}{{x:1}} b{{x:2}}"));
    assert_eq!(dump(&s), ["b{x:2}"]);
    assert_eq!(count(&r, "selector_too_long"), 1);
}

#[test]
fn layers_nested_past_the_limit_are_skipped() {
    let l = limits(|l| l.max_depth = 2);
    let (s, r) = sheet_with(
        "@layer a{@layer b{x{y:1} @layer c{z{y:2}} w{y:3}}} v{y:4}",
        &l,
    );
    assert_eq!(dump(&s), ["x{y:1}", "w{y:3}", "v{y:4}"]);
    assert_eq!(count(&r, "depth_cut"), 1);
    // A hundred thousand open layers is a loop, not a recursion.
    let deep = "@layer a{".repeat(100_000);
    let (s, r) = sheet(&deep);
    assert!(s.rules.is_empty());
    assert!(count(&r, "depth_cut") >= 1);
}

#[test]
fn the_report_is_bounded_and_counts_what_it_does_not_list() {
    let css = "a:hover{x:1}".repeat(4_000);
    let (_, r) = sheet(&css);
    assert_eq!(r.entries.len(), MAX_ENTRIES);
    assert_eq!(count(&r, "selector_pseudo_class"), 4_000);
    assert_eq!(r.omitted, 4_000 - MAX_ENTRIES);
}

/// The work of `css` and its length.
fn work(css: &str) -> (usize, usize) {
    let (_, r) = sheet(css);
    (r.work, css.len())
}

/// A text that stresses one thing, and the least work for each byte of it that the counter must count there. Where the
/// reader looks at a byte once (or skips a string or a comment it counts whole) the floor is 1: a step is a byte. A
/// piece of a selector is parsed again by the selector reader, which counts a step for each character, so the floors
/// are what each shape measured, less a tenth: a counter that is removed, halved or dropped on one path fails the
/// shape that reads through it.
type Shape = (&'static str, String, f64);

/// Texts that stress one thing each, scaled by `n`.
fn shapes(n: usize) -> Vec<Shape> {
    vec![
        ("rules", "a.b>c d{x:y;z:w}".repeat(n), 0.32),
        ("open braces", "{".repeat(n), 1.0),
        ("close braces", "}".repeat(n), 1.0),
        ("brackets", "a[".repeat(n), 1.0),
        ("parens", format!("a{{b:{}", "(".repeat(n)), 1.0),
        ("strings", format!("a{{b:{}", "\"x".repeat(n)), 1.2),
        ("comments", "/*".repeat(n), 1.1),
        ("escapes", "\\".repeat(n), 0.5),
        ("at-rules", "@media{".repeat(n), 1.0),
        ("layers", "@layer a{".repeat(n), 1.0),
        ("nested", format!("a{{{}", "b{".repeat(n)), 1.0),
        ("commas", format!("{}{{x:1}}", "a,".repeat(n)), 1.0),
        ("combinators", format!("{}{{x:1}}", "a>".repeat(n)), 1.0),
        ("long selector", format!("{}{{x:1}}", "a ".repeat(n)), 1.0),
        ("long value", format!("a{{b:{}}}", "v".repeat(n)), 1.0),
        ("many decls", format!("a{{{}}}", "b:c;".repeat(n)), 1.0),
        ("non-ascii", "\u{e9}\u{1f600}{x:\u{e9}}".repeat(n), 0.17),
        (
            "important",
            format!("a{{b:{}!important}}", " ".repeat(n)),
            1.0,
        ),
        // One string, and one comment, of the length of the text: the counter of the string arm and of the two comment
        // paths is exact here, so a counter that is gone or halved fails them.
        (
            "one long string",
            format!("a{{b:\"{}\"}}", "x".repeat(n)),
            1.0,
        ),
        (
            "one long string with escapes",
            format!("a{{b:\"{}\"}}", "\\x".repeat(n)),
            1.0,
        ),
        (
            "one long comment",
            format!("/*{}*/a{{b:c}}", "x".repeat(n)),
            1.0,
        ),
        (
            "one long unterminated comment",
            format!("a{{b:c}}/*{}", "x".repeat(n)),
            1.0,
        ),
        (
            "comments inside a value",
            format!("a{{b:c{}}}", "/**/".repeat(n)),
            1.2,
        ),
    ]
}

/// The work stays within six times the text and at least the floor of each shape, and doubles with the text.
#[test]
fn the_work_is_linear_in_the_text_for_every_shape() {
    for (name, css, floor) in shapes(20_000) {
        let (w, len) = work(&css);
        assert!(w <= 6 * len + 64, "{name}: {w} steps for {len} bytes");
        assert!(
            w as f64 >= floor * len as f64,
            "{name}: only {w} steps for {len} bytes (floor {floor})"
        );
    }
    for text in ["a", "{", ";", " ", "/*", "\\"] {
        assert!(work(text).0 > 0, "{text:?}");
    }
    assert_eq!(work("").0, 0);
    // Doubling the text at most doubles the work, up to a constant, and at least grows it by half.
    for ((name, a, _), (_, b, _)) in shapes(1_000).into_iter().zip(shapes(2_000)) {
        let (wa, _) = work(&a);
        let (wb, _) = work(&b);
        assert!(wb <= 2 * wa + 256, "{name}: {wa} then {wb}");
        assert!(2 * wb >= 3 * wa, "{name}: {wa} then {wb}");
    }
}

/// A measure that is not the counter: texts of a size at which a path that is quadratic in the text, counted or
/// not, would not end (a text of 2 MB is 4 * 10^12 steps squared), so the test ends, or it does not.
#[test]
fn every_shape_is_read_at_a_size_where_a_quadratic_path_would_not_end() {
    let n = if cfg!(target_arch = "wasm32") {
        20_000
    } else {
        120_000
    };
    for (name, css, _) in shapes(n) {
        let (s, r) = sheet(&css);
        assert!(r.work > 0 && s.rules.len() <= 5_000, "{name}");
    }
}

#[test]
fn a_megabyte_of_one_piece_is_read_and_not_kept() {
    let css = format!("a{{b:{}}} c{{d:e}}", "x".repeat(1 << 20));
    let (s, r) = sheet(&css);
    assert_eq!(dump(&s), ["c{d:e}"]);
    assert!(r.work <= 6 * css.len());
}
