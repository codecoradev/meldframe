//! The Tailwind classes in the cascade: class rules after the sheet and under the style attribute, a utility against a
//! rule of the sheet by specificity, and the custom property of an `after:` utility, which is the ::after box's own.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;

#[test]
fn classes_are_cascaded_as_class_rules_after_the_sheet_and_under_the_style_attribute() {
    let html = r#"<div class="flex flex-col gap-[8px] p-[4px_8px]" style="gap:1px">x</div>"#;
    assert_eq!(
        main(
            html,
            "div{gap:2px;padding:0;display:block} #nope{x:1}",
            "div"
        ),
        // The type rule is below a class, the inline style above it, and the classes keep the order written.
        [
            "gap:2px",
            "padding:0",
            "display:block",
            "display:flex",
            "flex-direction:column",
            "gap:8px",
            "padding:4px 8px",
            "gap:1px@"
        ]
    );
    // A class rule of the sheet comes first at the same specificity, and a more specific rule beats a class.
    assert_eq!(
        main(
            r#"<p id=a class="flex y">"#,
            ".y{x:1} #a{display:block}",
            "p"
        ),
        ["x:1", "display:flex", "display:block"]
    );
}

#[test]
fn a_utility_meets_a_rule_of_the_sheet_by_specificity_and_then_the_utility_is_last() {
    // One class each: the utility is after every rule of the sheet and wins.
    let html = r#"<p class="hero w-[2px]">t</p>"#;
    assert_eq!(
        main(html, ".hero{width:1px}", "p"),
        ["width:1px", "width:2px"]
    );
    // Two classes in a rule beat one in a utility, and so does an id.
    let html = r#"<p id=i class="a b w-[2px]">t</p>"#;
    assert_eq!(
        main(html, ".a.b{width:1px}", "p"),
        ["width:2px", "width:1px"]
    );
    assert_eq!(main(html, "#i{width:1px}", "p"), ["width:2px", "width:1px"]);
    // The box of the border: `.a::after` and `after:inset-[-1px]` are as specific, and the utility is last.
    let (p, c, _) = cascaded(
        r#"<div class="a after:inset-[-1px] after:absolute">t</div>"#,
        ".a::after{inset:0;position:relative}",
    );
    assert_eq!(
        list(&c.elements[el(&p, "div").index()].after),
        [
            "inset:0",
            "position:relative",
            "inset:-1px",
            "position:absolute"
        ]
    );
    // A rule that is more specific than a class on the box wins over the utility.
    let (p, c, _) = cascaded(
        r#"<div id=d class="after:inset-[-1px]">t</div>"#,
        "#d::after{inset:0}",
    );
    assert_eq!(
        list(&c.elements[el(&p, "div").index()].after),
        ["inset:-1px", "inset:0"]
    );
}

#[test]
fn a_custom_property_in_an_after_utility_is_the_boxs_own_and_is_no_property() {
    let html = r#"<div class="[--q:3px] after:[--r:5px] after:w-[var(--r)] after:[height:var(--q)] after:content-['']">t</div>"#;
    let (p, st, r) = styled(html, "");
    let d = st.get(el(&p, "div"));
    let after = d.after.as_ref().expect("the ::after box");
    // `--r` is resolved on the box, `--q` is the element's, and no custom property is a property of the box.
    assert_eq!(
        (&*after["width"].value, &*after["height"].value),
        ("5px", "3px")
    );
    assert!(
        after.keys().all(|k| !k.starts_with("--")),
        "{:?}",
        after.keys().collect::<Vec<_>>()
    );
    // The element has its own and not the box's.
    assert_eq!(
        d.custom.get("--q").cloned().flatten().as_deref(),
        Some("3px")
    );
    assert!(!d.custom.contains_key("--r") && d.props.keys().all(|k| !k.starts_with("--")));
    assert_eq!(count(&r, "class_unresolved"), 0);
    // The same rules written as css read the same.
    let (p, st2, _) = styled(
        r#"<div class="x">t</div>"#,
        ".x{--q:3px} .x::after{--r:5px;width:var(--r);height:var(--q);content:\"\"}",
    );
    assert_eq!(st2.get(el(&p, "div")).after, st.get(el(&p, "div")).after);
}
