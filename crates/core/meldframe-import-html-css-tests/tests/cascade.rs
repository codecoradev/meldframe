//! The cascade: which declarations apply to which element, and in what order. Selectors against the tree, the
//! order of origins, specificity and source order, `!important`, the `style` attribute, the generated boxes, the
//! `html` and `body` a fragment does not have, and the budget.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::{EntryKind, Limits};

#[test]
fn declarations_come_lowest_first_by_specificity_then_source_order() {
    let html = r#"<p id=a class="b c">x</p>"#;
    assert_eq!(
        main(html, "#a{x:3} .b{x:2} p{x:1} p{y:1} .c{y:2} *{z:0}", "p"),
        ["z:0", "x:1", "y:1", "x:2", "y:2", "x:3"]
    );
    // Equal specificity: the later rule is later in the list; inside a rule, in the order written.
    assert_eq!(
        main("<p>", "p{a:1;b:2} p{a:3;b:4}", "p"),
        ["a:1", "b:2", "a:3", "b:4"]
    );
}

#[test]
fn important_beats_normal_and_inline_beats_author_of_the_same_kind() {
    let html = r#"<p id=a style="x:inline; y:inline!important; z:inline">t</p>"#;
    assert_eq!(
        main(
            html,
            "#a{x:author!important; y:author!important; w:author} p{z:author}",
            "p"
        ),
        [
            "z:author",
            "w:author",
            "x:inline@",
            "z:inline@",
            "x:author!",
            "y:author!",
            "y:inline!@",
        ]
    );
}

#[test]
fn a_list_of_selectors_counts_once_with_the_specificity_of_the_best_one_that_matches() {
    let html = r#"<div><p id=a class=b>t</p></div>"#;
    // `p, #a, .nope` matches by `p` and `#a`: one entry, as specific as `#a`.
    assert_eq!(
        main(html, "p, #a, .nope{x:list} .b{x:class} div p{x:desc}", "p"),
        ["x:desc", "x:class", "x:list"]
    );
}

#[test]
fn descendant_and_child_combinators_look_up_the_tree() {
    let html =
        "<div id=d><section><p class=x>a</p></section><p class=y>b</p></div><p class=z>c</p>";
    let (p, c, _) = cascaded(
        html,
        "div p{a:1} div>p{b:1} section>p{c:1} div>section p{d:1} body p{e:1} html p{f:1} \
         html>body>p{g:1} body>div>p{h:1}",
    );
    let at = |class: &str| {
        let id = p
            .tree
            .ids()
            .find(|i| p.tree.element(*i).attr("class") == Some(class))
            .unwrap();
        list(&c.elements[id.index()].main)
    };
    // Every top-level element has a body and an html above it, though the text has neither. Two types are
    // less specific than three.
    assert_eq!(at("x"), ["a:1", "c:1", "e:1", "f:1", "d:1"]);
    assert_eq!(at("y"), ["a:1", "b:1", "e:1", "f:1", "h:1"]);
    assert_eq!(at("z"), ["e:1", "f:1", "g:1"]);
}

#[test]
fn a_descendant_combinator_tries_the_farther_ancestors_when_a_child_combinator_needs_them() {
    // `a > b c` for the `c` under `a > b > b`: the nearer `b` has the wrong parent, the farther has `a`.
    assert_eq!(
        main("<a><b><b><c></c></b></b></a>", "a > b c{x:1}", "c"),
        ["x:1"]
    );
    assert!(main("<a><x><b><c></c></b></x></a>", "a > b c{x:1}", "c").is_empty());
    // A chain of descendants stops at the nearest match: no failure to find is made worse by depth.
    let deep = format!("{}<p>x</p>{}", "<div>".repeat(60), "</div>".repeat(60));
    let rule = format!("a {} p{{x:1}}", "div ".repeat(14));
    let (_, c, r) = cascaded(&deep, &rule);
    assert!(c.complete);
    assert!(r.work < 40_000, "{}", r.work);
}

#[test]
fn attribute_selectors_read_what_the_tree_holds() {
    let html = r#"<p data-node="n 1" data-name=x hidden class="a  b">t</p>"#;
    let css = r#"[hidden]{a:1} [data-node="n\20 1"]{b:1} [data-name=x]{c:1} [data-name=y]{d:1} [class="a  b"]{e:1} [HIDDEN]{f:1} .b.a{g:1}"#;
    // All of them are as specific as a class, so they come in the order written, and `.b.a` is more specific.
    assert_eq!(
        main(html, css, "p"),
        ["a:1", "b:1", "c:1", "e:1", "f:1", "g:1"]
    );
}

