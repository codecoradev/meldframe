//! The Tailwind decoder: the vocabulary of the export's `tw.rs` read back to declarations, the `after:` variant,
//! `_` and `\_`, what is not read and why, and a guard that the vocabulary is still the
//! export's.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::Pseudo;
use html_import::tw::{Decoded, Unresolved, decode};

fn dec(class: &str) -> Decoded {
    decode(class, 1_000).unwrap_or_else(|e| panic!("{class} is not read: {e:?}"))
}

/// `prop:value` of every declaration of a class, with `::after` when it is for the generated box.
fn read(class: &str) -> String {
    let d = dec(class);
    let list: Vec<String> = d.decls.iter().map(|(p, v)| format!("{p}:{v}")).collect();
    format!(
        "{}{}",
        list.join(";"),
        if d.pseudo == Some(Pseudo::After) {
            " ::after"
        } else {
            ""
        }
    )
}

fn why(class: &str) -> &'static str {
    match decode(class, 1_000) {
        Ok(d) => panic!("{class} is read as {d:?}"),
        Err(Unresolved(w)) => w,
    }
}

#[test]
fn the_named_utilities_of_the_export_are_read() {
    for (class, want) in [
        ("absolute", "position:absolute"),
        ("relative", "position:relative"),
        ("flex", "display:flex"),
        ("flex-row", "flex-direction:row"),
        ("flex-col", "flex-direction:column"),
        ("shrink-0", "flex-shrink:0"),
        ("overflow-hidden", "overflow:hidden"),
        ("pointer-events-none", "pointer-events:none"),
        ("self-stretch", "align-self:stretch"),
        ("items-start", "align-items:flex-start"),
        ("items-center", "align-items:center"),
        ("items-end", "align-items:flex-end"),
        ("justify-start", "justify-content:flex-start"),
        ("justify-between", "justify-content:space-between"),
        ("text-center", "text-align:center"),
        ("text-right", "text-align:right"),
        ("whitespace-pre", "white-space:pre"),
        ("whitespace-pre-wrap", "white-space:pre-wrap"),
        ("w-fit", "width:fit-content"),
        ("w-auto", "width:auto"),
        ("h-auto", "height:auto"),
        ("content-['']", "content:\"\""),
    ] {
        assert_eq!(read(class), want, "{class}");
    }
}

#[test]
fn the_arbitrary_value_utilities_of_the_export_are_read() {
    for (class, want) in [
        ("left-[10px]", "left:10px"),
        ("top-[-4.5px]", "top:-4.5px"),
        ("inset-[-1px]", "inset:-1px"),
        ("w-[100px]", "width:100px"),
        ("h-[40px]", "height:40px"),
        ("min-w-[20px]", "min-width:20px"),
        ("min-h-[0]", "min-height:0"),
        ("gap-[8px]", "gap:8px"),
        ("p-[4px_8px]", "padding:4px 8px"),
        ("m-[0_0_0_4px]", "margin:0 0 0 4px"),
        ("flex-[1_1_0%]", "flex:1 1 0%"),
        ("text-[length:16px]", "font-size:16px"),
        ("font-[600]", "font-weight:600"),
        ("leading-[19px]", "line-height:19px"),
        ("text-[color:#112233]", "color:#112233"),
        (
            "bg-[color:var(--c-brand)]",
            "background-color:var(--c-brand)",
        ),
        (
            "rounded-[calc(var(--r)_*_1px)]",
            "border-radius:calc(var(--r) * 1px)",
        ),
        ("opacity-[0.5]", "opacity:0.5"),
        (
            "shadow-[0px_2px_4px_0px_#00000040,0px_1px_1px_0px_#fff]",
            "box-shadow:0px 2px 4px 0px #00000040,0px 1px 1px 0px #fff",
        ),
    ] {
        assert_eq!(read(class), want, "{class}");
    }
}

