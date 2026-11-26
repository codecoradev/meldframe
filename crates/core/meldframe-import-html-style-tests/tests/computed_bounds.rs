//! The computed style and its bounds: the budget of the stage, the sharing of custom properties and the notes.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::Limits;

#[test]
fn a_style_stage_over_its_budget_leaves_the_elements_past_it_with_the_inherited_values_only() {
    let html = format!(
        "<div style=\"color:#123456\">{}</div>",
        "<p style=\"--a:1; width:2px\">x</p>".repeat(100)
    );
    let limits = Limits {
        max_work: 60,
        ..Limits::default()
    };
    let (p, st, r) = styled_with(&html, "", &limits);
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    let ps: Vec<_> = p
        .tree
        .ids()
        .filter(|i| p.tree.element(*i).tag == "p")
        .collect();
    let done = ps
        .iter()
        .filter(|i| st.get(**i).get("width").is_some())
        .count();
    assert!(0 < done && done < 100, "{done}");
    // The cut ones still inherit.
    assert_eq!(st.get(*ps.last().unwrap()).color, "#123456");
    assert!(st.get(*ps.last().unwrap()).get("width").is_none());
}

#[test]
fn custom_properties_are_shared_until_an_element_adds_one_and_copying_them_is_counted() {
    let root: String = (0..2_000).map(|i| format!("--r{i}:{i};")).collect();
    let css = format!(":root{{{root}}} p{{--own:1}}");
    let html = "<p>x</p>".repeat(3_000);
    let (_, st, r) = styled(&html, &css);
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    // Not a copy of two thousand properties for each of three thousand elements.
    assert!(r.work < 3 * Limits::default().max_work, "{}", r.work);
    let (p, st, _) = styled("<div><p>x</p></div><p>y</p>", &css);
    let ps: Vec<_> = p
        .tree
        .ids()
        .filter(|i| p.tree.element(*i).tag == "p")
        .collect();
    assert_eq!(st.get(ps[0]).custom.len(), 2_001);
    let div = st.get(el(&p, "div"));
    assert!(std::sync::Arc::ptr_eq(
        &div.custom,
        &st.body.as_ref().unwrap().custom
    ));
}

#[test]
fn the_notes_of_an_element_are_bounded() {
    let decls: String = (0..40).map(|i| format!("width:var(--n{i});")).collect();
    let (p, st, r) = styled(&format!("<p style=\"{decls}\">x</p>"), "");
    assert_eq!(st.get(el(&p, "p")).notes.len(), 32);
    assert_eq!(count(&r, "note_omitted"), 8);
}

/// The steps of the style stage alone.
fn steps(html: &str, css: &str) -> usize {
    style_cost(html, css, &Limits::default()).1
}

#[test]
fn each_charge_of_the_style_budget_is_pinned_alone() {
    // The defaults are applied twice (to the element and to what `revert` gives back): the html has one (display),
    // the body two (display and margin), a div one: 2 + 4 + 2.
    assert_eq!(steps("<div>x</div>", ""), 8);
    // A declaration of a rule costs 1.
    assert_eq!(steps("<div>x</div>", "div{width:1px}"), 9);
    assert_eq!(
        steps("<div>x</div>", "div{width:1px;height:1px;top:1px}"),
        11
    );
    // A custom property costs what resolving it costs (1 and its replacements) and copying the map it joins (the
    // parent's entries, none at the root): `--a` on the root, then `--b` on the div, which copies one.
    assert_eq!(steps("<div>x</div>", ":root{--a:1px}"), 8 + 1);
    assert_eq!(
        steps("<div>x</div>", ":root{--a:1px} div{--b:2px}"),
        8 + 1 + (1 + 1)
    );
    // A `var()` costs 1 for the declaration and 1 for the resolution, one for each replacement and one for each 64
    // bytes looked at: `var(--a)` and `1px` are 11 bytes.
    assert_eq!(
        steps("<div>x</div>", ":root{--a:1px} div{width:var(--a)}"),
        8 + 1 + 1 + 2
    );
    let long = "x".repeat(130);
    assert_eq!(
        steps(
            "<div>x</div>",
            &format!(":root{{--a:{long}}} div{{width:var(--a)}}")
        ),
        8 + (1 + 2) + 1 + (1 + 1 + 2)
    );
    // A shorthand is one declaration, however many longhands it makes.
    assert_eq!(
        steps(
            "<div>x</div>",
            "div{margin:1px;padding:1px;border:1px solid red}"
        ),
        8 + 3
    );
    // A rule no element has costs nothing here.
    assert_eq!(steps("<div>x</div>", ".nope{width:1px}"), 8);
}