#[test]
fn a_repeated_class_does_not_repeat_a_rule() {
    assert_eq!(main(r#"<p class="a a a">"#, ".a{x:1}", "p"), ["x:1"]);
}

#[test]
fn the_generated_boxes_have_their_own_lists_and_the_style_attribute_never_reaches_them() {
    let html = r#"<div data-node="n1" style="color:red">t</div>"#;
    let css = "[data-node=\"n1\"]{position:relative} [data-node=\"n1\"]::after{content:\"\";inset:0} \
               div:before{x:1} div::after, p{y:2}";
    let (p, c, _) = cascaded(html, css);
    let m = &c.elements[el(&p, "div").index()];
    assert_eq!(list(&m.main), ["position:relative", "color:red@"]);
    assert_eq!(list(&m.before), ["x:1"]);
    assert_eq!(list(&m.after), ["y:2", "content:\"\"", "inset:0"]);
}

#[test]
fn the_html_and_body_of_a_document_are_elements_and_of_a_fragment_are_virtual() {
    let css = ":root{a:1} html{b:1} body{c:1} :root>body{d:1} body>p{e:1} *{f:1}";
    // A document: the elements themselves; nothing is made up.
    let (p, c, _) = cascaded("<html><body><p>x</p></body></html>", css);
    assert!(c.html.is_none() && c.body.is_none());
    assert_eq!(
        list(&c.elements[el(&p, "html").index()].main),
        ["f:1", "b:1", "a:1"]
    );
    assert_eq!(
        list(&c.elements[el(&p, "body").index()].main),
        ["f:1", "c:1", "d:1"]
    );
    assert_eq!(list(&c.elements[el(&p, "p").index()].main), ["f:1", "e:1"]);
    // A fragment: both are virtual and the fragment is inside them.
    let (p, c, _) = cascaded("<p>x</p>", css);
    assert_eq!(list(&c.html.unwrap().main), ["f:1", "b:1", "a:1"]);
    assert_eq!(list(&c.body.unwrap().main), ["f:1", "c:1", "d:1"]);
    assert_eq!(list(&c.elements[el(&p, "p").index()].main), ["f:1", "e:1"]);
    // A text with a `body` and no `html` has the virtual `html` only.
    let (p, c, _) = cascaded("<body><p>x</p></body>", css);
    assert!(c.body.is_none());
    assert_eq!(list(&c.html.unwrap().main), ["f:1", "b:1", "a:1"]);
    assert_eq!(
        list(&c.elements[el(&p, "body").index()].main),
        ["f:1", "c:1", "d:1"]
    );
}

#[test]
fn the_root_has_nothing_and_every_element_has_a_place() {
    let (p, c, _) = cascaded("<div><p></p></div><span></span>", "*{a:1}");
    assert_eq!(c.elements.len(), p.tree.len() + 1);
    assert!(c.elements[0].main.is_empty());
    assert!(c.elements[1..].iter().all(|m| m.main.len() == 1));
}

#[test]
fn a_cascade_over_its_budget_stops_at_an_element_and_says_so() {
    let html = "<p>x</p>".repeat(100);
    let limits = Limits {
        max_work: 250,
        ..Limits::default()
    };
    let (p, c, r) = cascaded_with(&html, "p{x:1}", &limits);
    assert!(!c.complete);
    let done = c.elements.iter().filter(|m| !m.main.is_empty()).count();
    assert!(0 < done && done < 100, "{done}");
    // The elements before the cut have all they should, and the ones from it on have nothing.
    assert!(c.elements[1..=done].iter().all(|m| m.main.len() == 1));
    assert!(c.elements[done + 1..].iter().all(|m| m.main.is_empty()));
    assert_eq!(count(&r, "cascade_work_limit"), 1);
    assert!(
        r.entries.iter().any(|e| e.kind == EntryKind::Cut
            && e.at == p.tree.element(p.tree.ids().nth(done).unwrap()).at)
    );
    assert!(r.work <= 250 + 40);
}

#[test]
fn thousands_of_universal_rules_over_thousands_of_elements_stay_within_the_budget() {
    let html = "<p>x</p>".repeat(3_000);
    let css = "*{a:1}".repeat(3_000);
    let (_, c, r) = cascaded(&html, &css);
    assert!(!c.complete);
    let limit = Limits::default().max_work;
    let steps = r.work - sheet(&css).1.work;
    assert!(steps <= limit + 3_000 * 2, "{steps} steps");
}

#[test]
fn the_work_grows_with_the_elements_and_the_selectors_that_could_pick_them() {
    // 5,000 rules for classes no element has cost nothing per element; 100 rules for the one they have cost 100.
    let html = "<p class=y>x</p>".repeat(1_000);
    let cascade_work = |css: &str| {
        let (_, c, r) = cascaded(&html, css);
        assert!(c.complete);
        r.work - sheet(css).1.work
    };
    let none: String = (0..5_000).map(|i| format!(".n{i}{{x:1}}")).collect();
    assert!(cascade_work(&none) < 20_000);
    let w = cascade_work(&".y{x:1}".repeat(100));
    assert!(w > 50 * 1_000 && w < 5 * 100 * 1_000, "{w}");
}

#[test]
fn a_style_attribute_is_read_with_the_limits_and_reports_what_it_refuses() {
    let (p, c, r) = cascaded(r#"<p style="a:1; bad; b:2">x</p>"#, "");
    assert_eq!(
        list(&c.elements[el(&p, "p").index()].main),
        ["a:1@", "b:2@"]
    );
    assert_eq!(names(&r, "invalid_declaration"), ["bad"]);
}

/// The steps the cascade took, apart from what reading the sheet cost.
fn steps(html: &str, css: &str) -> usize {
    let (_, c, r) = cascaded(html, css);
    assert!(c.complete);
    r.work - sheet(css).1.work
}

#[test]
fn each_charge_of_the_budget_is_pinned_alone() {
    // A selector looked at: the html and body of a fragment and the paragraph each look at three, test three
    // (a compound with no part costs 1) and make three declarations candidates: 3 * (3 + 3 + 3).
    assert_eq!(steps("<p>", "*{x:1} *{x:1} *{x:1}"), 27);
    // A compound costs 1 and one for each part: `p.a.b#c[d]` is 6, found through its id (1), with 1 declaration.
    assert_eq!(steps("<p class=\"a b\" id=c d>", "p.a.b#c[d]{x:1}"), 8);
    // A declaration of a rule that picks the element costs 1: 20,000 of them, for the two virtual elements and three
    // paragraphs, each with its one selector looked at and one compound.
    let wide = format!("*{{{}}}", "x:1;".repeat(20_000));
    assert_eq!(
        steps("<p>x</p><p>x</p><p>x</p>", &wide),
        5 * (1 + 1 + 20_000)
    );
    // A declaration of a style attribute costs 1, and reading its 11 bytes is counted by the reader (11 steps).
    assert_eq!(steps("<p style=\"a:1;b:2;c:3\">", ""), 3 + 11);
    // A rule that no element has (it is not looked at) costs nothing.
    assert_eq!(steps("<p>", ".nope{x:1} #nope{x:1}"), 0);
}

#[test]
fn one_rule_with_many_declarations_over_many_elements_stops_at_the_budget_and_one_element_over() {
    let wide = format!("*{{{}}}", "x:1;".repeat(20_000));
    let l = Limits {
        max_work: 50_000,
        ..Limits::default()
    };
    let (p, c, r) = cascaded_with(&"<p>x</p>".repeat(1_000), &wide, &l);
    assert!(!c.complete);
    // The two virtual elements fit (40,004 steps); the first paragraph passes 50,000 and is not kept.
    assert_eq!(c.html.as_ref().map(|m| m.main.len()), Some(20_000));
    assert_eq!(c.body.as_ref().map(|m| m.main.len()), Some(20_000));
    assert!(c.elements.iter().all(|m| m.main.is_empty()));
    assert_eq!(count(&r, "cascade_work_limit"), 1);
    let first = p.tree.ids().next().unwrap();
    assert_eq!(r.entries[0].at, p.tree.element(first).at);
    let steps = r.work - sheet(&wide).1.work;
    assert!(steps <= l.max_work + 20_002, "{steps}");
}

#[test]
fn a_chain_of_descendants_over_a_deep_tree_costs_a_few_hundred_steps_and_a_child_combinator_is_cut()
{
    let deep = format!("{}<p>x</p>{}", "<div>".repeat(60), "</div>".repeat(60));
    // Sixteen compounds are the most a selector has: `a`, fourteen `div` and `p`.
    let rule = format!("a {} p{{x:1}}", "div ".repeat(14));
    assert!(steps(&deep, &rule) < 400);
    let found = format!("{} p{{x:1}}", "div ".repeat(15));
    assert_eq!(main(&deep, &found, "p"), ["x:1"]);
    assert!(steps(&deep, &found) < 400);
    // With a child combinator to the left every ancestor is tried again for each step: the budget ends it.
    let rule = format!("x > {} p{{x:1}}", "div ".repeat(14));
    let (_, c, r) = cascaded(&deep, &rule);
    assert!(!c.complete);
    let steps = r.work - sheet(&rule).1.work;
    assert!(steps <= Limits::default().max_work + 200, "{steps}");
}

#[test]
fn the_report_of_a_style_attribute_carries_the_byte_of_its_element() {
    let (p, _, r) = cascaded("<div>a</div>\n<p style=\"bad; x:1\">b</p>", "");
    let at = p.tree.element(el(&p, "p")).at;
    assert!(at > 10);
    assert_eq!(names(&r, "invalid_declaration"), ["bad"]);
    assert_eq!(r.entries[0].at, at);
}
