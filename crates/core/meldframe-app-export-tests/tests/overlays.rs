//! The open state of the shortcut sheet and of the command palette is state of the app layer (audit finding T11, issue 324):
//! one overlay is open at a time, the palette stays shut while a dialog is open (the rule that needed a scan of the page
//! for dialogs), and the dialogs a person opens by a button are open by the state and not by a flag of the page.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, BindField, Event, ManageOp, Overlay, OverlayOp};

fn overlay(l: &Loopback) -> Option<Overlay> {
    l.app.state().overlay
}

fn toggle(l: &mut Loopback, overlay: Overlay) {
    l.send(Action::Overlay {
        op: OverlayOp::Toggle { overlay },
    });
}

fn changes(l: &mut Loopback) -> Vec<Option<Overlay>> {
    l.events
        .drain(..)
        .filter_map(|e| match e {
            Event::OverlayChanged { overlay } => Some(overlay),
            _ => None,
        })
        .collect()
}

#[test]
fn no_overlay_is_open_at_first_and_a_toggle_opens_and_closes_one() {
    let mut l = Loopback::live();
    assert_eq!(overlay(&l), None);
    toggle(&mut l, Overlay::Help);
    assert_eq!(overlay(&l), Some(Overlay::Help));
    toggle(&mut l, Overlay::Help);
    assert_eq!(overlay(&l), None);
    assert_eq!(changes(&mut l), vec![Some(Overlay::Help), None]);
}

#[test]
fn one_overlay_is_open_at_a_time() {
    let mut l = Loopback::live();
    toggle(&mut l, Overlay::Help);
    // The palette replaces the sheet, and the sheet replaces the palette.
    toggle(&mut l, Overlay::Palette);
    assert_eq!(overlay(&l), Some(Overlay::Palette));
    toggle(&mut l, Overlay::Help);
    assert_eq!(overlay(&l), Some(Overlay::Help));
    l.send(Action::Overlay {
        op: OverlayOp::Open {
            overlay: Overlay::Palette,
        },
    });
    assert_eq!(overlay(&l), Some(Overlay::Palette));
    assert_eq!(
        changes(&mut l),
        vec![
            Some(Overlay::Help),
            Some(Overlay::Palette),
            Some(Overlay::Help),
            Some(Overlay::Palette)
        ]
    );
}

#[test]
fn closing_closes_the_overlay_that_is_open_and_nothing_else() {
    let mut l = Loopback::live();
    toggle(&mut l, Overlay::Palette);
    l.events.clear();
    // The sheet is not the one open: a stale close of it leaves the palette as it is, and says nothing.
    l.send(Action::Overlay {
        op: OverlayOp::Close {
            overlay: Overlay::Help,
        },
    });
    assert_eq!(overlay(&l), Some(Overlay::Palette));
    assert!(changes(&mut l).is_empty());
    l.send(Action::Overlay {
        op: OverlayOp::Close {
            overlay: Overlay::Palette,
        },
    });
    assert_eq!(overlay(&l), None);
    assert_eq!(changes(&mut l), vec![None]);
}

#[test]
fn opening_what_is_open_says_nothing() {
    let mut l = Loopback::live();
    let open = || Action::Overlay {
        op: OverlayOp::Open {
            overlay: Overlay::Help,
        },
    };
    l.send(open());
    l.events.clear();
    l.send(open());
    assert_eq!(overlay(&l), Some(Overlay::Help));
    assert!(changes(&mut l).is_empty());
}

#[test]
fn ctrl_k_is_ignored_while_the_export_dialog_is_open() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    assert!(l.app.state().export.dialog.is_some());
    l.events.clear();
    toggle(&mut l, Overlay::Palette);
    assert_eq!(overlay(&l), None, "two overlays would stack");
    assert!(changes(&mut l).is_empty());
    l.send(Action::CloseExport {});
    toggle(&mut l, Overlay::Palette);
    assert_eq!(overlay(&l), Some(Overlay::Palette));
}