#[test]
fn a_sheet_that_makes_var_cost_a_lot_stops_far_under_the_budget_in_declarations() {
    // The most one declaration can cost: 110 uses of a 7-byte custom property (the value is at most 1,000 bytes), about
    // 140 steps, and 50 of them for each of 5,000 paragraphs. Charged one step each it was 250,000 declarations.
    let value = vec!["var(--k)"; 110].join(" ");
    let mut css = String::from(":root{--k:1234567}");
    for i in 0..50 {
        css.push_str(&format!("p{{a{i}:{value}}}"));
    }
    let html = "<p>x</p>".repeat(5_000);
    let l = Limits::default();
    let (st, work, r) = style_cost(&html, &css, &l);
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    // About 2,000,000 / 140 = 14,000 declarations, so 285 paragraphs, and not 5,000.
    let done = st
        .elements
        .iter()
        .filter(|s| s.props.contains_key("a0"))
        .count();
    assert!((200..400).contains(&done), "{done}");
    assert!(work <= l.max_work + 50 * 150, "{work}");
    // The elements after the cut only inherit.
    assert!(!st.elements.last().unwrap().props.contains_key("a0"));
}

#[test]
fn a_value_shared_with_its_rule_is_kept_once_and_the_bytes_retained_stay_bounded() {
    use std::collections::HashSet;
    // 200 rules of 900 bytes each, 400 paragraphs that every rule picks: 72 MB if each element copied each value (a
    // declaration of 900 bytes is 15 steps, so more paragraphs would reach the budget of steps).
    let css: String = (0..200)
        .map(|i| format!("p{{a{i}:{}{i}}}", "v".repeat(897)))
        .collect();
    let html = "<p>x</p>".repeat(400);
    let (st, _, _) = style_cost(&html, &css, &Limits::default());
    assert!(st.complete);
    let mut seen = HashSet::new();
    let mut distinct = 0;
    let mut entries = 0;
    for s in st.elements.iter().skip(1) {
        // The properties of the rules; the defaults of a paragraph make a few bytes of pieces for each element.
        for (_, p) in s.props.iter().filter(|(k, _)| k.starts_with('a')) {
            entries += 1;
            if seen.insert(std::sync::Arc::as_ptr(&p.value) as *const u8 as usize) {
                distinct += p.value.len();
            }
        }
    }
    assert!(entries >= 200 * 400, "{entries}");
    assert!(
        distinct <= css.len(),
        "{distinct} bytes held for {} of css",
        css.len()
    );
    // What is made is allocated: a `var()` result and the pieces of a shorthand are one each for each element.
    let css = ":root{--w:12345678901234567890} p{width:var(--w); margin:1px 2px 3px 4px}";
    let (st, _, _) = style_cost(&"<p>x</p>".repeat(10), css, &Limits::default());
    let mut ptrs = HashSet::new();
    for s in st.elements.iter().skip(1) {
        ptrs.insert(std::sync::Arc::as_ptr(&s.props["width"].value) as *const u8 as usize);
    }
    assert_eq!(ptrs.len(), 10);
}

#[test]
fn the_bytes_a_style_stage_keeps_are_bounded_and_the_elements_past_them_only_inherit() {
    // Each paragraph makes 16 properties of a var() result of 800 bytes: about 13 KB an element.
    let css = format!(
        ":root{{--v:{}}} p{{{}}}",
        "x".repeat(800),
        (0..16)
            .map(|i| format!("a{i}:var(--v);"))
            .collect::<String>()
    );
    let l = Limits {
        max_style_bytes: 200_000,
        ..Limits::default()
    };
    let (st, work, r) = style_cost(&"<p>x</p>".repeat(1_000), &css, &l);
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    assert!(work < l.max_work);
    let held: usize = st
        .elements
        .iter()
        .flat_map(|s| s.props.values())
        .filter(|p| p.source != html_import::css::Source::Inherited)
        .map(|p| p.value.len())
        .sum();
    assert!(held <= l.max_style_bytes + 16 * 900, "{held}");
    assert!(held > l.max_style_bytes / 2, "{held}");
    // The message says which bound it was.
    assert!(r.entries[0].message.contains("200000 bytes"));
}

#[test]
fn a_declaration_is_charged_a_step_and_a_step_for_each_64_bytes_of_its_value() {
    let at = |len: usize| steps("<div>x</div>", &format!("div{{width:{}}}", "1".repeat(len)));
    assert_eq!((at(1), at(63)), (9, 9));
    assert_eq!((at(64), at(127)), (10, 10));
    assert_eq!((at(128), at(200), at(990)), (11, 12, 8 + 1 + 15));
    // A shorthand is charged for the text it cuts up: a font of 990 bytes is 16 steps, not 1.
    let font = format!("div{{font:12px {}}}", "a".repeat(980));
    assert_eq!(steps("<div>x</div>", &font), 8 + 1 + 990 / 64);
}

