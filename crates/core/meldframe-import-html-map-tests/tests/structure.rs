//! Which elements make nodes, of which kind, in what order and with what names (`docs/import-html.md` section 3.2).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::ParseError;
use html_import::map::{Options, plan};
use meldframe_protocol::{Command, NodeKind};

#[test]
fn the_commands_add_the_nodes_in_the_order_of_the_text_a_parent_before_its_children() {
    let p = planned(
        "<section data-name=Hero><h1>Title</h1><div><p>one</p><p>two</p></div></section>",
        RESET,
    );
    let list = nodes(&p);
    let ids: Vec<&str> = list.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        ["imp-0", "imp-1", "imp-2", "imp-3", "imp-4"],
        "{ids:?}"
    );
    let parents: Vec<Option<&str>> = list.iter().map(|n| n.parent_id.as_deref()).collect();
    assert_eq!(
        parents,
        [
            None,
            Some("imp-0"),
            Some("imp-0"),
            Some("imp-2"),
            Some("imp-2")
        ]
    );
    let texts: Vec<&str> = list.iter().map(|n| n.text.as_str()).collect();
    assert_eq!(texts, ["", "Title", "", "one", "two"]);
    assert!(list.iter().all(|n| n.pos.is_empty()));
    assert!(
        p.commands
            .iter()
            .all(|c| matches!(c, Command::AddNode { index: None, .. }))
    );
    assert_eq!(p.nodes.len(), list.len());
    assert_eq!(p.nodes[1].id, "imp-1");
    assert!(!p.exceeds_batch());
}

#[test]
fn elements_become_a_frame_a_text_or_a_rect() {
    let list = map("<div data-name=Card><p data-name=Label>Hi</p>\
         <div data-name=Swatch style='width:40px;height:20px'></div>\
         <div data-name=Box><span>x</span><div>y</div></div></div>");
    assert_eq!(named(&list, "Card").kind, NodeKind::Frame);
    assert_eq!(named(&list, "Label").kind, NodeKind::Text);
    assert_eq!(named(&list, "Label").text, "Hi");
    assert_eq!(named(&list, "Swatch").kind, NodeKind::Rect);
    assert_eq!(named(&list, "Box").kind, NodeKind::Frame);
}

#[test]
fn a_box_with_padding_is_a_frame_with_a_text_not_a_text() {
    let list = map("<div data-name=Wrap><p data-name=Pad style='padding:4px'>x</p></div>");
    let pad = named(&list, "Pad");
    assert_eq!(pad.kind, NodeKind::Frame);
    let kids: Vec<_> = list
        .iter()
        .filter(|n| n.parent_id.as_deref() == Some(pad.id.as_str()))
        .collect();
    assert_eq!(kids.len(), 1);
    assert_eq!((kids[0].kind, kids[0].text.as_str()), (NodeKind::Text, "x"));
}

#[test]
fn a_childless_box_with_a_fixed_height_and_a_filling_width_is_a_rect() {
    let list =
        map("<div data-name=Root><div data-name=Bar style='height:5px'></div><p>x</p></div>");
    let bar = named(&list, "Bar");
    assert_eq!(bar.kind, NodeKind::Rect);
    assert_eq!(
        (bar.sw, bar.sh),
        (
            meldframe_protocol::SizeMode::Fill,
            meldframe_protocol::SizeMode::Fixed
        )
    );
}

#[test]
fn a_childless_flex_container_or_padded_box_is_a_frame_not_a_rect() {
    let list = map(
        "<div data-name=Root><div data-name=Flex style='display:flex;width:10px;height:10px'></div>\
         <div data-name=Padded style='width:10px;height:10px;padding:2px'></div>\
         <div data-name=NoHeight style='width:10px'></div></div>",
    );
    for n in ["Flex", "Padded", "NoHeight"] {
        assert_eq!(named(&list, n).kind, NodeKind::Frame, "{n}");
    }
}

#[test]
fn a_node_is_named_by_data_name_then_id_then_first_class_then_tag() {
    let list = map(
        "<div data-name=Outer><p data-name='Given' id=i class='c d'>a</p><p id=ident class='c d'>b</p>\
         <p class='  first second'>c</p><p>d</p></div>",
    );
    assert_eq!(names(&list), ["Outer", "Given", "ident", "first", "p"]);
}

