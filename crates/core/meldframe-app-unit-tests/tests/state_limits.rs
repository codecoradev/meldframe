//! The limits and tables a front end shows are state, not copies (audit finding T3, issue 321): `AppState.limits` holds
//! the ceiling of an import, the pixel limit and the scales of a PNG export and the table of frame presets, and each
//! value is the one the rule that uses it reads. The manage view's limits stay in `state.manage.limits`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{AppState, FramePreset, MODE_NAME_MAX, ManageLimits, SCALES};
use meldframe_check::Limits;
use serde_json::json;

#[test]
fn state_carries_limits() {
    let app = app();
    let state = app.state();
    let core = Limits::default();

    // The manage view's limits are the core's `Limits` (and a mode's name is a short label).
    assert_eq!(
        state.manage.limits,
        ManageLimits {
            name_max: core.max_name,
            mode_name_max: MODE_NAME_MAX,
            collections: core.max_collections,
            modes: core.max_modes,
            variables: core.max_variables,
        }
    );

    // The most variables the Import variables dialog takes, the dialog's own constant.
    assert_eq!(
        state.limits.import_variables,
        meldframe_app::variables_import_dialog::MAX_VARIABLES
    );

    // The pixel limit of an export is the area limit of a capture: the server reads the same constant for
    // `CAPTURE.max_pixels` (its test is in `apps/server/src/main.rs`), so this pin of the number (1 << 24, which the TypeScript
    // copy and the wasm bridge test repeat) is the one place a change of the limit has to be made on purpose. The scales are
    // the ones the export rules accept.
    assert_eq!(state.limits.export_max_pixels, 1 << 24);
    assert_eq!(state.limits.export_scales, SCALES.to_vec());
}

#[test]
fn state_carries_the_preset_table_in_menu_order() {
    let app = app();
    let rows: Vec<_> = app
        .state()
        .limits
        .presets
        .iter()
        .map(|p| (p.id, p.name.as_str(), p.width, p.height))
        .collect();
    assert_eq!(
        rows,
        vec![
            (FramePreset::Desktop, "Desktop", 1440.0, 900.0),
            (FramePreset::DesktopHd, "Desktop HD", 1920.0, 1080.0),
            (FramePreset::Tablet, "Tablet", 768.0, 1024.0),
            (FramePreset::Mobile, "Mobile", 390.0, 844.0),
            (FramePreset::PhoneSmall, "Phone small", 375.0, 667.0),
            (FramePreset::A4, "A4", 595.0, 842.0),
            (FramePreset::Letter, "Letter", 612.0, 792.0),
        ]
    );
    // The table is the rule's own: each row is what `FramePreset` makes.
    for (row, preset) in app.state().limits.presets.iter().zip(FramePreset::ALL) {
        assert_eq!(row.id, preset);
        assert_eq!((row.width, row.height), preset.size());
        assert_eq!(row.name, preset.name());
    }
}

#[test]
fn the_limits_have_a_wire_shape_and_survive_a_round_trip() {
    let state = serde_json::to_value(app().state()).unwrap();
    let limits = &state["limits"];
    assert_eq!(limits["importVariables"], 50);
    assert_eq!(limits["exportMaxPixels"], 16_777_216);
    assert_eq!(limits["exportScales"], json!([0.5, 1.0, 2.0, 3.0, 4.0]));
    assert_eq!(
        limits["presets"][4],
        json!({"id": "phoneSmall", "name": "Phone small", "width": 375.0, "height": 667.0})
    );
    let back: AppState = serde_json::from_value(state).unwrap();
    assert_eq!(&back, app().state());
}
