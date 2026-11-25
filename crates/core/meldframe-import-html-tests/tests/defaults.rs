//! The browser's defaults, as the small table the note lists, the inherited properties and their initial values.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use html_import::css::defaults::{INHERITED, inherits, initial, ua};
use html_import::html::parse;

/// The defaults of the first element of `html`, as `name:value`.
fn of(html: &str) -> Vec<String> {
    let p = parse(html, &Default::default()).unwrap();
    let id = p.tree.ids().next().unwrap();
    ua(p.tree.element(id))
        .iter()
        .map(|(n, v)| format!("{n}:{v}"))
        .collect()
}

#[test]
fn headings_have_their_size_weight_and_margins() {
    for (tag, size, margin) in [
        ("h1", "2em", "0.67em 0"),
        ("h2", "1.5em", "0.83em 0"),
        ("h3", "1.17em", "1em 0"),
        ("h4", "1em", "1.33em 0"),
        ("h5", "0.83em", "1.67em 0"),
        ("h6", "0.67em", "2.33em 0"),
    ] {
        assert_eq!(
            of(&format!("<{tag}>")),
            [
                "display:block".into(),
                format!("font-size:{size}"),
                "font-weight:bold".into(),
                format!("margin:{margin}")
            ]
        );
    }
}

#[test]
fn the_other_elements_of_the_note_have_what_the_note_says() {
    for (html, want) in [
        ("<body>", &["display:block", "margin:8px"][..]),
        ("<p>", &["display:block", "margin:1em 0"]),
        (
            "<ul>",
            &["display:block", "margin:1em 0", "padding-left:40px"],
        ),
        (
            "<ol>",
            &["display:block", "margin:1em 0", "padding-left:40px"],
        ),
        ("<blockquote>", &["display:block", "margin:1em 40px"]),
        (
            "<pre>",
            &["display:block", "margin:1em 0", "white-space:pre"],
        ),
        ("<strong>", &["font-weight:bold"]),
        ("<b>", &["font-weight:bold"]),
        ("<small>", &["font-size:0.83em"]),
        ("<li>", &["display:list-item"]),
        ("<a href=x>", &["color:#0000ee"]),
        ("<td>", &["display:table-cell"]),
        ("<tr>", &["display:table-row"]),
        ("<table>", &["display:table"]),
        (
            "<th>",
            &[
                "display:table-cell",
                "font-weight:bold",
                "text-align:center",
            ],
        ),
        ("<div>", &["display:block"]),
        ("<section>", &["display:block"]),
        ("<style>", &["display:none"]),
        ("<head>", &["display:none"]),
        ("<template>", &["display:none"]),
        ("<div hidden>", &["display:block", "display:none"]),
        ("<input type=hidden>", &["display:none"]),
    ] {
        assert_eq!(of(html), want, "{html}");
    }
}

#[test]
fn what_is_not_in_the_table_has_no_default_and_a_link_needs_its_href() {
    for html in [
        "<span>",
        "<em>",
        "<button>",
        "<input>",
        "<img>",
        "<x-custom>",
        "<a>",
        "<svg>",
    ] {
        assert!(of(html).is_empty(), "{html}");
    }
}

#[test]
fn the_inherited_properties_are_the_note_and_their_initial_values_are_set() {
    for p in [
        "color",
        "font-size",
        "font-weight",
        "text-align",
        "white-space",
        "font-family",
        "line-height",
        "visibility",
    ] {
        assert!(inherits(p), "{p}");
    }
    for p in [
        "display",
        "padding-left",
        "margin-top",
        "background-color",
        "width",
        "--x",
        "border-top-width",
    ] {
        assert!(!inherits(p), "{p}");
    }
    assert!(
        INHERITED.windows(2).all(|w| w[0] < w[1]),
        "sorted, so that it reads at a glance"
    );
    // Every inherited property has an initial value but font-family, which has none the mapping needs.
    for p in INHERITED {
        assert_eq!(initial(p).is_none(), p == "font-family", "{p}");
    }
    assert_eq!(
        (
            initial("color"),
            initial("font-size"),
            initial("text-align")
        ),
        (Some("#000000"), Some("16px"), Some("start"))
    );
    assert_eq!(initial("width"), None);
}
