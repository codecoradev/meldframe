//! The search, the ranking and the keys of the run palette (issue E8.1.5): the cases of `packages/web-shell/src/palette.test.ts`
//! with the same expected order.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::palette::*;
use meldframe_app_rules::palette::{
    Entry, KeyResult, Run, filter, first_enabled, highlight_at, on_key, results_text,
};

fn custom(id: &str, label: &str, keywords: &'static [&'static str]) -> Entry {
    Entry {
        id: id.into(),
        label: label.into(),
        group: "G",
        shortcut: None,
        detail: None,
        enabled: true,
        keywords,
        opens_dialog: false,
        run: Run::Undo,
    }
}

#[test]
fn an_empty_or_blank_query_keeps_everything_in_order() {
    let es = list(ON, false, None);
    assert_eq!(found(&es, ""), ids(&es));
    assert_eq!(found(&es, "   "), ids(&es));
}

#[test]
fn case_and_the_space_around_the_words_do_not_matter() {
    assert_eq!(found(&list(ON, false, None), "  UNDO "), ["undo"]);
}

#[test]
fn a_label_that_starts_with_the_word_beats_a_word_that_does_then_any_match_then_a_keyword() {
    let es = [
        custom("context", "Context menu", &[]),
        custom("retext", "Retext", &[]),
        custom("bold", "Bold text", &[]),
        custom("tool", "Text tool", &[]),
        custom("zzz", "Zzz", &["text"]),
    ];
    assert_eq!(
        found(&es, "text"),
        ["tool", "bold", "context", "retext", "zzz"]
    );
}

#[test]
fn the_scores_of_the_words_add_up() {
    // "text" and "tool" score 4 + 1 for the first (a label that starts with the one, a keyword for the other) and 3 + 3 for the
    // second (a word of the label for each): the sum puts the second first, the best single word would put the first.
    let es = [
        custom("box", "Text box", &["tool"]),
        custom("draw", "Draw text tool", &[]),
    ];
    assert_eq!(found(&es, "text tool"), ["draw", "box"]);
}

#[test]
fn the_entry_of_the_same_name_comes_first_for_a_word_of_its_label() {
    let es = list(ON, false, None);
    assert_eq!(found(&es, "fit"), ["zoom-fit", "zoom-fit-selection"]);
    assert_eq!(&found(&es, "frame")[..2], ["tool-frame", "preset-desktop"]);
}

#[test]
fn every_word_has_to_match() {
    let es = list(ON, false, None);
    assert_eq!(found(&es, "new a4"), ["preset-a4"]);
    assert_eq!(found(&es, "zoom selection"), ["zoom-fit-selection"]);
    assert!(found(&es, "zoom banana").is_empty());
}

#[test]
fn an_entry_is_found_by_its_size_its_group_or_a_keyword() {
    let es = list(ON, false, None);
    assert_eq!(found(&es, "1920"), ["preset-desktopHd"]);
    assert_eq!(found(&es, "png"), ["export"]);
    assert_eq!(found(&es, "keyboard"), ["shortcuts"]);
    assert_eq!(found(&es, "circle"), ["tool-ellipse"]);
    assert_eq!(found(&es, "oval"), ["tool-ellipse"]);
    assert_eq!(found(&es, "panels"), ["toggle-layers", "toggle-inspector"]);
}

#[test]
fn an_entry_that_cannot_run_comes_after_those_that_can_and_stays_in_the_list() {
    let order = |can_undo, can_redo| -> Vec<(String, bool)> {
        let es = list(
            Flags {
                can_undo,
                can_redo,
                ..ON
            },
            false,
            None,
        );
        filter(&es, "do")
            .into_iter()
            .map(|e| (e.id, e.enabled))
            .collect()
    };
    let row = |id: &str, on| (id.to_string(), on);
    assert_eq!(order(true, false), [row("undo", true), row("redo", false)]);
    assert_eq!(order(false, true), [row("redo", true), row("undo", false)]);
    assert_eq!(
        order(false, false),
        [row("undo", false), row("redo", false)]
    );
}

#[test]
fn what_is_typed_is_text_and_not_a_pattern() {
    let es = list(ON, false, None);
    assert!(found(&es, ".*").is_empty());
    assert!(found(&es, "(").is_empty());
}

// The keys of the palette.

