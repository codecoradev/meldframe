//! Frame presets (Desktop, Desktop HD, Tablet, Mobile, Phone small, A4 and Letter) make a frame of the preset's size and name, select it, return to the
//! select tool and undo as one step.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Event, FramePreset, Tool};
use meldframe_protocol::NodeKind;

fn preset(p: FramePreset, x: f64, y: f64) -> Action {
    Action::CreatePreset {
        preset: p,
        x,
        y,
        parent: Some("root".into()),
    }
}

#[test]
fn a_preset_makes_a_frame_of_its_size_and_name() {
    for (p, w, h, name) in [
        (FramePreset::Desktop, 1440.0, 900.0, "Desktop"),
        (FramePreset::DesktopHd, 1920.0, 1080.0, "Desktop HD"),
        (FramePreset::Tablet, 768.0, 1024.0, "Tablet"),
        (FramePreset::Mobile, 390.0, 844.0, "Mobile"),
        (FramePreset::PhoneSmall, 375.0, 667.0, "Phone small"),
        (FramePreset::A4, 595.0, 842.0, "A4"),
        (FramePreset::Letter, 612.0, 792.0, "Letter"),
    ] {
        let mut l = Loopback::live();
        l.send(preset(p, 12.0, 34.0));
        let id = l.app.state().selection[0].clone();
        let n = get(&l.app, &id);
        assert_eq!(
            (n.kind, n.x, n.y, n.w, n.h, n.name.as_str()),
            (NodeKind::Frame, 12.0, 34.0, w, h, name)
        );
        assert_eq!(n.parent_id.as_deref(), Some("root"));
        assert_eq!(p.size(), (w, h));
        assert_eq!(p.name(), name);
    }
}

#[test]
fn the_menu_lists_every_preset_once_in_the_order_of_the_web_client() {
    let names: Vec<&str> = FramePreset::ALL.iter().map(|p| p.name()).collect();
    assert_eq!(
        names,
        [
            "Desktop",
            "Desktop HD",
            "Tablet",
            "Mobile",
            "Phone small",
            "A4",
            "Letter"
        ]
    );
}

#[test]
fn it_selects_the_frame_returns_to_the_select_tool_and_is_one_undo_step() {
    let mut l = Loopback::live();
    let before = l.app.nodes().len();
    l.app.dispatch(Action::SetTool { tool: Tool::Frame });
    l.send(preset(FramePreset::Mobile, 0.0, 0.0));
    assert_eq!(l.app.state().tool, Tool::Select);
    assert_eq!(l.app.state().selection.len(), 1);
    assert_eq!(l.app.nodes().len(), before + 1);
    l.send(Action::Undo {});
    assert_eq!(l.app.nodes().len(), before);
}

#[test]
fn a_position_that_is_not_a_number_is_refused_and_nothing_is_made() {
    let mut l = Loopback::live();
    l.app.take_events();
    let before = l.app.nodes().len();
    l.app.dispatch(preset(FramePreset::Desktop, f64::NAN, 0.0));
    assert_eq!(l.app.nodes().len(), before);
    assert!(
        l.app
            .take_events()
            .iter()
            .any(|e| matches!(e, Event::Refused { .. }))
    );
}

#[test]
fn a_preset_in_a_parent_that_holds_no_children_is_refused() {
    let mut l = Loopback::live();
    l.app.take_events();
    let before = l.app.nodes().len();
    l.app.dispatch(Action::CreatePreset {
        preset: FramePreset::Tablet,
        x: 0.0,
        y: 0.0,
        parent: Some("r1".into()),
    });
    assert_eq!(l.app.nodes().len(), before);
    assert_eq!(
        l.app.take_events(),
        vec![Event::Refused {
            code: "bad_parent".into()
        }]
    );
}

#[test]
fn the_wire_form_names_the_preset_in_lower_case() {
    let a: Action = serde_json::from_str(
        r#"{"type":"createPreset","preset":"tablet","x":1,"y":2,"parent":null}"#,
    )
    .unwrap();
    assert!(matches!(
        a,
        Action::CreatePreset {
            preset: FramePreset::Tablet,
            parent: None,
            ..
        }
    ));
    for (wire, p) in [
        ("desktopHd", FramePreset::DesktopHd),
        ("phoneSmall", FramePreset::PhoneSmall),
        ("a4", FramePreset::A4),
        ("letter", FramePreset::Letter),
    ] {
        let json =
            format!(r#"{{"type":"createPreset","preset":"{wire}","x":0,"y":0,"parent":null}}"#);
        let a: Action = serde_json::from_str(&json).unwrap();
        assert!(
            matches!(a, Action::CreatePreset { preset, .. } if preset == p),
            "{wire}"
        );
    }
    let back = serde_json::to_string(&Action::CreatePreset {
        preset: FramePreset::Mobile,
        x: 1.0,
        y: 2.0,
        parent: None,
    })
    .unwrap();
    assert!(back.contains(r#""preset":"mobile""#), "{back}");
}
