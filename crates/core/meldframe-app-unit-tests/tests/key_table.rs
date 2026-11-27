//! The key table and the rule that reads it (audit finding T7a, decision 43 clause Q1): every row, the modifier variants, and the
//! keys a front end must never see as editor keys.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Direction, KeyFocus, KeyInput, Modifiers, Overlay, Panel, Tool, ZoomCommand};
use meldframe_app_rules::keys::{Binding, Combo, Makes, TABLE, decide};

fn bindings() -> Vec<(&'static str, Binding)> {
    TABLE
        .iter()
        .flat_map(|g| g.rows)
        .flat_map(|r| r.bindings.iter().map(move |b| (r.id, *b)))
        .collect()
}

fn input(key: &str, code: &str, m: Modifiers) -> KeyInput {
    KeyInput {
        key: key.into(),
        code: code.into(),
        modifiers: m,
        repeat: false,
        composing: false,
        focus: KeyFocus::Page,
    }
}

fn plain(key: &str) -> KeyInput {
    input(key, "", Modifiers::default())
}

fn with(key: &str, m: Modifiers) -> KeyInput {
    input(key, "", m)
}

const CTRL: Modifiers = Modifiers {
    ctrl: true,
    meta: false,
    alt: false,
    shift: false,
};
const META: Modifiers = Modifiers {
    ctrl: false,
    meta: true,
    alt: false,
    shift: false,
};
const ALT: Modifiers = Modifiers {
    ctrl: false,
    meta: false,
    alt: true,
    shift: false,
};
const SHIFT: Modifiers = Modifiers {
    ctrl: false,
    meta: false,
    alt: false,
    shift: true,
};

/// Every press that is the combo: each way the free modifiers can be held, and Ctrl and Command for the command key.
fn presses(c: &Combo) -> Vec<KeyInput> {
    let mut out = Vec::new();
    for command in if c.command {
        vec![CTRL, META]
    } else {
        vec![Modifiers::default()]
    } {
        for alt in [false, true] {
            for shift in [false, true] {
                if !c.alt.allows(alt) || !c.shift.allows(shift) {
                    continue;
                }
                let m = Modifiers {
                    alt,
                    shift,
                    ..command
                };
                out.push(input(c.key, c.code.unwrap_or(""), m));
            }
        }
    }
    out
}

/// Every thing the app layer makes of a key, written out so that a new one does not compile until it is listed here (and so
/// until the test below asks the table for a row of it).
fn every_makes() -> Vec<Makes> {
    let all = vec![
        Makes::SetTool(Tool::Select),
        Makes::SetTool(Tool::Frame),
        Makes::SetTool(Tool::Rectangle),
        Makes::SetTool(Tool::Ellipse),
        Makes::SetTool(Tool::Text),
        Makes::Undo,
        Makes::Redo,
        Makes::DeleteSelection,
        Makes::Nudge(Direction::Up),
        Makes::Nudge(Direction::Down),
        Makes::Nudge(Direction::Left),
        Makes::Nudge(Direction::Right),
        Makes::BeginTextEdit,
        Makes::Cancel,
        Makes::TogglePanel(Panel::Layers),
        Makes::TogglePanel(Panel::Properties),
        Makes::ToggleOverlay(Overlay::Help),
        Makes::ToggleOverlay(Overlay::Palette),
        Makes::Zoom(ZoomCommand::In),
        Makes::Zoom(ZoomCommand::Out),
        Makes::Zoom(ZoomCommand::Fit),
        Makes::Zoom(ZoomCommand::FitSelection),
    ];
    for m in &all {
        match m {
            Makes::SetTool(_)
            | Makes::Undo
            | Makes::Redo
            | Makes::DeleteSelection
            | Makes::Nudge(_)
            | Makes::BeginTextEdit
            | Makes::Cancel
            | Makes::TogglePanel(_)
            | Makes::ToggleOverlay(_)
            | Makes::Zoom(_)
            | Makes::Host => {}
        }
    }
    all
}

#[test]
fn the_table_has_named_rows_with_a_key_each_and_no_id_twice() {
    assert!(TABLE.len() > 2);
    let mut ids: Vec<&str> = Vec::new();
    for g in TABLE {
        assert!(!g.title.trim().is_empty() && !g.rows.is_empty(), "{}", g.id);
        for r in g.rows {
            assert!(
                !r.name.trim().is_empty() && !r.bindings.is_empty(),
                "{}",
                r.id
            );
            ids.push(r.id);
        }
    }
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "an id is used twice");
}