fn rows(enabled: &[bool]) -> Vec<Entry> {
    enabled
        .iter()
        .map(|&on| Entry {
            enabled: on,
            ..custom("x", "X", &[])
        })
        .collect()
}

#[test]
fn the_highlight_starts_on_the_first_entry_that_can_run() {
    assert_eq!(first_enabled(&rows(&[false, true, true])), Some(1));
    assert_eq!(first_enabled(&rows(&[false, false])), None);
    assert_eq!(first_enabled(&[]), None);
}

#[test]
fn the_arrows_move_over_what_can_run_and_wrap_round() {
    let r = rows(&[true, false, true]);
    assert_eq!(on_key("ArrowDown", Some(0), &r), Some(KeyResult::Move(2)));
    assert_eq!(on_key("ArrowDown", Some(2), &r), Some(KeyResult::Move(0)));
    assert_eq!(on_key("ArrowUp", Some(0), &r), Some(KeyResult::Move(2)));
    assert_eq!(on_key("ArrowUp", Some(2), &r), Some(KeyResult::Move(0)));
}

#[test]
fn down_goes_to_the_next_row_and_up_to_the_one_before() {
    let r = rows(&[true, true, true, true]);
    assert_eq!(on_key("ArrowDown", Some(1), &r), Some(KeyResult::Move(2)));
    assert_eq!(on_key("ArrowUp", Some(1), &r), Some(KeyResult::Move(0)));
    assert_eq!(on_key("ArrowUp", Some(0), &r), Some(KeyResult::Move(3)));
    assert_eq!(on_key("ArrowDown", Some(3), &r), Some(KeyResult::Move(0)));
}

/// The TypeScript counts "no row highlighted" as -1, so Down goes to the first row and Up to the one before the last (-1 minus one,
/// round the ends). The Rust keeps it: the same keys give the same row in both front ends until the TypeScript is retired.
#[test]
fn with_no_row_highlighted_down_goes_to_the_first_and_up_to_the_one_before_the_last() {
    let r = rows(&[true, true, true, true]);
    assert_eq!(on_key("ArrowDown", None, &r), Some(KeyResult::Move(0)));
    assert_eq!(on_key("ArrowUp", None, &r), Some(KeyResult::Move(2)));
}

#[test]
fn enter_runs_the_active_entry_unless_it_cannot_run() {
    assert_eq!(
        on_key("Enter", Some(1), &rows(&[true, true])),
        Some(KeyResult::Run(1))
    );
    assert_eq!(on_key("Enter", Some(1), &rows(&[true, false])), None);
    assert_eq!(on_key("Enter", None, &rows(&[true])), None);
    assert_eq!(on_key("Enter", Some(3), &rows(&[true])), None);
}

#[test]
fn an_empty_list_does_nothing_and_every_other_key_is_the_fields() {
    assert_eq!(on_key("ArrowDown", None, &[]), None);
    assert_eq!(on_key("Enter", Some(0), &[]), None);
    assert_eq!(on_key("ArrowDown", Some(0), &rows(&[false, false])), None);
    for k in ["Home", "End", "a", "Tab", "ArrowLeft", "ArrowRight"] {
        assert_eq!(on_key(k, Some(0), &rows(&[true, true])), None, "{k}");
    }
}

// The highlighted row and the count.

#[test]
fn the_highlighted_row_starts_on_the_first_that_can_run_and_follows_the_pointer_while_it_can() {
    let mk = |id: &str, on| Entry {
        id: id.into(),
        enabled: on,
        ..custom("x", "X", &[])
    };
    let shown = [mk("a", false), mk("b", true), mk("c", true), mk("d", false)];
    assert_eq!(highlight_at(&shown, None), Some(1));
    assert_eq!(highlight_at(&[], None), None);
    assert_eq!(
        highlight_at(&[shown[0].clone(), shown[3].clone()], None),
        None
    );
    assert_eq!(highlight_at(&shown, Some("c")), Some(2));
    assert_eq!(highlight_at(&shown, Some("d")), Some(1));
    assert_eq!(highlight_at(&shown, Some("gone")), Some(1));
}

#[test]
fn the_count_read_to_a_screen_reader_says_no_results_one_result_or_how_many() {
    let said: Vec<String> = [0, 1, 2, 25].into_iter().map(results_text).collect();
    assert_eq!(said, ["No results", "1 result", "2 results", "25 results"]);
}
