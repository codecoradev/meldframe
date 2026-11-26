//! The tab of the right card is state of the app layer (audit finding T10, issue 323): choosing Activity asks for the newest
//! batches and the checkpoints once, a tab that is selected already asks for nothing, and the tab changes while the
//! connection is down or halted but asks for nothing then. The server is scripted (`scripted/mod.rs`).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
mod scripted;
use common::*;
use meldframe_app::{Action, ActivityOp, Connection, Effect, Event, RightTab};
use scripted::*;

fn op(op: ActivityOp) -> Action {
    Action::Activity { op }
}

#[test]
fn activating_the_activity_tab_requests_batches_once() {
    let mut c = Cp::live();
    c.l.events.clear();
    let fetches = |fx: &[Effect]| {
        fx.iter()
            .filter(|e| matches!(e, Effect::FetchBatches { .. }))
            .count()
    };
    let fx = c.l.send(op(ActivityOp::SetTab {
        tab: RightTab::Activity,
    }));
    assert_eq!(c.l.app.state().right_tab, RightTab::Activity);
    assert_eq!(fetches(&fx), 1, "the newest batches are asked for");
    assert!(
        fx.iter().any(|e| matches!(e, Effect::FetchCheckpoints {})),
        "and the checkpoints"
    );
    assert!(c.l.events.contains(&Event::RightTabChanged {
        tab: RightTab::Activity
    }));
    let fx = c.l.send(op(ActivityOp::SetTab {
        tab: RightTab::Activity,
    }));
    assert_eq!(
        fetches(&fx),
        0,
        "a second click on the tab that is selected asks for nothing"
    );
    let fx = c.l.send(op(ActivityOp::SetTab {
        tab: RightTab::Design,
    }));
    assert_eq!(
        (fetches(&fx), c.l.app.state().right_tab),
        (0, RightTab::Design)
    );
    let fx = c.l.send(op(ActivityOp::SetTab {
        tab: RightTab::Activity,
    }));
    assert_eq!(fetches(&fx), 1, "coming back asks again");
}

#[test]
fn the_tab_changes_while_the_connection_is_down_and_asks_for_nothing() {
    let mut a = meldframe_app::App::new("user:me", "c0", 1, doc(), 0).unwrap();
    let fx = a.dispatch(op(ActivityOp::SetTab {
        tab: RightTab::Activity,
    }));
    assert_eq!(a.state().right_tab, RightTab::Activity);
    assert!(fx.is_empty());
}

#[test]
fn a_halted_app_changes_the_tab_and_asks_for_nothing() {
    let mut c = Cp::live();
    // An op the file cannot take diverges the replica: the app halts and takes only what changes the view.
    c.l.remote("agent:x", "b1:0", 0, set("ghost", |p| p.x = Some(1.0)));
    assert_eq!(c.l.app.state().connection, Connection::Halted);
    c.l.events.clear();
    let fx = c.send(op(ActivityOp::SetTab {
        tab: RightTab::Activity,
    }));
    assert_eq!(
        c.l.app.state().right_tab,
        RightTab::Activity,
        "the tab is a view"
    );
    assert!(fx.is_empty(), "and asks for nothing while halted");
}