#[test]
fn every_thing_the_app_makes_of_a_key_has_a_row_and_every_row_makes_one() {
    let table: Vec<Makes> = bindings().iter().map(|(_, b)| b.makes).collect();
    for m in every_makes() {
        assert!(table.contains(&m), "{m:?} has no row in the table");
    }
    let known = every_makes();
    for m in &table {
        assert!(
            *m == Makes::Host || known.contains(m),
            "{m:?} is in the table and not in the list of handlers"
        );
    }
}

#[test]
fn every_binding_makes_what_its_row_says_for_each_way_of_pressing_it() {
    for (id, b) in bindings() {
        for key in presses(&b.combo) {
            let want = (b.makes != Makes::Host).then_some(b.makes);
            assert_eq!(decide(&key), want, "{id}: {key:?}");
        }
    }
}

#[test]
fn no_two_rows_answer_the_same_press() {
    let all = bindings();
    for (id, b) in &all {
        for key in presses(&b.combo) {
            let hits: Vec<&str> = all
                .iter()
                .filter(|(_, o)| o.combo.matches(&key))
                .map(|(i, _)| *i)
                .collect();
            assert_eq!(hits, vec![*id], "{key:?}");
        }
    }
}

#[test]
fn command_is_ctrl_or_meta_on_every_platform_and_is_never_free() {
    for (id, b) in bindings()
        .iter()
        .filter(|(_, b)| b.combo.command && b.makes != Makes::Host)
    {
        for mut key in presses(&b.combo) {
            assert!(key.modifiers.command(), "{id}");
            key.modifiers.ctrl = false;
            key.modifiers.meta = false;
            assert_eq!(decide(&key), None, "{id} without Ctrl or Command");
        }
    }
    let both = Modifiers {
        ctrl: true,
        meta: true,
        ..Modifiers::default()
    };
    assert_eq!(decide(&with("z", both)), Some(Makes::Undo));
    assert_eq!(decide(&with("z", CTRL)), decide(&with("Z", META)));
}

#[test]
fn a_letter_matches_in_either_case_and_a_named_key_only_as_it_is_named() {
    assert_eq!(decide(&plain("v")), Some(Makes::SetTool(Tool::Select)));
    assert_eq!(decide(&plain("V")), Some(Makes::SetTool(Tool::Select)));
    assert_eq!(
        decide(&with("V", SHIFT)),
        Some(Makes::SetTool(Tool::Select))
    );
    assert_eq!(decide(&plain("Delete")), Some(Makes::DeleteSelection));
    assert_eq!(decide(&plain("delete")), None);
    assert_eq!(decide(&plain("arrowup")), None);
}

#[test]
fn undo_and_redo_have_their_variants_and_no_others() {
    for m in [CTRL, META] {
        assert_eq!(decide(&with("z", m)), Some(Makes::Undo));
        assert_eq!(decide(&with("y", m)), Some(Makes::Redo));
        let shifted = Modifiers { shift: true, ..m };
        assert_eq!(decide(&with("Z", shifted)), Some(Makes::Redo));
        assert_eq!(decide(&with("y", shifted)), None);
        let alted = Modifiers { alt: true, ..m };
        assert_eq!(decide(&with("z", alted)), None, "Ctrl+Alt+Z is not undo");
    }
    assert_eq!(
        decide(&with("Z", SHIFT)),
        None,
        "Shift+Z without Ctrl or Command is not redo"
    );
    assert_eq!(decide(&plain("z")), None);
}

#[test]
fn the_panel_keys_are_the_physical_digit_keys_and_option_changes_the_character() {
    // On a Mac, Option+1 types a different character, so the key says nothing and the code decides.
    assert_eq!(
        decide(&input("\u{a1}", "Digit1", ALT)),
        Some(Makes::TogglePanel(Panel::Layers))
    );
    assert_eq!(
        decide(&input("\u{2122}", "Digit2", ALT)),
        Some(Makes::TogglePanel(Panel::Properties))
    );
    assert_eq!(
        decide(&input("1", "Digit1", Modifiers::default())),
        None,
        "a digit alone is not a panel key"
    );
    assert_eq!(decide(&input("1", "Digit3", ALT)), None);
    assert_eq!(
        decide(&input(
            "1",
            "Digit1",
            Modifiers {
                alt: true,
                ctrl: true,
                ..Modifiers::default()
            }
        )),
        None
    );
    assert_eq!(
        decide(&input("1", "", ALT)),
        None,
        "with no code there is no physical key to read"
    );
}

