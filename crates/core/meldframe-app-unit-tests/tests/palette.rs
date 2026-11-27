//! The run palette as a function of the app state and a query (audit finding T7c, decision 43 clause Q2, issue E8.1.5): its
//! entries and the switch "Auto layout on canvas" (`palette_rank.rs` has the search, the ranking and the keys). The cases are those of
//! `packages/web-shell/src/palette.test.ts`, with the same expected order; that file stays as the legacy copy until the web front ends
//! are retired. The words are the palette's (U5): they are fixed text, and none of them carries text from a file (U9).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::palette::*;
use meldframe_app::{FramePreset, Panel, Tool};
use meldframe_app_rules::keys::sheet::sheet;
use meldframe_app_rules::palette::{Entry, Run, Zoom, entries};

const PRESET_IDS: [&str; 7] = [
    "preset-desktop",
    "preset-desktopHd",
    "preset-tablet",
    "preset-mobile",
    "preset-phoneSmall",
    "preset-a4",
    "preset-letter",
];

#[test]
fn the_entries_list_what_the_editor_has_each_id_once_with_a_name_and_a_group() {
    let es = list(ON, false, None);
    let mut want = vec![
        "tool-select",
        "tool-frame",
        "tool-rectangle",
        "tool-ellipse",
        "tool-text",
        "undo",
        "redo",
        "delete",
    ];
    want.extend(PRESET_IDS);
    want.extend([
        "zoom-in",
        "zoom-out",
        "zoom-reset",
        "zoom-fit",
        "zoom-fit-selection",
        "toggle-layers",
        "toggle-inspector",
        "export",
        "variant",
        "shortcuts",
    ]);
    assert_eq!(ids(&es), want);
    let mut once = ids(&es);
    once.sort_unstable();
    once.dedup();
    assert_eq!(once.len(), es.len());
    for e in &es {
        assert!(
            !e.label.trim().is_empty() && !e.group.trim().is_empty(),
            "{}",
            e.id
        );
    }
}

#[test]
fn the_keys_shown_are_the_keys_of_the_sheet_written_for_the_platform_and_none_is_invented() {
    for mac in [false, true] {
        let es = list(ON, mac, None);
        let sheet_keys: Vec<String> = sheet(mac, true)
            .iter()
            .flat_map(|g| g.rows.iter().flat_map(|r| r.keys.clone()))
            .collect();
        for e in es.iter().filter(|e| e.shortcut.is_some()) {
            let k = e.shortcut.as_ref().unwrap();
            assert!(sheet_keys.contains(k), "{}: {k}", e.id);
        }
        let key = |id: &str| by(&es, id).shortcut.clone();
        assert_eq!(
            key("undo").as_deref(),
            Some(if mac { "\u{2318}Z" } else { "Ctrl+Z" })
        );
        assert_eq!(
            key("redo").as_deref(),
            Some(if mac {
                "\u{21e7}\u{2318}Z"
            } else {
                "Ctrl+Shift+Z"
            })
        );
        assert_eq!(key("tool-frame").as_deref(), Some("F"));
        assert_eq!(key("tool-ellipse").as_deref(), Some("E"));
        assert_eq!(by(&es, "tool-ellipse").label, "Ellipse tool");
        assert_eq!(
            key("zoom-fit").as_deref(),
            Some(if mac { "\u{21e7}1" } else { "Shift+1" })
        );
        assert_eq!(
            key("toggle-layers").as_deref(),
            Some(if mac { "\u{2325}1" } else { "Alt+1" })
        );
        assert_eq!(key("shortcuts").as_deref(), Some("?"));
        assert_eq!(key("zoom-reset"), None);
        assert_eq!(key("export"), None);
    }
}

#[test]
fn an_entry_is_enabled_only_where_the_editor_can_do_it() {
    let flags =
        |es: &[Entry]| ["undo", "redo", "delete", "export", "variant"].map(|id| by(es, id).enabled);
    let on = list(ON, false, None);
    assert_eq!(flags(&on), [true, false, true, true, true]);
    let off = list(
        Flags {
            can_undo: false,
            can_redo: true,
            can_delete: false,
            can_export: false,
            ..ON
        },
        false,
        None,
    );
    assert_eq!(flags(&off), [false, true, false, false, false]);
    assert!(on.iter().filter(|e| e.id != "redo").all(|e| e.enabled));
}

#[test]
fn a_panel_toggle_is_named_by_what_it_does_now() {
    let name = |layers_open, inspector_open| {
        let es = list(
            Flags {
                layers_open,
                inspector_open,
                ..ON
            },
            false,
            None,
        );
        [
            by(&es, "toggle-layers").label.clone(),
            by(&es, "toggle-inspector").label.clone(),
        ]
    };
    assert_eq!(
        name(false, false),
        ["Show layers panel", "Show right panel"]
    );
    assert_eq!(name(true, true), ["Hide layers panel", "Hide right panel"]);
}