#[test]
fn an_arbitrary_property_is_read_and_underscores_are_spaces_unless_escaped() {
    for (class, want) in [
        ("[box-sizing:border-box]", "box-sizing:border-box"),
        ("[Word-Break:keep-all]", "word-break:keep-all"),
        ("[--c-brand:#3366ff]", "--c-brand:#3366ff"),
        ("[--Mixed:1]", "--Mixed:1"),
        ("[border:2px_solid_#000]", "border:2px solid #000"),
        (
            "[font-family:'Inter_Variable']",
            "font-family:'Inter Variable'",
        ),
        (
            "[background-image:url(a\\_b.png),linear-gradient(90deg,red_0%,blue_100%)]",
            "background-image:url(a_b.png),linear-gradient(90deg,red 0%,blue 100%)",
        ),
        (
            "[justify-content:safe_center]",
            "justify-content:safe center",
        ),
        (
            "[background-image:url(a\\3a_b)]",
            "background-image:url(a\\3a b)",
        ),
        ("[content:'a:b']", "content:'a:b'"),
    ] {
        assert_eq!(read(class), want, "{class}");
    }
}

#[test]
fn the_after_variant_is_the_generated_box() {
    for (class, want) in [
        ("after:content-['']", "content:\"\" ::after"),
        ("after:absolute", "position:absolute ::after"),
        ("after:pointer-events-none", "pointer-events:none ::after"),
        ("after:inset-[-1px]", "inset:-1px ::after"),
        (
            "after:[border:2px_solid_#000]",
            "border:2px solid #000 ::after",
        ),
        ("after:rounded-[5px]", "border-radius:5px ::after"),
    ] {
        assert_eq!(read(class), want, "{class}");
    }
}

#[test]
fn what_is_not_in_the_vocabulary_is_unresolved_with_its_reason() {
    for (class, reason) in [
        ("p-4", "not_in_vocabulary"),
        ("text-lg", "not_in_vocabulary"),
        ("bg-blue-500", "not_in_vocabulary"),
        ("rounded-lg", "not_in_vocabulary"),
        ("flex-wrap", "not_in_vocabulary"),
        ("hero", "not_in_vocabulary"),
        ("-m-[4px]", "not_in_vocabulary"),
        ("flex!", "not_in_vocabulary"),
        ("!flex", "not_in_vocabulary"),
        ("fixed", "not_in_vocabulary"),
        ("[unclosed", "not_in_vocabulary"),
        ("[nocolon]", "not_in_vocabulary"),
        ("[1a:b]", "not_in_vocabulary"),
        ("[:b]", "not_in_vocabulary"),
        ("hover:underline", "variant"),
        ("md:flex", "variant"),
        ("dark:bg-[color:#000]", "variant"),
        ("before:content-['']", "variant"),
        ("after:hover:flex", "not_in_vocabulary"),
        ("after:p-4", "not_in_vocabulary"),
        ("w-[10px", "not_closed"),
        ("w-[]", "empty_value"),
        ("[width:]", "empty_value"),
        ("w-[1px;color:red]", "invalid_value"),
        ("[x:a}b]", "invalid_value"),
        ("[x:{]", "invalid_value"),
        ("", "not_in_vocabulary"),
    ] {
        assert_eq!(why(class), reason, "{class:?}");
    }
    assert_eq!(
        decode(&format!("w-[{}]", "1".repeat(1_001)), 1_000),
        Err(Unresolved("value_too_long"))
    );
    assert!(decode(&format!("w-[{}]", "1".repeat(1_000)), 1_000).is_ok());
}

/// The lines of the export's `tw.rs` that make up its vocabulary: `("prop", "value") => Some("class")` and
/// `"prop" => "prefix"`.
type Vocabulary = (Vec<(String, String, String)>, Vec<(String, String)>);

fn export_vocabulary() -> Vocabulary {
    let src = include_str!("../../../export/export/src/tw.rs");
    let quoted =
        |s: &str| -> Vec<String> { s.split('"').skip(1).step_by(2).map(str::to_owned).collect() };
    let (mut fixed, mut named) = (Vec::new(), Vec::new());
    for line in src.lines().map(str::trim) {
        if let Some((l, r)) = line.split_once(" => ") {
            let (l, r) = (quoted(l), quoted(r));
            if line.contains("Some(\"") && l.len() == 2 && r.len() == 1 && l[0] != "position" {
                fixed.push((l[0].clone(), l[1].clone(), r[0].clone()));
            } else if line.starts_with('"')
                && l.len() == 1
                && r.len() == 1
                && line.ends_with("\",")
                && !line.contains("Some")
            {
                named.push((l[0].clone(), r[0].clone()));
            }
        }
    }
    (fixed, named)
}