#[test]
fn the_sheet_key_and_the_palette_key() {
    assert_eq!(
        decide(&with("?", SHIFT)),
        Some(Makes::ToggleOverlay(Overlay::Help))
    );
    assert_eq!(
        decide(&plain("?")),
        Some(Makes::ToggleOverlay(Overlay::Help))
    );
    assert_eq!(decide(&with("?", CTRL)), None);
    assert_eq!(
        decide(&with("k", CTRL)),
        Some(Makes::ToggleOverlay(Overlay::Palette))
    );
    assert_eq!(
        decide(&with("K", META)),
        Some(Makes::ToggleOverlay(Overlay::Palette))
    );
    assert_eq!(
        decide(&with(
            "k",
            Modifiers {
                shift: true,
                ..CTRL
            }
        )),
        None
    );
    assert_eq!(decide(&with("k", Modifiers { alt: true, ..CTRL })), None);
    assert_eq!(decide(&plain("k")), None);
}

#[test]
fn enter_and_escape_are_plain_keys() {
    assert_eq!(decide(&plain("Enter")), Some(Makes::BeginTextEdit));
    assert_eq!(
        decide(&with("Enter", CTRL)),
        None,
        "Ctrl+Enter finishes a text edit: the field's"
    );
    assert_eq!(decide(&with("Enter", SHIFT)), None);
    assert_eq!(decide(&plain("Escape")), Some(Makes::Cancel));
    assert_eq!(decide(&with("Escape", ALT)), None);
}

#[test]
fn the_keys_the_front_end_answers_make_nothing() {
    for key in [" ", "Tab", "Shift"] {
        assert_eq!(decide(&plain(key)), None, "{key}");
    }
    for word in ["Scroll", "Click", "Double-click", "Space + drag"] {
        assert_eq!(decide(&plain(word)), None, "{word} is a gesture, not a key");
    }
}

#[test]
fn a_key_during_composition_a_repeat_or_in_a_field_is_never_an_editor_key() {
    for (id, b) in bindings() {
        for mut key in presses(&b.combo) {
            key.composing = true;
            assert_eq!(decide(&key), None, "{id} while composing");
            key.composing = false;
            key.repeat = true;
            // a held key is no editor key, but the zoom steps (`+`, `=`, `-`), which go on zooming
            assert_eq!(decide(&key), b.repeats.then_some(b.makes), "{id} repeating");
            key.repeat = false;
            key.focus = KeyFocus::TextField;
            assert_eq!(decide(&key), None, "{id} in a text field");
        }
    }
}

#[test]
fn a_region_keeps_the_plain_keys_and_lets_command_and_option_combinations_through() {
    let region = |mut k: KeyInput| {
        k.focus = KeyFocus::Region;
        k
    };
    for key in [
        "v",
        "Delete",
        "Backspace",
        "ArrowUp",
        "Enter",
        "Escape",
        "?",
    ] {
        assert_eq!(
            decide(&region(plain(key))),
            None,
            "{key} in a menu or a dialog"
        );
    }
    assert_eq!(decide(&region(with("V", SHIFT))), None);
    assert_eq!(decide(&region(with("z", CTRL))), Some(Makes::Undo));
    assert_eq!(
        decide(&region(with("k", META))),
        Some(Makes::ToggleOverlay(Overlay::Palette))
    );
    assert_eq!(
        decide(&region(input("1", "Digit1", ALT))),
        Some(Makes::TogglePanel(Panel::Layers))
    );
}

#[test]
fn a_control_keeps_enter_and_nothing_else() {
    let control = |mut k: KeyInput| {
        k.focus = KeyFocus::Control;
        k
    };
    assert_eq!(decide(&control(plain("Enter"))), None);
    assert_eq!(
        decide(&control(plain("v"))),
        Some(Makes::SetTool(Tool::Select))
    );
    assert_eq!(
        decide(&control(plain("Delete"))),
        Some(Makes::DeleteSelection)
    );
}

#[test]
fn an_unknown_key_is_no_editor_key() {
    let long = "v".repeat(10_000);
    for key in [
        "",
        "F13",
        "Dead",
        "Unidentified",
        "\u{202e}",
        "vv",
        long.as_str(),
        "\u{1F300}",
    ] {
        assert_eq!(decide(&plain(key)), None, "{key:?}");
        assert_eq!(decide(&with(key, CTRL)), None, "{key:?} with Ctrl");
    }
}
