//! The zoom keys of the key table (E8.1.4): `+`, `=`, `-` and Shift+1, Shift+2 make the camera zoom commands, read as the
//! TypeScript canvas read them (the character, the physical key when there is one, a held key that goes on zooming).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{KeyFocus, KeyInput, Modifiers, ZoomCommand};
use meldframe_app_rules::keys::{Makes, decide};

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

const NONE: Modifiers = Modifiers {
    ctrl: false,
    meta: false,
    alt: false,
    shift: false,
};
const CTRL: Modifiers = Modifiers { ctrl: true, ..NONE };
const META: Modifiers = Modifiers { meta: true, ..NONE };
const ALT: Modifiers = Modifiers { alt: true, ..NONE };
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..NONE
};

#[test]
fn the_zoom_keys_make_the_zoom_commands_and_only_without_ctrl_command_or_option() {
    use ZoomCommand::{Fit, FitSelection, In, Out};
    let zoom = |c| Some(Makes::Zoom(c));
    assert_eq!(decide(&plain("+")), zoom(In));
    assert_eq!(decide(&with("+", SHIFT)), zoom(In));
    assert_eq!(decide(&plain("=")), zoom(In));
    assert_eq!(decide(&plain("-")), zoom(Out));
    // Shift+1 and Shift+2 are the physical keys: the character they type depends on the layout (`!` and `@` on a US one).
    assert_eq!(decide(&input("!", "Digit1", SHIFT)), zoom(Fit));
    assert_eq!(decide(&input("@", "Digit2", SHIFT)), zoom(FitSelection));
    assert_eq!(decide(&input("1", "Digit1", Modifiers::default())), None);
    for m in [CTRL, META, ALT] {
        for key in ["+", "=", "-"] {
            assert_eq!(decide(&with(key, m)), None, "{key} with {m:?}");
        }
    }
    // With Ctrl or Command the digit is no fit key (Option and a digit are the panel keys, which have their own rows).
    for m in [CTRL, META] {
        let key = input("1", "Digit1", Modifiers { shift: true, ..m });
        assert_eq!(decide(&key), None);
    }
}

#[test]
fn zoom_out_is_the_minus_key_with_or_without_shift_and_the_fit_keys_fall_back_to_the_character_when_there_is_no_code()
 {
    use ZoomCommand::{Fit, FitSelection, Out};
    let zoom = |c| Some(Makes::Zoom(c));
    // Shift and the minus key types `_` on a US layout: the TypeScript took both characters
    assert_eq!(decide(&input("_", "Minus", SHIFT)), zoom(Out));
    assert_eq!(
        decide(&input("-", "Minus", Modifiers::default())),
        zoom(Out)
    );
    assert_eq!(decide(&input("_", "", SHIFT)), zoom(Out));
    assert_eq!(decide(&input("-", "", SHIFT)), zoom(Out));
    // a front end that has no physical code gives the character: `1` and `!` (and `2`, `@`) with Shift held
    for (key, want) in [
        ("1", Fit),
        ("!", Fit),
        ("2", FitSelection),
        ("@", FitSelection),
    ] {
        assert_eq!(decide(&input(key, "", SHIFT)), zoom(want), "{key}");
        assert_eq!(
            decide(&input(key, "", Modifiers::default())),
            None,
            "{key} without Shift"
        );
    }
    // with a code, the code decides, as it does for the panel keys: Option and a digit never reach the fit rows
    assert_eq!(decide(&input("!", "Digit2", SHIFT)), zoom(FitSelection));
    assert_eq!(
        decide(&input(
            "¡",
            "",
            Modifiers {
                alt: true,
                ..Modifiers::default()
            }
        )),
        None
    );
}

#[test]
fn a_held_zoom_key_goes_on_zooming_and_no_other_key_repeats() {
    use ZoomCommand::{In, Out};
    for (key, want) in [("+", In), ("=", In), ("-", Out)] {
        let mut k = plain(key);
        k.repeat = true;
        assert_eq!(decide(&k), Some(Makes::Zoom(want)), "{key} held");
    }
    // the fit keys are a command and not a held key
    let mut fit = input("!", "Digit1", SHIFT);
    fit.repeat = true;
    assert_eq!(decide(&fit), None);
}
