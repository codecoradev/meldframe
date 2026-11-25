//! The tree the builder makes from well-formed and slightly loose HTML: fragments and documents, names, attributes,
//! text and entities, void elements, the implied end tags, the elements that are read as raw text, and what the
//! builder leaves out.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::{Child, Limits, ParseError, parse};

#[test]
fn a_fragment_is_the_children_of_the_root() {
    assert_eq!(
        shape(r#"<div class="a b"><p>Hello</p></div><span>x</span>"#),
        r#"div(class=a b){p{"Hello"}} span{"x"}"#
    );
}

#[test]
fn a_document_keeps_html_head_and_body_as_elements_and_drops_the_doctype_and_comments() {
    let p = run(
        "<!DOCTYPE html><html lang=en><head><title>T</title><meta charset=utf-8></head>\
         <body><!-- note --><p>x</p></body></html>",
    );
    assert_eq!(
        dump(&p.tree),
        r#"html(lang=en){head{title{"T"} meta(charset=utf-8)} body{p{"x"}}}"#
    );
    assert_eq!(count(&p, "doctype"), 1);
    assert_eq!(count(&p, "comment"), 1);
}

#[test]
fn tag_and_attribute_names_are_folded_to_lower_case_and_values_are_not() {
    assert_eq!(
        shape(r#"<DIV CLASS="A B" Data-Name='X'></DIV>"#),
        "div(class=A B data-name=X)"
    );
}

#[test]
fn attributes_can_be_quoted_unquoted_or_bare_and_the_first_of_a_name_wins() {
    assert_eq!(
        shape(r#"<input type=text disabled value = "a > b" value=two data-x='1"2'>"#),
        r#"input(type=text disabled= value=a > b data-x=1"2)"#
    );
    assert_eq!(
        shape("<div\n\tid=a\n\tclass=b\n>x</div>"),
        r#"div(id=a class=b){"x"}"#
    );
}

#[test]
fn a_slash_before_the_end_of_a_tag_closes_nothing_but_a_void_element() {
    // As a browser reads HTML: `<div/>` is an open div, `<br/>` is a br.
    assert_eq!(shape("<div/>a<br/>b"), r#"div{"a" br "b"}"#);
}

#[test]
fn event_attributes_are_not_kept_and_are_counted_once_by_kind() {
    let p = run(r#"<a onclick="x()" ONMOUSEOVER=y href=u onerror=z>t</a>"#);
    assert_eq!(dump(&p.tree), r#"a(href=u){"t"}"#);
    assert_eq!(count(&p, "event_attribute"), 3);
}

#[test]
fn text_keeps_its_spaces_and_line_ends_become_line_feeds() {
    assert_eq!(shape("<p>  a \r\n\t b\rc  </p>"), r#"p{"  a \n\t b\nc  "}"#);
    assert_eq!(shape("<div> <p>x</p> </div>"), r#"div{" " p{"x"} " "}"#);
}

#[test]
fn text_around_a_comment_is_one_text() {
    assert_eq!(shape("<p>a<!-- x -->b<!---->c</p>"), r#"p{"abc"}"#);
}

#[test]
fn character_references_are_read_in_text_and_in_attribute_values() {
    assert_eq!(
        shape(
            "<p title=\"&quot;a&quot; &amp; b\">&lt;b&gt; &amp;amp; &nbsp;&copy;&#65;&#x42;&#X1F600;</p>"
        ),
        "p(title=\"a\" & b){\"<b> &amp; \u{a0}\u{a9}AB\u{1f600}\"}"
    );
}

#[test]
fn a_reference_that_is_not_in_the_subset_stays_as_written() {
    assert_eq!(
        shape("<p>&bogus; &amp &lt &#; &#x; &#12 &</p>"),
        r#"p{"&bogus; &amp &lt &#; &#x; &#12 &"}"#
    );
}

#[test]
fn a_numeric_reference_to_nothing_valid_is_the_replacement_character() {
    assert_eq!(
        shape("<p>&#0;&#xD800;&#x110000;&#1114112;</p>"),
        "p{\"\u{fffd}\u{fffd}\u{fffd}\u{fffd}\"}"
    );
}

#[test]
fn void_elements_have_no_children_and_their_end_tag_is_dropped() {
    let p = run("<p>a<br>b<img src=x alt=y></p><hr><input></input>");
    assert_eq!(dump(&p.tree), r#"p{"a" br "b" img(src=x alt=y)} hr input"#);
    assert_eq!(count(&p, "stray_end_tag"), 1);
}

#[test]
fn a_paragraph_ends_where_a_block_starts_or_another_paragraph_does() {
    assert_eq!(shape("<p>a<p>b"), r#"p{"a"} p{"b"}"#);
    assert_eq!(shape("<p>a<div>b</div>"), r#"p{"a"} div{"b"}"#);
    assert_eq!(
        shape("<p>a<span>b<ul><li>c</ul>"),
        r#"p{"a" span{"b"}} ul{li{"c"}}"#
    );
    // A button is between the paragraph and the block: the paragraph stays open.
    assert_eq!(
        shape("<p>a<button>b<div>c</div></button>d"),
        r#"p{"a" button{"b" div{"c"}} "d"}"#
    );
}

#[test]
fn list_items_definitions_and_headings_end_the_one_before() {
    assert_eq!(
        shape("<ul><li>a<li>b<li>c</ul>"),
        r#"ul{li{"a"} li{"b"} li{"c"}}"#
    );
    assert_eq!(
        shape("<ul><li>a<ol><li>b<li>c</ol><li>d</ul>"),
        r#"ul{li{"a" ol{li{"b"} li{"c"}}} li{"d"}}"#
    );
    assert_eq!(
        shape("<dl><dt>a<dd>b<dt>c</dl>"),
        r#"dl{dt{"a"} dd{"b"} dt{"c"}}"#
    );
    assert_eq!(shape("<h1>a<h2>b"), r#"h1{"a"} h2{"b"}"#);
}

#[test]
fn table_rows_and_cells_end_the_ones_before() {
    assert_eq!(
        shape("<table><thead><tr><th>a<th>b<tbody><tr><td>c<td>d<tr><td>e</table>"),
        r#"table{thead{tr{th{"a"} th{"b"}}} tbody{tr{td{"c"} td{"d"}} tr{td{"e"}}}}"#
    );
}

#[test]
fn options_end_the_option_before_inside_their_select() {
    assert_eq!(
        shape("<select><option>a<option>b<optgroup><option>c</select>"),
        r#"select{option{"a"} option{"b"} optgroup{option{"c"}}}"#
    );
}

#[test]
fn an_element_the_reader_does_not_know_is_kept_with_its_content() {
    assert_eq!(
        shape(r#"<my-widget a=1><x:y>t</x:y></my-widget>"#),
        r#"my-widget(a=1){x:y{"t"}}"#
    );
}

#[test]
fn an_end_tag_closes_the_nearest_open_element_of_its_name_and_what_is_inside_it() {
    let p = run("<b><i>x</b>y</i>z");
    assert_eq!(dump(&p.tree), r#"b{i{"x"}} "yz""#);
    assert_eq!(count(&p, "misnested_end_tag"), 1);
    assert_eq!(count(&p, "stray_end_tag"), 1);
}

#[test]
fn an_end_tag_with_nothing_open_to_close_is_dropped() {
    let p = run("<i></i>a</div>b</p></x>");
    assert_eq!(dump(&p.tree), r#"i "ab""#);
    assert_eq!(count(&p, "stray_end_tag"), 3);
}

#[test]
fn what_is_still_open_at_the_end_is_closed_there() {
    let p = run("<div><p>a<span>b");
    assert_eq!(dump(&p.tree), r#"div{p{"a" span{"b"}}}"#);
    assert_eq!(count(&p, "unclosed_at_end"), 3);
}

#[test]
fn style_is_kept_as_one_raw_text_for_the_css_reader_and_script_is_dropped() {
    let css = "a > b { content: \"</p>&amp;\" } p::before { x: 1 }";
    let p = run(&format!(
        "<style>{css}</style><script>var a = '<div>';</script><p>x</p>"
    ));
    assert_eq!(dump(&p.tree), format!("style{{{}}} p{{\"x\"}}", q(css)));
    assert_eq!(p.tree.style_texts().collect::<Vec<_>>(), vec![css]);
    assert_eq!(count(&p, "script"), 1);
}

#[test]
fn a_raw_text_element_ends_at_its_own_end_tag_in_any_case() {
    assert_eq!(shape("<STYLE>a</Style >b"), r#"style{"a"} "b""#);
    assert_eq!(
        shape("<style>a</styled>b</style>c"),
        r#"style{"a</styled>b"} "c""#
    );
}

#[test]
fn title_and_textarea_hold_text_with_references_read_and_no_markup() {
    assert_eq!(shape("<title>a &amp; <b>c</title>"), r#"title{"a & <b>c"}"#);
    assert_eq!(
        shape("<textarea>\n<p>&lt;</textarea><p>"),
        r#"textarea{"\n<p><"} p"#
    );
}

#[test]
fn a_less_than_sign_that_starts_no_tag_is_text() {
    assert_eq!(shape("<p>a < b <3 <- <</p>"), r#"p{"a < b <3 <- <"}"#);
    assert_eq!(shape("<p>< div>x"), r#"p{"< div>x"}"#);
}

#[test]
fn a_processing_instruction_or_cdata_is_dropped_like_a_comment() {
    let p = run("<?xml version=\"1.0\"?><p>a<![CDATA[ x > ]]>b</p>");
    assert_eq!(dump(&p.tree), r#"p{"a ]]>b"}"#);
    assert_eq!(count(&p, "comment"), 2);
}

#[test]
fn every_element_knows_its_parent_and_where_its_start_tag_was() {
    let html = "<div><p>a</p>  <span></span></div>";
    let p = run(html);
    let div = match p.tree.root().children[0] {
        Child::Element(e) => e,
        _ => panic!(),
    };
    assert_eq!(p.tree.element(div).at, 0);
    let kids: Vec<_> = p
        .tree
        .element(div)
        .children
        .iter()
        .filter_map(|c| match c {
            Child::Element(e) => Some(*e),
            _ => None,
        })
        .collect();
    assert_eq!(kids.len(), 2);
    assert_eq!(p.tree.element(kids[0]).at, html.find("<p>").unwrap());
    assert_eq!(p.tree.element(kids[1]).at, html.find("<span>").unwrap());
    for k in kids {
        assert_eq!(p.tree.element(k).parent, div);
    }
    assert_eq!(p.tree.element(div).parent, p.tree.root_id());
    assert_eq!(p.tree.len(), 3);
}

#[test]
fn an_attribute_is_found_by_name() {
    let p = run("<a href=x data-name=\"N\">t</a>");
    let Child::Element(a) = p.tree.root().children[0] else {
        panic!()
    };
    assert_eq!(p.tree.element(a).attr("data-name"), Some("N"));
    assert_eq!(p.tree.element(a).attr("href"), Some("x"));
    assert_eq!(p.tree.element(a).attr("class"), None);
}

#[test]
fn the_same_text_gives_the_same_tree_and_report() {
    let html = "<div class=a><p>x<p>y &amp; z<!-- c --></div></b><script>1</script>";
    let a = run(html);
    let b = run(html);
    assert_eq!(a.tree, b.tree);
    assert_eq!(a.report, b.report);
}

// The tests from here on: regression tests of fixes are the heading, list item and definition ending a paragraph, the
// first of an attribute name deciding, the depth cut of a raw text element and the attribute cap counting cut names; the
// others (NUL, ASCII folding, `</scriptx>`, unfinished input, Empty, TooLarge, TooMany) are coverage and pass on the old code.

#[test]
fn a_heading_a_list_item_or_a_definition_also_ends_a_paragraph() {
    for (html, want) in [
        ("<p>a<h2>b</h2>", r#"p{"a"} h2{"b"}"#),
        ("<p>a<li>b", r#"p{"a"} li{"b"}"#),
        ("<dl><dt>a<p>b<dd>c</dl>", r#"dl{dt{"a" p{"b"}} dd{"c"}}"#),
        ("<ul><li><p>a<li>b</ul>", r#"ul{li{p{"a"}} li{"b"}}"#),
        // A button between them keeps the paragraph open, as for a block.
        (
            "<p><button>x<h2>y</h2></button>",
            r#"p{button{"x" h2{"y"}}}"#,
        ),
    ] {
        assert_eq!(shape(html), want, "{html}");
    }
}

#[test]
fn the_first_of_an_attribute_name_decides_and_every_repeat_is_counted() {
    let p = run("<a b=1 B=2 b=3 c=4>");
    assert_eq!(dump(&p.tree), "a(b=1 c=4)");
    assert_eq!(count(&p, "duplicate_attribute"), 2);
    // A first one that is cut for its length still decides.
    let p = run(&format!("<a b=\"{}\" b=ok c=d>", "x".repeat(1_001)));
    assert_eq!(dump(&p.tree), "a(c=d)");
    assert_eq!(count(&p, "attribute_too_long"), 1);
    assert_eq!(count(&p, "duplicate_attribute"), 1);
}

#[test]
fn a_text_with_a_nul_keeps_it_as_the_replacement_character() {
    assert_eq!(shape("<p>a\0b</p>"), "p{\"a\u{fffd}b\"}");
}

#[test]
fn attribute_names_are_folded_for_ascii_only() {
    assert_eq!(shape("<p \u{d1}=X \u{c4}B=y>"), "p(\u{d1}=X \u{c4}b=y)");
}

#[test]
fn a_script_ends_at_its_own_end_tag_only() {
    let p = run("<i>x</i><script>a</scriptx>b</script>c");
    assert_eq!(dump(&p.tree), r#"i{"x"} "c""#);
    assert_eq!(count(&p, "script"), 1);
}

#[test]
fn an_unfinished_tag_or_comment_ends_the_read_and_keeps_what_came_before() {
    assert_eq!(shape(r#"<p>a<b>c</b><i class="x"#), r#"p{"a" b{"c"}}"#);
    assert_eq!(shape("<p>a<!-- b"), r#"p{"a"}"#);
}

#[test]
fn a_text_with_no_element_is_refused_as_empty() {
    for html in ["", "plain text", "<!-- a --><script>x</script>"] {
        assert_eq!(
            refused(html, &Limits::default()),
            ParseError::Empty,
            "{html:?}"
        );
    }
}

#[test]
fn the_input_bound_is_exact_and_refuses_before_reading() {
    let l = Limits {
        max_input_bytes: 5,
        ..roomy()
    };
    assert!(parse("<p>ab", &l).is_ok());
    assert_eq!(
        refused("<p>abc", &l),
        ParseError::TooLarge { bytes: 6, max: 5 }
    );
}

#[test]
fn the_element_bound_is_exact_and_names_where_the_first_one_past_it_starts() {
    let l = Limits {
        max_elements: 3,
        ..roomy()
    };
    assert!(parse("<b></b><b></b><b></b>", &l).is_ok());
    assert_eq!(
        refused("<b></b><b></b><b></b><b></b>", &l),
        ParseError::TooMany { max: 3, at: 21 }
    );
}

#[test]
fn an_element_past_the_depth_is_flattened_and_a_raw_text_one_loses_its_content() {
    let l = Limits {
        max_depth: 1,
        ..roomy()
    };
    let p = parse(
        "<div><style>a{color:red}</style><textarea>hi</textarea><p>x</p></div>",
        &l,
    )
    .unwrap();
    assert_eq!(dump(&p.tree), r#"div{"x"}"#);
    assert_eq!(p.tree.style_texts().count(), 0);
    assert_eq!(count(&p, "depth_cut"), 3);
}

#[test]
fn a_name_cut_for_its_length_counts_toward_the_attribute_cap() {
    let l = Limits {
        max_attributes: 3,
        max_value_bytes: 3,
        ..roomy()
    };
    let p = parse("<a x=toolong a=1 b=2 c=3 d=4>", &l).unwrap();
    // `x` is cut and counts as one of the three names looked at, so two are kept and the next two are left out.
    assert_eq!(dump(&p.tree), "a(a=1 b=2)");
    assert_eq!(count(&p, "attribute_too_long"), 1);
    assert_eq!(count(&p, "attribute_dropped"), 2);
}

#[test]
fn a_heading_ends_a_heading_only_when_it_is_the_open_element_on_top() {
    // `top_only`: the span is on top, so the second heading does not close the first through it.
    assert_eq!(shape("<h1><span><h2>x"), r#"h1{span{h2{"x"}}}"#);
    // A paragraph is looked for further down.
    assert_eq!(shape("<p><span><h2>x"), r#"p{span} h2{"x"}"#);
}

#[test]
fn an_attribute_of_on_and_one_letter_is_an_event_attribute_and_on_alone_is_not() {
    let p = run("<a onx=1 ony=2 on=3 o=4>");
    assert_eq!(dump(&p.tree), "a(on=3 o=4)");
    assert_eq!(count(&p, "event_attribute"), 2);
}