#[test]
fn every_dialog_and_the_bind_picker_keep_the_palette_shut_and_the_sheet_is_not_in_the_way() {
    // Each row opens one and closes it again; `blocked` says it is open (a front end reads these).
    type Row = (&'static str, Vec<Action>, Action);
    let manage = |op| Action::Manage { op };
    let rows: Vec<Row> = vec![
        (
            "export variables",
            vec![Action::OpenVariablesExport {}],
            Action::CloseVariablesExport {},
        ),
        (
            "import variables",
            vec![Action::OpenVariablesImport {}],
            Action::CloseVariablesImport {},
        ),
        (
            "manage",
            vec![manage(ManageOp::Open {})],
            manage(ManageOp::Close {}),
        ),
        (
            "bind picker",
            vec![
                Action::Select { ids: ids(&["r1"]) },
                Action::OpenPicker {
                    id: "r1".into(),
                    field: BindField::Fill,
                },
            ],
            Action::ClosePicker {},
        ),
    ];
    for (name, opening, closing) in rows {
        let mut l = Loopback::live();
        for a in opening {
            l.send(a);
        }
        toggle(&mut l, Overlay::Palette);
        assert_eq!(overlay(&l), None, "{name}: the palette stays shut");
        l.send(Action::Overlay {
            op: OverlayOp::Open {
                overlay: Overlay::Palette,
            },
        });
        assert_eq!(overlay(&l), None, "{name}: an open is refused too");
        toggle(&mut l, Overlay::Help);
        assert_eq!(overlay(&l), Some(Overlay::Help), "{name}: the sheet opens");
        l.send(closing);
        toggle(&mut l, Overlay::Palette);
        assert_eq!(overlay(&l), Some(Overlay::Palette), "{name}: shut again");
    }
}

#[test]
fn opening_a_dialog_closes_the_palette_and_leaves_the_sheet() {
    // The other half of "never two at once": a dialog that opens over an open palette closes it (the palette closes itself first when
    // a command of its own opens the dialog, so a person does not see this; another front end, or an agent, might).
    let manage = |op| Action::Manage { op };
    let rows: Vec<(&str, Vec<Action>)> = vec![
        (
            "export",
            vec![
                Action::Select {
                    ids: ids(&["root"]),
                },
                Action::OpenExport {},
            ],
        ),
        ("export variables", vec![Action::OpenVariablesExport {}]),
        ("import variables", vec![Action::OpenVariablesImport {}]),
        ("manage", vec![manage(ManageOp::Open {})]),
        (
            "bind picker",
            vec![
                Action::Select { ids: ids(&["r1"]) },
                Action::OpenPicker {
                    id: "r1".into(),
                    field: BindField::Fill,
                },
            ],
        ),
    ];
    for (name, opening) in rows {
        let mut l = Loopback::live();
        // The selection does not matter to the palette: the actions that select run first, and then it opens.
        for a in &opening[..opening.len() - 1] {
            l.send(a.clone());
        }
        toggle(&mut l, Overlay::Palette);
        l.events.clear();
        l.send(opening.last().unwrap().clone());
        assert_eq!(overlay(&l), None, "{name}: the palette closes");
        assert_eq!(changes(&mut l), vec![None], "{name}: and says so once");
        // The sheet is allowed beside a dialog, on purpose.
        toggle(&mut l, Overlay::Help);
        assert_eq!(overlay(&l), Some(Overlay::Help), "{name}");
    }
}

#[test]
fn a_dialog_that_is_refused_leaves_the_palette_open() {
    let mut l = Loopback::live();
    toggle(&mut l, Overlay::Palette);
    // Nothing is selected, so there is no frame to export.
    l.send(Action::OpenExport {});
    assert_eq!(overlay(&l), Some(Overlay::Palette));
}

#[test]
fn an_overlay_opens_and_closes_while_the_connection_is_halted() {
    let mut l = Loopback::live();
    // An op the file cannot take diverges the replica: the app halts and takes only what changes the view.
    l.remote("agent:x", "b1:0", 0, set_x("ghost"));
    assert_eq!(l.app.state().connection, meldframe_app::Connection::Halted);
    toggle(&mut l, Overlay::Palette);
    assert_eq!(overlay(&l), Some(Overlay::Palette));
    toggle(&mut l, Overlay::Palette);
    assert_eq!(overlay(&l), None);
}

fn set_x(id: &str) -> meldframe_protocol::Command {
    meldframe_protocol::Command::UpdateNode {
        id: id.into(),
        patch: meldframe_protocol::NodePatch {
            x: Some(1.0),
            ..Default::default()
        },
    }
}
