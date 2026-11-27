//! The key table and the rule that reads it (audit finding T7a, decision 43 clause Q1): every row, the modifier variants, and the
//! keys a front end must never see as editor keys.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Direction, KeyFocus, KeyInput, Modifiers, Overlay, Panel, Tool};
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
