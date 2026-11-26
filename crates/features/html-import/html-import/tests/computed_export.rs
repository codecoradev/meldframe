//! The computed style of what the export writes: the boxes it generates for a border, its own stylesheet and a node rule.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "style_common/mod.rs"]
mod css_common;
use css_common::*;

fn get(s: &html_import::css::Style, p: &str) -> String {
    s.get(p).unwrap_or("<none>").to_owned()
}

#[test]
fn the_generated_boxes_have_declared_properties_only_with_var_and_shorthands_read() {
    let css = ":root{--w:2px} [data-node=n1]{color:#f00} \
               [data-node=n1]::after{content:\"\"; position:absolute; pointer-events:none; inset:-1px; \
               border:var(--w) solid #000; border-radius:5px; color:inherit; font-size:99px}";
    let s = style_of("<div data-node=n1 style=\"top:0\">t</div>", css, "div");
    let after = s.after.expect("::after");
    let names: Vec<&str> = after.keys().map(|k| &**k).collect();
    assert!(
        names.contains(&"content")
            && names.contains(&"left")
            && names.contains(&"border-top-width")
    );
    assert!(names.contains(&"border-bottom-right-radius"));
    assert_eq!(&*after["content"].value, "\"\"");
    assert_eq!(&*after["left"].value, "-1px");
    assert_eq!(&*after["border-left-width"].value, "2px");
    // Nothing is taken from the element: `color:inherit` finds no parent, and `top:0` of the style attribute is
    // not there (the `top` is the one of `inset`).
    assert!(!after.contains_key("color") && &*after["top"].value == "-1px");
    assert!(s.before.is_none());
    assert_eq!(s.font_size, 16.0);
}

#[test]
fn the_exports_own_stylesheet_and_a_node_rule_are_read_as_the_export_wrote_them() {
    let css = "*,::before,::after{box-sizing:border-box}body{margin:0;font-family:'Inter Variable'}p{margin:0}\n\
        :root{--c-brand:#3366ff}\n\
        [data-node=\"n1\"]{position:absolute;left:10px;top:20px;width:100px;height:auto;display:flex;flex-direction:row;\
        gap:8px;padding:4px 8px;align-items:center;justify-content:safe center;overflow:hidden;\
        background-color:var(--c-brand);border-radius:calc(var(--c-brand) * 1px)}\n\
        [data-node=\"n2\"]{font-size:16px;font-weight:600;line-height:19px;white-space:pre;word-break:keep-all;\
        text-align:center;color:#112233;width:auto;min-width:40px;margin:0 0 0 4px}";
    let (p, st, _) = styled("<div data-node=n1><p data-node=n2>x</p></div>", css);
    let d = st.get(el(&p, "div"));
    assert_eq!(get(d, "left"), "10px");
    assert_eq!(get(d, "justify-content"), "safe center");
    assert_eq!(get(d, "padding-right"), "8px");
    assert_eq!(get(d, "background-color"), "#3366ff");
    assert_eq!(get(d, "border-top-left-radius"), "calc(#3366ff * 1px)");
    let t = st.get(el(&p, "p"));
    assert_eq!(
        (t.font_size, t.font_weight, t.color.as_str()),
        (16.0, 600.0, "#112233")
    );
    assert_eq!(
        (get(t, "margin-left"), get(t, "margin-top")),
        ("4px".into(), "0".into())
    );
    assert_eq!(get(t, "white-space"), "pre");
    assert_eq!(get(t, "font-family"), "'Inter Variable'");
    assert!(d.notes.iter().all(|n| n.property == "border-radius"));
}