#[test]
fn many_long_shorthands_over_many_elements_are_cut_with_the_work_bounded() {
    // 255 rules of a font of 990 bytes, over 5,000 paragraphs: 250 KB of css. Each declaration is 16 steps and its
    // pieces are allocated for each element (about 1.4 KB), so the byte budget of 64 MiB ends it first, after about
    // 180 paragraphs, and the steps stay under their own budget; the time is not asserted, the work is.
    let css = format!("p{{font:12px {}}}", "a".repeat(980)).repeat(255);
    let (st, work, r) = style_cost(&"<p>x</p>".repeat(5_000), &css, &Limits::default());
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    let done = st
        .elements
        .iter()
        .filter(|s| s.props.contains_key("font-size"))
        .count();
    assert!((100..700).contains(&done), "{done}");
    assert!(work <= Limits::default().max_work, "{work}");
}

/// The bytes the style stage kept before its last element: the smallest `max_style_bytes` that cuts nothing. The last
/// element of `html` is the one that is not counted, so it is an `<i>` that makes nothing.
fn bytes(html: &str, css: &str) -> usize {
    let (mut lo, mut hi) = (0usize, 1usize << 22);
    while lo < hi {
        let mid = (lo + hi) / 2;
        let l = Limits {
            max_style_bytes: mid,
            ..Limits::default()
        };
        if style_cost(html, css, &l).0.complete {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

#[test]
fn each_charge_of_the_byte_budget_is_pinned_alone() {
    // 64 bytes for each property put, and the text that was made. The defaults are applied twice: the html has one
    // property (display), the body two (display and a margin of four longhands), the div one: 2 * 64, 2 * (64 +
    // 4 * 64 + 46 + 12) + 2 * 64, 2 * 64.
    let page = "<div></div><i></i>";
    let base = bytes(page, "");
    assert_eq!(base, 128 + (128 + 2 * (64 + 4 * 64 + 46 + 12)) + 128);
    // A declaration of a rule: one entry.
    assert_eq!(bytes(page, "div{width:1px}"), base + 64);
    // A shorthand: one entry, and for each longhand an entry and its name and value, which were made.
    assert_eq!(
        bytes(page, "div{padding:1px}"),
        base + 64 + 4 * 64 + 50 + 4 * 3
    );
    // A value that is made by var(): its bytes.
    assert_eq!(
        bytes(page, ":root{--a:12345} div{width:var(--a)}"),
        base + 64 + 5
    );
    // What an element inherits: an entry for each property, in the child.
    let nested = "<div><span></span></div><i></i>";
    let plain = bytes(nested, "");
    assert_eq!(bytes(nested, "div{color:#f00}"), plain + 64 + 64);
    // The custom properties an element copies from its parent: an entry for each one it copies, and none for the root.
    assert_eq!(bytes(page, ":root{--a:1px}"), base);
    assert_eq!(bytes(page, ":root{--a:1px} div{--b:2px}"), base + 64);
    assert_eq!(
        bytes(page, ":root{--a:1px;--c:3px} div{--b:2px}"),
        base + 2 * 64
    );
    // A note: 128 bytes, its property and its value.
    let note = |color: &str| bytes(&format!("<div style=\"color:{color}\"></div><i></i>"), "");
    assert_eq!(note("lab(0 0 0)"), note("currentcolor") + 128 + 5 + 10);
}

#[test]
fn many_short_properties_over_many_elements_are_cut_by_the_byte_budget_before_the_step_budget() {
    // 300 properties of a paragraph make 19 KB of entries for each of 200 paragraphs: 60,000 steps, 3.8 MB. With a
    // budget of 1 MB of bytes the stage stops after about 52 paragraphs, with the steps far under theirs.
    let css: String = (0..300).map(|i| format!("p{{a{i}:1}}")).collect();
    let l = Limits {
        max_style_bytes: 1_000_000,
        ..Limits::default()
    };
    let (st, work, r) = style_cost(&"<p>x</p>".repeat(200), &css, &l);
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    assert!(work < 100_000, "{work}");
    assert!(
        r.entries[0].message.contains("1000000 bytes"),
        "{}",
        r.entries[0].message
    );
    let done = st
        .elements
        .iter()
        .filter(|s| s.props.contains_key("a0"))
        .count();
    assert!((45..60).contains(&done), "{done}");
}
