//! The shortcut sheet written from the key table (audit finding T7a, decision 43 clause Q1). The expected sheets are literals:
//! what `sheetGroups` of `packages/web-shell/src/shortcuts.ts` gave on 27 Nov 2026 (`node --experimental-strip-types` over that
//! file, both platforms), except for two rows that differ on purpose and were written by hand (E8.1.1): the page has one row,
//! `move-in-flow` ("Move one place in an auto layout frame", the four plain arrows, only with the layout on the canvas, the
//! flow-only `ArrowKey`), and no Shift+arrow; the table has `nudge` (the plain arrows, always) and `nudge-far` (Shift and the
//! arrows). The literals pin the Rust table to the page for every other row; an edit to the TypeScript table fails no test here,
//! and the two copies are kept in step by hand until the TypeScript one is retired (E8.6.1).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_rules::keys::sheet::{sheet, sheet_of};
use meldframe_app_rules::keys::{Binding, Combo, Group, Makes, Row};

fn written(mac: bool, auto_layout: bool) -> Vec<String> {
    sheet(mac, auto_layout)
        .iter()
        .flat_map(|g| {
            g.rows
                .iter()
                .map(move |r| format!("{}|{}|{}|{}", g.id, r.id, r.name, r.keys.join(" ; ")))
        })
        .collect()
}

const PC: &str = "\
tools|tool-select|Select|V
tools|tool-frame|Frame|F
tools|tool-rectangle|Rectangle|R
tools|tool-ellipse|Ellipse|E
tools|tool-text|Text|T
edit|undo|Undo|Ctrl+Z
edit|redo|Redo|Ctrl+Shift+Z ; Ctrl+Y
edit|delete|Delete the selection|Delete ; Backspace
edit|nudge|Move the selection by 1 unit, or one place in an auto layout frame|\u{2191} ; \u{2193} ; \u{2190} ; \u{2192}
edit|nudge-far|Move the selection by 10 units|Shift+\u{2191} ; Shift+\u{2193} ; Shift+\u{2190} ; Shift+\u{2192}
edit|edit-text|Edit the selected text|Enter
edit|commit-text|Finish editing the text|Ctrl+Enter
edit|cancel|Cancel a drag or a text edit|Esc
view|zoom-in|Zoom in|+ ; =
view|zoom-out|Zoom out|-
view|fit-all|Fit all|Shift+1
view|fit-selection|Zoom to selection|Shift+2
view|scroll|Pan|Scroll
view|pan-space|Pan with the pointer|Space + drag
view|zoom-wheel|Zoom at the pointer|Ctrl+Scroll
selection|add-select|Add to or take from the selection|Shift+Click
selection|edit-dblclick|Edit a text|Double-click
panels|layers-panel|Layers panel|Alt+1
panels|right-panel|Right panel|Alt+2
panels|shortcuts|Shortcuts|?
panels|palette|Command palette|Ctrl+K";

const MAC: &str = "\
tools|tool-select|Select|V
tools|tool-frame|Frame|F
tools|tool-rectangle|Rectangle|R
tools|tool-ellipse|Ellipse|E
tools|tool-text|Text|T
edit|undo|Undo|\u{2318}Z
edit|redo|Redo|\u{21e7}\u{2318}Z ; \u{2318}Y
edit|delete|Delete the selection|Delete ; Backspace
edit|nudge|Move the selection by 1 unit, or one place in an auto layout frame|\u{2191} ; \u{2193} ; \u{2190} ; \u{2192}
edit|nudge-far|Move the selection by 10 units|\u{21e7}\u{2191} ; \u{21e7}\u{2193} ; \u{21e7}\u{2190} ; \u{21e7}\u{2192}
edit|edit-text|Edit the selected text|Enter
edit|commit-text|Finish editing the text|\u{2318} Enter
edit|cancel|Cancel a drag or a text edit|Esc
view|zoom-in|Zoom in|+ ; =
view|zoom-out|Zoom out|-
view|fit-all|Fit all|\u{21e7}1
view|fit-selection|Zoom to selection|\u{21e7}2
view|scroll|Pan|Scroll
view|pan-space|Pan with the pointer|Space + drag
view|zoom-wheel|Zoom at the pointer|\u{2318} Scroll
selection|add-select|Add to or take from the selection|\u{21e7} Click
selection|edit-dblclick|Edit a text|Double-click
panels|layers-panel|Layers panel|\u{2325}1
panels|right-panel|Right panel|\u{2325}2
panels|shortcuts|Shortcuts|?
panels|palette|Command palette|\u{2318}K";

#[test]
fn the_sheet_written_from_the_table_is_the_sheet_the_page_has_but_for_the_two_arrow_rows() {
    assert_eq!(written(false, true).join("\n"), PC);
    assert_eq!(written(true, true).join("\n"), MAC);
}

#[test]
fn a_row_that_needs_auto_layout_is_in_the_sheet_only_while_it_is_on() {
    // The table has no such row since the arrow keys became the nudge, so the rule is read from a table made for it.
    const ROWS: &[Row] = &[
        Row {
            id: "always",
            name: "Always",
            needs_auto_layout: false,
            bindings: &[Binding {
                combo: Combo::key("A"),
                makes: Makes::Undo,
                repeats: false,
            }],
        },
        Row {
            id: "laid-out",
            name: "Only with the layout",
            needs_auto_layout: true,
            bindings: &[Binding {
                combo: Combo::key("B"),
                makes: Makes::Undo,
                repeats: false,
            }],
        },
    ];
    const GROUPS: &[Group] = &[Group {
        id: "g",
        title: "G",
        rows: ROWS,
    }];
    let ids = |auto| -> Vec<&str> {
        sheet_of(GROUPS, false, auto)[0]
            .rows
            .iter()
            .map(|r| r.id)
            .collect()
    };
    assert_eq!(ids(true), ["always", "laid-out"]);
    assert_eq!(ids(false), ["always"]);
}

#[test]
fn the_groups_have_the_ids_and_titles_the_page_has() {
    let groups: Vec<(&str, &str)> = sheet(false, true).iter().map(|g| (g.id, g.title)).collect();
    assert_eq!(
        groups,
        [
            ("tools", "Tools"),
            ("edit", "Edit"),
            ("view", "View"),
            ("selection", "Selection"),
            ("panels", "Panels and help"),
        ]
    );
    assert_eq!(
        sheet(true, true)
            .iter()
            .map(|g| g.title)
            .collect::<Vec<_>>(),
        groups.iter().map(|g| g.1).collect::<Vec<_>>()
    );
}