#[test]
fn the_vocabulary_is_still_the_exports() {
    let (fixed, named) = export_vocabulary();
    // The export writes 19 fixed utilities besides the position and `content-['']` (a quote in it that the line
    // parser cannot cut, so it is looked at below), and 19 prefixes.
    assert!(
        fixed.len() == 19 && named.len() == 19,
        "{} fixed, {} named",
        fixed.len(),
        named.len()
    );
    for (prop, value, class) in &fixed {
        assert_eq!(dec(class).decls, [(prop.clone(), value.clone())], "{class}");
    }
    for (prop, prefix) in &named {
        let class = if prefix.ends_with(':') {
            format!("{prefix}VALUE]")
        } else {
            format!("{prefix}[VALUE]")
        };
        assert_eq!(
            dec(&class).decls,
            [(prop.clone(), "VALUE".into())],
            "{class}"
        );
    }
    let src = include_str!("../../../export/export/src/tw.rs");
    assert!(
        src.contains("(\"position\", _) => Some(value)")
            && src.contains("content-['']")
            && src.contains("after:")
    );
}

#[test]
fn a_value_with_a_bang_is_not_a_value() {
    for class in [
        "[width:10px_!important]",
        "w-[10px!]",
        "after:[color:red_!important]",
        "text-[color:#fff_!important]",
    ] {
        assert_eq!(why(class), "invalid_value", "{class}");
    }
    // The bang that is only text is refused with it: a string has no `!` in the export's vocabulary.
    assert_eq!(why("[content:'a!']"), "invalid_value");
}