#[test]
fn there_is_one_entry_for_each_frame_preset_with_its_name_and_size() {
    let es = list(ON, false, None);
    let frames: Vec<(&str, &str)> = es
        .iter()
        .filter(|e| e.group == "Frames")
        .map(|e| (e.label.as_str(), e.detail.as_deref().unwrap()))
        .collect();
    assert_eq!(
        frames,
        [
            ("New frame: Desktop", "1440 \u{d7} 900"),
            ("New frame: Desktop HD", "1920 \u{d7} 1080"),
            ("New frame: Tablet", "768 \u{d7} 1024"),
            ("New frame: Mobile", "390 \u{d7} 844"),
            ("New frame: Phone small", "375 \u{d7} 667"),
            ("New frame: A4", "595 \u{d7} 842"),
            ("New frame: Letter", "612 \u{d7} 792"),
        ]
    );
}

#[test]
fn each_entry_makes_the_command_the_editor_already_has_for_it() {
    let es = list(ON, false, None);
    let makes = |id: &str| by(&es, id).run;
    assert_eq!(makes("tool-rectangle"), Run::Tool(Tool::Rectangle));
    assert_eq!(makes("undo"), Run::Undo);
    assert_eq!(makes("redo"), Run::Redo);
    assert_eq!(makes("delete"), Run::DeleteSelection);
    assert_eq!(makes("preset-a4"), Run::NewFrame(FramePreset::A4));
    assert_eq!(makes("zoom-fit-selection"), Run::Zoom(Zoom::FitSelection));
    assert_eq!(makes("zoom-in"), Run::Zoom(Zoom::In));
    assert_eq!(makes("toggle-layers"), Run::TogglePanel(Panel::Layers));
    assert_eq!(
        makes("toggle-inspector"),
        Run::TogglePanel(Panel::Properties)
    );
    assert_eq!(makes("export"), Run::Export);
    assert_eq!(makes("variant"), Run::Variant);
    assert_eq!(makes("shortcuts"), Run::Help);
}

#[test]
fn an_entry_that_opens_a_dialog_says_so() {
    let es = list(ON, false, None);
    let dialogs: Vec<&str> = es
        .iter()
        .filter(|e| e.opens_dialog)
        .map(|e| e.id.as_str())
        .collect();
    assert_eq!(dialogs, ["export", "shortcuts"]);
}

#[test]
fn no_label_carries_text_from_the_file() {
    // The selected frame is named by whoever wrote the file: the palette words are its own, whatever that name is.
    let plain = list(ON, false, Some(true));
    let mut hostile = state(ON);
    hostile.export.target.as_mut().unwrap().name = "\u{202e}Export\u{200b}".into();
    let hostile = entries(&hostile, false, Some(true));
    assert_eq!(plain, hostile);
    for e in &hostile {
        assert!(
            e.label
                .chars()
                .all(|c| !c.is_control() && !matches!(c, '\u{200b}' | '\u{202e}')),
            "{}",
            e.id
        );
    }
}

// The entry "Auto layout on canvas".

#[test]
fn the_switch_entry_is_there_only_when_the_page_has_the_switch_in_the_view_group() {
    assert!(list(ON, false, None).iter().all(|e| e.id != "auto-layout"));
    let es = list(ON, false, Some(false));
    let e = by(&es, "auto-layout");
    assert_eq!(
        (e.label.as_str(), e.group, e.enabled),
        ("Auto layout on canvas", "View", true)
    );
    assert_eq!(e.run, Run::AutoLayout);
    let without: Vec<&str> = ids(&es)
        .into_iter()
        .filter(|i| *i != "auto-layout")
        .collect();
    assert_eq!(without, ids(&list(ON, false, None)));
}

#[test]
fn the_switch_entry_says_what_is_on() {
    let on = list(ON, false, Some(true));
    assert!(
        by(&on, "auto-layout")
            .detail
            .as_deref()
            .unwrap()
            .starts_with("On")
    );
    let off = list(ON, false, Some(false));
    assert!(
        by(&off, "auto-layout")
            .detail
            .as_deref()
            .unwrap()
            .starts_with("Off")
    );
}

#[test]
fn the_switch_entry_is_found_by_a_few_words_of_it_and_by_what_it_is_about() {
    let es = list(ON, false, Some(true));
    for q in ["auto layout", "layout canvas", "hug", "switch"] {
        assert!(found(&es, q).iter().any(|i| i == "auto-layout"), "{q}");
    }
    assert_eq!(found(&es, "auto layout")[0], "auto-layout");
}