#[test]
fn a_name_loses_hidden_characters_and_is_cut_to_256() {
    let long = "n".repeat(300);
    let html =
        format!("<div data-name=\"Ho\u{200b}me\u{202e}\"><p data-name=\"{long}\">x</p></div>");
    let list = map(&html);
    assert_eq!(list[0].name, "Home");
    assert_eq!(list[1].name.chars().count(), 256);
}

#[test]
fn the_root_is_named_by_the_option_then_the_title_then_imported_html() {
    let one = "<title>Page title</title><div><p>a</p></div><div><p>b</p></div>";
    let titled = nodes(&planned(one, RESET));
    assert_eq!(titled[0].name, "Page title");
    let o = Options {
        name: Some("Given".into()),
        ..Options::default()
    };
    assert_eq!(nodes(&plan(one, RESET, &o).unwrap())[0].name, "Given");
    let none = nodes(&planned("<div><p>a</p></div><div><p>b</p></div>", RESET));
    assert_eq!(none[0].name, "Imported HTML");
    assert_eq!(none[0].kind, NodeKind::Frame);
    assert_eq!(none.len(), 3, "the plain wrappers go");
}

#[test]
fn a_single_top_level_frame_is_the_root_itself_and_keeps_its_own_name() {
    let list = map("<title>Page</title><div data-name=Hero><p>a</p><p>b</p></div>");
    assert_eq!(list[0].name, "Hero");
    assert_eq!(list[0].parent_id, None);
    assert_eq!(list.len(), 3);
}

#[test]
fn a_lone_text_or_rect_is_not_the_root_a_frame_is_made_around_it() {
    for html in ["<p>x</p>", "<div style='width:5px;height:5px'></div>"] {
        let list = map(html);
        assert_eq!((list.len(), list[0].kind), (2, NodeKind::Frame), "{html}");
        assert_eq!(list[0].name, "Imported HTML");
    }
}

#[test]
fn what_is_not_shown_makes_no_node_and_is_listed() {
    let p = planned(
        "<div data-name=A><p>shown</p><div id=gone style='display:none'><p>inner</p></div>\
         <p hidden>h</p><script>x()</script><style>p{}</style></div>",
        RESET,
    );
    assert_eq!(names(&nodes(&p)), ["A", "p"]);
    let listed: Vec<(&str, &str)> = p
        .report
        .entries
        .iter()
        .filter(|e| e.code == "display_none")
        .map(|e| (e.name.as_str(), e.kind.as_str()))
        .collect();
    assert_eq!(listed, [("div", "listed"), ("p", "listed")]);
    // What the browser's defaults hide is not content: counted, not listed.
    assert_eq!(count(&p, "non_visual"), 1, "{:?}", p.report.counts);
}

#[test]
fn a_text_with_no_element_is_refused_as_empty_and_too_much_is_refused() {
    let e = |html: &str| plan(html, "", &Options::default()).unwrap_err();
    assert_eq!(e("just text"), ParseError::Empty);
    assert_eq!(e("<div style='display:none'>x</div>"), ParseError::Empty);
    assert_eq!(e("<script>x</script><!-- c -->"), ParseError::Empty);
    let big = "a".repeat(200 * 1024);
    let r = plan("<p>x</p>", &big, &Options::default()).map(|_| ());
    assert!(r.is_ok(), "256 KiB is for both together: {r:?}");
    let r = plan(&format!("<p>{big}</p>"), &big, &Options::default());
    assert_eq!(r.unwrap_err().code(), "E_IMPORT_HTML_TOO_LARGE");
}

#[test]
fn a_whole_document_is_read_from_its_body() {
    let html = "<!doctype html><html><head><title>T</title><style>body{margin:0}</style></head>\
                <body><div data-name=Page><p>x</p></div></body></html>";
    let list = nodes(&planned(html, ""));
    assert_eq!(names(&list), ["Page", "p"]);
}

#[test]
fn the_ids_take_the_prefix_of_the_options() {
    let o = Options {
        id_prefix: "imp-abc".into(),
        ..Options::default()
    };
    let p = plan("<div><p>a</p><p>b</p></div>", "", &o).unwrap();
    assert_eq!(p.nodes[2].id, "imp-abc-2");
}
