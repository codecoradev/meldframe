//! Wrappers that go, placeholders and controls (`docs/import-html.md` section 3.2).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::{Bind, NodeKind};

#[test]
fn a_plain_wrapper_goes_and_its_content_takes_its_place() {
    let p = planned(
        "<div data-name=Top><div><div><p data-name=Deep>x</p></div></div><p data-name=Other>y</p></div>",
        RESET,
    );
    let list = nodes(&p);
    assert_eq!(names(&list), ["Top", "Deep", "Other"]);
    assert_eq!(list[1].parent_id.as_deref(), Some("imp-0"));
    assert_eq!(count(&p, "wrapper_collapsed"), 2);
    assert_eq!(
        p.report
            .entries
            .iter()
            .filter(|e| e.code == "wrapper_collapsed")
            .count(),
        2
    );
}

#[test]
fn a_wrapper_with_a_name_a_padding_or_two_children_stays() {
    let list = map(
        "<div data-name=Top><div data-name=Named><p>a</p></div><div style='padding:3px' data-name=Pad><p>b</p></div>\
         <div data-name=Two><p>c</p><p>d</p></div></div>",
    );
    assert_eq!(names(&list)[..4], ["Top", "Named", "p", "Pad"]);
    assert!(names(&list).contains(&"Two"));
}

#[test]
fn only_a_block_wrapper_can_go() {
    let list = map("<div data-name=Top><div style='display:inline-block'><p>a</p></div></div>");
    assert_eq!(list.len(), 3, "{:?}", names(&list));
}

#[test]
fn an_image_is_a_rect_with_its_src_listed_and_nothing_fetched() {
    let p = planned(
        "<div data-name=Card><img src='https://x.test/a.png' alt=a style='width:10px;height:10px'></div>",
        RESET,
    );
    let list = nodes(&p);
    assert_eq!(list[1].kind, NodeKind::Rect);
    assert_eq!((p.assets.len(), count(&p, "image_src")), (1, 1));
    assert_eq!(
        (p.assets[0].node.as_str(), p.assets[0].src.as_str()),
        ("imp-1", "https://x.test/a.png")
    );
    assert_eq!(list[1].fill, Bind::Lit("none".into()));
}

#[test]
fn what_the_model_cannot_hold_is_a_placeholder_and_listed_as_refused() {
    let p = planned(
        "<div data-name=Box><svg><circle/></svg><video src=v></video><canvas></canvas></div>",
        RESET,
    );
    assert_eq!(p.nodes.len(), 4);
    for n in &p.nodes[1..] {
        assert_eq!(n.kind, NodeKind::Rect);
        assert_eq!(n.unsupported[0].code, "refused_content", "{n:?}");
    }
}

#[test]
fn a_button_and_a_field_are_frames_with_the_text_they_show() {
    let list = map(
        "<div data-name=Form><button data-name=Go>Go <b>now</b></button>\
         <input data-name=Name value='Ada' placeholder='Your name'>\
         <input data-name=Ph placeholder='Email'><textarea data-name=Area>Long text</textarea>\
         <select data-name=Pick><option>a</option></select></div>",
    );
    let text_of = |frame: &str| {
        let f = named(&list, frame);
        assert_eq!(f.kind, NodeKind::Frame, "{frame}");
        list.iter()
            .filter(|n| n.parent_id.as_deref() == Some(f.id.as_str()))
            .map(|n| n.text.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(text_of("Go"), ["Go now"]);
    assert_eq!(text_of("Name"), ["Ada"]);
    assert_eq!(text_of("Ph"), ["Email"]);
    assert_eq!(text_of("Area"), ["Long text"]);
    assert!(text_of("Pick").is_empty());
}

#[test]
fn a_control_lists_that_the_browser_draws_it_and_a_frame_was_made() {
    let p = planned(
        "<div data-name=F><button data-name=B>Go</button></div>",
        RESET,
    );
    let b = &p.nodes[1];
    assert_eq!(b.name, "B");
    assert_eq!(
        (
            b.approximated[0].property.as_str(),
            b.approximated[0].from.as_str()
        ),
        ("element", "button")
    );
}

#[test]
fn controls_with_no_equivalent_make_no_node_and_are_refused() {
    let p = planned(
        "<div data-name=Form><input type=checkbox><input type=radio><input type=range><input type=file><input type=color>\
         <p>x</p></div>",
        RESET,
    );
    assert_eq!(names(&nodes(&p)), ["Form", "p"]);
    let n = p
        .report
        .entries
        .iter()
        .filter(|e| e.code == "element_skipped")
        .count();
    assert_eq!(n, 5);
}