#[test]
fn class_unresolved_is_listed_not_refused_and_does_not_count_as_a_lost_property() {
    use html_import::css::EntryKind;
    let (_, _, r) = cascaded(r#"<p class="hero p-4 hover:flex">t</p>"#, ".nothing{x:1}");
    assert_eq!(r.entries.len(), 3);
    assert!(
        r.entries
            .iter()
            .all(|e| e.kind == EntryKind::Listed && e.code == "class_unresolved")
    );
    assert_eq!(EntryKind::Listed.as_str(), "listed");
    // A refused selector is a refusal, and is not the same kind.
    let (_, _, r) = cascaded(r#"<p class="hero">t</p>"#, "a:hover{x:1}");
    assert!(r.entries.iter().any(|e| e.kind == EntryKind::Refused));
    assert!(r.entries.iter().any(|e| e.kind == EntryKind::Listed));
}

#[test]
fn three_hundred_unresolved_classes_keep_the_report_at_its_cap_and_are_all_counted() {
    use html_import::css::{EntryKind, MAX_ENTRIES};
    let classes: String = (0..300).map(|i| format!("u{i} ")).collect();
    let html = format!("<p class=\"{classes}\">t</p><p class=\"{classes}\">u</p>");
    let (p, st, r) = styled(&html, "");
    assert_eq!(
        r.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Listed)
            .count(),
        MAX_ENTRIES
    );
    assert_eq!(count(&r, "class_unresolved"), 600);
    assert_eq!(r.omitted, 0);
    assert_eq!(st.get(el(&p, "p")).unresolved.len(), 32);
}

#[test]
fn a_class_that_a_rule_names_belongs_to_the_rule_and_is_not_decoded() {
    let (p, c, r) = cascaded(
        r#"<p class="flex hero">x</p>"#,
        ".flex{display:block} .hero{x:1}",
    );
    assert_eq!(
        list(&c.elements[el(&p, "p").index()].main),
        ["display:block", "x:1"]
    );
    assert_eq!(count(&r, "class_unresolved"), 0);
    // Even when the name is in a longer selector, or one the reader refuses to match.
    let (p, c, _) = cascaded(r#"<p class="flex">x</p>"#, ".a .flex{y:1}");
    assert!(c.elements[el(&p, "p").index()].main.is_empty());
}

#[test]
fn unresolved_classes_are_listed_once_on_the_element_and_counted_and_reported_once_each() {
    let html = r#"<p class="p-4 hover:flex flex p-4 hero">a</p><p class="p-4 text-lg">b</p>"#;
    let (p, st, r) = styled(html, "");
    let first = st.get(el(&p, "p"));
    assert_eq!(
        first.unresolved.iter().map(|c| &**c).collect::<Vec<_>>(),
        ["p-4", "hover:flex", "hero"]
    );
    assert_eq!(get_first(&first.props, "display"), "flex");
    assert_eq!(count(&r, "class_unresolved"), 6);
    assert_eq!(
        names(&r, "class_unresolved"),
        ["p-4", "hover:flex", "hero", "text-lg"]
    );
    assert!(r.entries[0].message.contains("not_in_vocabulary"));
    // At most 32 are listed, however many there are.
    let many: String = (0..100).map(|i| format!("u{i} ")).collect();
    let (p, st, _) = styled(
        &format!("<p class=\"{}\">", &many[..many.len().min(990)]),
        "",
    );
    assert_eq!(st.get(el(&p, "p")).unresolved.len(), 32);
}

fn get_first(props: &html_import::css::Props, name: &str) -> String {
    props
        .get(name)
        .map(|p| p.value.to_string())
        .unwrap_or_default()
}

#[test]
fn the_tailwind_form_of_an_export_gives_the_same_style_as_its_css_form() {
    let css_form = "[data-node=n1]{position:absolute;left:10px;top:20px;width:100px;height:auto;display:flex;flex-direction:row;\
        gap:8px;padding:4px 8px;align-items:center;justify-content:space-between;overflow:hidden;background-color:#ffffff;\
        border-radius:5px;opacity:0.5;box-shadow:0px 2px 4px 0px #00000040}\n\
        [data-node=n1]::after{content:\"\";position:absolute;pointer-events:none;inset:-1px;border:2px solid #000000;border-radius:6px}\n\
        [data-node=n2]{width:auto;min-width:40px;font-size:16px;font-weight:600;line-height:19px;white-space:pre;\
        text-align:center;color:#112233;flex:1 1 0%;margin:0 0 0 4px;align-self:stretch;flex-shrink:0}";
    let tw_form = "<div data-node=n1 class=\"absolute left-[10px] top-[20px] w-[100px] h-auto flex flex-row gap-[8px] \
        p-[4px_8px] items-center justify-between overflow-hidden bg-[color:#ffffff] rounded-[5px] opacity-[0.5] \
        shadow-[0px_2px_4px_0px_#00000040] after:content-[''] after:absolute after:pointer-events-none after:inset-[-1px] \
        after:[border:2px_solid_#000000] after:rounded-[6px]\"><p data-node=n2 class=\"w-auto min-w-[40px] text-[length:16px] \
        font-[600] leading-[19px] whitespace-pre text-center text-[color:#112233] flex-[1_1_0%] m-[0_0_0_4px] self-stretch shrink-0\">x</p></div>";
    let css_html = "<div data-node=n1><p data-node=n2>x</p></div>";
    let (pa, a, ra) = styled(css_html, css_form);
    let (pb, b, rb) = styled(tw_form, "");
    for tag in ["div", "p"] {
        let (x, y) = (a.get(el(&pa, tag)), b.get(el(&pb, tag)));
        let strip = |s: &html_import::css::Style| {
            props(s)
                .into_iter()
                .chain(
                    s.after
                        .iter()
                        .flat_map(|a| a.iter().map(|(k, v)| format!("after {k}:{}", v.value))),
                )
                .collect::<Vec<_>>()
        };
        assert_eq!(strip(x), strip(y), "{tag}");
        assert_eq!(
            (x.color.as_str(), x.font_size, x.font_weight),
            (y.color.as_str(), y.font_size, y.font_weight)
        );
        assert!(y.unresolved.is_empty() && y.notes.is_empty());
    }
    assert_eq!(
        count(&ra, "class_unresolved") + count(&rb, "class_unresolved"),
        0
    );
    assert!(b.get(el(&pb, "div")).after.is_some());
}

#[test]
fn hostile_classes_never_panic_and_cost_work_in_line_with_their_length() {
    let doc = "after:[border:2px_solid_#000]w-[a\\_b]text-[color:#fff]flex-col \u{e9}\u{1f600}[\\";
    for i in 0..=doc.len() {
        if doc.is_char_boundary(i) {
            let _ = decode(&doc[..i], 1_000);
        }
    }
    for text in [
        "[".repeat(50_000),
        ":".repeat(50_000),
        "after:".repeat(10_000),
        "\\_".repeat(50_000),
        "[a:".repeat(20_000),
    ] {
        let _ = decode(&text, 1_000);
    }
    // A hundred classes on each of 250 elements is read in time and leaves the cascade complete.
    let html = format!("<p class=\"{}\">x</p>", "p-4 flex ".repeat(100)).repeat(250);
    let (_, st, r) = styled(&html, "");
    assert!(st.complete);
    assert!(r.work < 400_000, "{}", r.work);
}
