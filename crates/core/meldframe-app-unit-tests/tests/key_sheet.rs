//! The shortcut sheet written from the key table (audit finding T7a, decision 43 clause Q1). The expected sheet is what `sheetGroups`
//! of `packages/web-shell/src/shortcuts.ts` gave when the table was moved (`node --experimental-strip-types` over that file), so
//! the two cannot drift apart unnoticed.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_rules::keys::sheet::sheet;

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
edit|move-in-flow|Move one place in an auto layout frame|\u{2191} ; \u{2193} ; \u{2190} ; \u{2192}
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
edit|move-in-flow|Move one place in an auto layout frame|\u{2191} ; \u{2193} ; \u{2190} ; \u{2192}
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
fn the_sheet_written_from_the_table_is_the_sheet_the_page_has() {
    assert_eq!(written(false, true).join("\n"), PC);
    assert_eq!(written(true, true).join("\n"), MAC);
}

#[test]
fn the_row_that_needs_auto_layout_is_in_the_sheet_only_while_it_is_on() {
    let on = written(false, true);
    let off = written(false, false);
    assert!(on.iter().any(|l| l.contains("|move-in-flow|")));
    assert!(!off.iter().any(|l| l.contains("|move-in-flow|")));
    assert_eq!(off.len() + 1, on.len());
}
