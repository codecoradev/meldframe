//! Image fill picking (E2.5, ADR-0015 follow-up): the picker lists the assets of the workspace through
//! `Effect::FetchAssets` and `Action::AssetsLoaded`. The app layer holds the rules; a front end only draws the list and
//! dispatches. Choosing an image (`Action::SetImageFill`) is in `fills.rs`.

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{
    ASSET_LIST_MAX, ASSET_PAGE, ASSET_PAGE_MAX, Action, AssetInfo, AssetPage, Effect, Event,
};

fn id_of(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

fn info(n: u8) -> AssetInfo {
    AssetInfo {
        id: id_of(n),
        bytes: 100 + u64::from(n),
        mime: "image/png".into(),
    }
}

/// Asset number `n` of a long list (ids in the order of the numbers, as the server lists them).
fn info_n(n: u32) -> AssetInfo {
    AssetInfo {
        id: format!("{n:08x}").repeat(8),
        bytes: 100,
        mime: "image/png".into(),
    }
}

fn page(ns: &[u8], next: Option<u8>) -> AssetPage {
    AssetPage {
        assets: ns.iter().map(|n| info(*n)).collect(),
        next: next.map(id_of),
    }
}

/// A page as the shell answers the request that is out now (`AppState::assets.request`).
fn loaded(l: &Loopback, after: Option<u8>, p: AssetPage) -> Action {
    loaded_for(state(l).request, after, p)
}

fn loaded_for(request: u32, after: Option<u8>, p: AssetPage) -> Action {
    Action::AssetsLoaded {
        request,
        after: after.map(id_of),
        page: p,
    }
}

fn failed(l: &Loopback, code: &str) -> Action {
    Action::AssetsRequestFailed {
        request: state(l).request,
        code: code.into(),
    }
}

fn open_for(l: &mut Loopback, id: &str) -> Vec<Effect> {
    select(l, id);
    l.send(Action::OpenAssets { id: id.into() })
}

fn state(l: &Loopback) -> meldframe_app::AssetsState {
    l.app.state().assets.clone()
}

fn fetches(fx: &[Effect]) -> Vec<(Option<String>, u32)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::FetchAssets { after, limit, .. } => Some((after.clone(), *limit)),
            _ => None,
        })
        .collect()
}

#[test]
fn opening_the_picker_asks_for_the_first_page_and_marks_the_node_it_picks_for() {
    let mut l = Loopback::live();
    let fx = open_for(&mut l, "r1");
    assert_eq!(fetches(&fx), vec![(None, ASSET_PAGE)]);
    assert_eq!(ASSET_PAGE, 24);
    let s = state(&l);
    assert_eq!(s.target.as_deref(), Some("r1"));
    assert!((s.loading, s.loaded, s.items.len()) == (true, false, 0));
    assert_eq!(l.app.state().history.undo_steps, 0, "view state: no step");
    assert!(refused(&mut l).is_empty());
}

#[test]
fn the_first_page_fills_the_list_and_load_more_follows_the_cursor_once() {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    l.send(loaded(&l, None, page(&[1, 2], Some(2))));
    let s = state(&l);
    assert_eq!(
        s.items.iter().map(|i| i.id.clone()).collect::<Vec<_>>(),
        vec![id_of(1), id_of(2)]
    );
    assert_eq!(
        (s.loading, s.loaded, s.next.clone()),
        (false, true, Some(id_of(2)))
    );

    let fx = l.send(Action::LoadMoreAssets {});
    assert_eq!(fetches(&fx), vec![(Some(id_of(2)), ASSET_PAGE)]);
    assert!(state(&l).loading);
    // A second press while the request is out asks for nothing.
    assert!(fetches(&l.send(Action::LoadMoreAssets {})).is_empty());

    // The page repeats the last id it follows: it is listed once.
    l.send(loaded(&l, Some(2), page(&[2, 3], None)));
    let s = state(&l);
    assert_eq!(s.items.len(), 3);
    assert_eq!((s.loading, s.next), (false, None));
    // No cursor, nothing to ask.
    assert!(fetches(&l.send(Action::LoadMoreAssets {})).is_empty());
}

#[test]
fn a_page_that_does_not_follow_the_list_fails_the_request_and_the_picker_stays_usable() {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    l.send(loaded(&l, None, page(&[1, 2], Some(2))));
    l.send(Action::LoadMoreAssets {});
    // The answer asked after 9, which the list does not end with.
    l.send(loaded(&l, Some(9), page(&[10], None)));
    let s = state(&l);
    assert_eq!(
        (s.loading, s.error.as_deref(), s.items.len(), s.next.clone()),
        (false, Some("invalid"), 2, Some(id_of(2))),
        "not stuck loading: what was read and the cursor stay"
    );
    // Retry works: it asks after the cursor, and the page that follows is taken.
    let fx = l.send(Action::RetryAssets {});
    assert_eq!(fetches(&fx), vec![(Some(id_of(2)), ASSET_PAGE)]);
    assert!(state(&l).loading && state(&l).error.is_none());
    l.send(loaded(&l, Some(2), page(&[3], None)));
    let s = state(&l);
    assert_eq!((s.items.len(), s.loading, s.error), (3, false, None));
}

#[test]
fn a_page_the_server_could_not_have_sent_is_a_failed_request_with_the_code_invalid() {
    let one = |f: &dyn Fn(&mut AssetPage)| {
        let mut p = page(&[1, 2], Some(2));
        f(&mut p);
        p
    };
    let bad = [
        one(&|p| p.assets[0].id = "A".repeat(64)),
        one(&|p| p.assets[1].id = "../../etc/passwd".into()),
        one(&|p| p.assets[0].mime = "image/svg+xml".into()),
        one(&|p| p.assets[0].mime = "text/html".into()),
        one(&|p| p.assets[0].bytes = 1 << 53),
        one(&|p| p.next = Some("not an id".into())),
        one(&|p| p.next = Some(id_of(1))),
        one(&|p| p.assets.clear()),
        AssetPage {
            assets: (0..=ASSET_PAGE_MAX).map(|n| info(n as u8)).collect(),
            next: None,
        },
    ];
    for p in bad {
        let mut l = Loopback::live();
        open_for(&mut l, "r1");
        l.send(loaded(&l, None, p));
        let s = state(&l);
        assert_eq!(
            (s.error.as_deref(), s.loading, s.loaded, s.items.len()),
            (Some("invalid"), false, false, 0)
        );
        assert_eq!(retry(&mut l), vec![(None, ASSET_PAGE)], "asked for again");
    }
}

#[test]
fn the_list_stops_at_its_cap_and_says_so() {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    for p in 0..40 {
        let after = state(&l).items.last().map(|i| i.id.clone());
        let assets: Vec<AssetInfo> = (0..ASSET_PAGE)
            .map(|k| info_n(p * ASSET_PAGE + k))
            .collect();
        let next = assets.last().map(|a| a.id.clone());
        let request = state(&l).request;
        let page = AssetPage { assets, next };
        l.send(Action::AssetsLoaded {
            request,
            after,
            page,
        });
        if state(&l).next.is_none() {
            break;
        }
        l.send(Action::LoadMoreAssets {});
    }
    let s = state(&l);
    assert_eq!(
        (s.items.len(), s.capped, s.next),
        (ASSET_LIST_MAX, true, None)
    );
    assert!(retry(&mut l).is_empty() && fetches(&l.send(Action::LoadMoreAssets {})).is_empty());
    // A list that ends before the cap is not capped, and opening again starts over.
    open_for(&mut l, "r1");
    assert!(!state(&l).capped);
    l.send(loaded(&l, None, page(&[1], None)));
    assert!(!state(&l).capped);
}

/// What `RetryAssets` asks for.
fn retry(l: &mut Loopback) -> Vec<(Option<String>, u32)> {
    fetches(&l.send(Action::RetryAssets {}))
}

#[test]
fn retry_asks_for_the_first_page_when_nothing_was_read_and_for_the_next_one_otherwise() {
    let mut l = Loopback::live();
    assert!(retry(&mut l).is_empty(), "never opened");
    open_for(&mut l, "r1");
    assert!(retry(&mut l).is_empty(), "a request is out");
    l.send(failed(&l, "unavailable"));
    assert_eq!(retry(&mut l), vec![(None, ASSET_PAGE)]);
    assert!(retry(&mut l).is_empty(), "the retry is out");
    l.send(loaded(&l, None, page(&[1], Some(1))));
    assert!(retry(&mut l).is_empty(), "nothing failed");
    l.send(Action::LoadMoreAssets {});
    l.send(failed(&l, "rate_limited"));
    assert_eq!(retry(&mut l), vec![(Some(id_of(1)), ASSET_PAGE)]);
    l.send(Action::CloseAssets {});
    assert!(retry(&mut l).is_empty(), "closed");
}

#[test]
fn closing_forgets_the_list_and_opening_again_reads_from_the_start() {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    l.send(loaded(&l, None, page(&[1, 2], Some(2))));
    l.send(Action::CloseAssets {});
    let s = state(&l);
    assert_eq!(
        (s.target, s.items.len(), s.next, s.loaded, s.loading),
        (None, 0, None, false, false),
        "a closed picker holds no list, so a snapshot does not carry it"
    );
    let fx = l.send(Action::OpenAssets { id: "r1".into() });
    assert_eq!(fetches(&fx), vec![(None, ASSET_PAGE)]);
    let s = state(&l);
    assert_eq!(
        (s.items.len(), s.loaded, s.loading, s.next),
        (0, false, true, None)
    );
    // An agent uploaded another image meanwhile: the first page of the new request is the list.
    l.send(loaded(&l, None, page(&[0, 1, 2], None)));
    assert_eq!(state(&l).items.len(), 3);
}

#[test]
fn an_answer_that_comes_after_the_picker_closed_is_ignored() {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    let late = state(&l).request;
    l.send(Action::CloseAssets {});
    l.send(loaded_for(late, None, page(&[1, 2], Some(2))));
    let s = state(&l);
    assert_eq!(
        (s.items.len(), s.loaded, s.loading, s.next),
        (0, false, false, None)
    );
    l.send(Action::AssetsRequestFailed {
        request: late,
        code: "unavailable".into(),
    });
    assert!(state(&l).error.is_none(), "a late failure is ignored too");
}

#[test]
fn an_answer_of_a_request_made_before_the_picker_opened_again_is_ignored_and_only_one_page_is_taken()
 {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    let first = state(&l).request;
    l.send(Action::CloseAssets {});
    let fx = l.send(Action::OpenAssets { id: "r1".into() });
    let second = state(&l).request;
    assert_ne!(first, second);
    assert_eq!(fetches(&fx).len(), 1);
    // The reply of the first request comes now: it is not the one out.
    l.send(loaded_for(first, None, page(&[9], None)));
    l.send(Action::AssetsRequestFailed {
        request: first,
        code: "unavailable".into(),
    });
    let s = state(&l);
    assert_eq!(
        (s.items.len(), s.loading, s.loaded, s.error),
        (0, true, false, None)
    );
    // Opening again while the request is out makes a new one and drops the older.
    let fx = l.send(Action::OpenAssets { id: "r1".into() });
    assert_eq!(fetches(&fx).len(), 1);
    l.send(loaded_for(second, None, page(&[1], None)));
    assert!(
        state(&l).items.is_empty(),
        "the answer of the superseded request"
    );
    l.send(loaded(&l, None, page(&[2], None)));
    assert_eq!(state(&l).items[0].id, id_of(2));
    // A page is taken once: the same answer again changes nothing.
    let n = state(&l).request;
    l.send(loaded_for(n, None, page(&[3], None)));
    assert_eq!(state(&l).items.len(), 1);
}

#[test]
fn load_more_does_nothing_while_the_picker_is_closed_or_a_request_is_out() {
    let mut l = Loopback::live();
    assert!(
        fetches(&l.send(Action::LoadMoreAssets {})).is_empty(),
        "never opened"
    );
    open_for(&mut l, "r1");
    l.send(loaded(&l, None, page(&[1], Some(1))));
    l.send(Action::CloseAssets {});
    assert!(
        fetches(&l.send(Action::LoadMoreAssets {})).is_empty(),
        "closed"
    );
    open_for(&mut l, "r1");
    l.send(loaded(&l, None, page(&[1], Some(1))));
    assert_eq!(fetches(&l.send(Action::LoadMoreAssets {})).len(), 1);
    let out = state(&l).request;
    assert!(
        fetches(&l.send(Action::LoadMoreAssets {})).is_empty(),
        "a request is out"
    );
    assert_eq!(state(&l).request, out, "no second request was numbered");
}

#[test]
fn a_failed_fetch_keeps_what_was_read_and_the_cursor_and_the_next_try_clears_the_code() {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    l.send(loaded(&l, None, page(&[1], Some(1))));
    l.send(Action::LoadMoreAssets {});
    l.send(failed(&l, "unavailable"));
    let s = state(&l);
    assert_eq!(
        (s.loading, s.error.as_deref(), s.items.len(), s.next.clone()),
        (false, Some("unavailable"), 1, Some(id_of(1)))
    );
    let fx = l.send(Action::LoadMoreAssets {});
    assert_eq!(fetches(&fx).len(), 1, "the person can try again");
    assert!(state(&l).error.is_none());
    l.send(loaded(&l, Some(1), page(&[2], None)));
    assert_eq!(state(&l).items.len(), 2);
}

#[test]
fn the_picker_opens_for_the_selected_node_only_and_not_for_text() {
    let mut l = Loopback::live();
    assert!(fetches(&l.send(Action::OpenAssets { id: "nope".into() })).is_empty());
    select(&mut l, "r1");
    assert!(fetches(&l.send(Action::OpenAssets { id: "root".into() })).is_empty());
    select(&mut l, "t1");
    assert!(fetches(&l.send(Action::OpenAssets { id: "t1".into() })).is_empty());
    assert_eq!(
        refused(&mut l),
        vec!["not_found", "not_selected", "image_on_text"]
    );
    assert!(state(&l).target.is_none() && !state(&l).loading);
}

#[test]
fn the_picker_closes_when_the_selection_moves_off_its_node() {
    let mut l = Loopback::live();
    open_for(&mut l, "r1");
    assert!(state(&l).target.is_some());
    select(&mut l, "root");
    assert!(state(&l).target.is_none());
    open_for(&mut l, "r1");
    l.send(Action::Delete {
        ids: vec!["r1".into()],
    });
    assert!(
        state(&l).target.is_none(),
        "a node that is gone has no picker"
    );
}

#[test]
fn the_wire_shapes_are_camel_case_and_strict() {
    let a: Action = serde_json::from_value(serde_json::json!({"type": "assetsLoaded", "request": 3, "page": {"assets": [{"id": id_of(1), "bytes": 5, "mime": "image/png"}], "next": null}, "after": null})).unwrap();
    assert!(matches!(a, Action::AssetsLoaded { after: None, .. }));
    assert!(
        serde_json::from_value::<Action>(
            serde_json::json!({"type": "openAssets", "id": "r1", "extra": 1})
        )
        .is_err()
    );
    let fx = serde_json::to_value(Effect::FetchAssets {
        after: None,
        limit: 50,
        request: 3,
    })
    .unwrap();
    assert_eq!(
        fx,
        serde_json::json!({"type": "fetchAssets", "after": null, "limit": 50, "request": 3})
    );
}

/// The front end redraws from events (a dispatch without one leaves its snapshot as it is), so every change of
/// `AppState::assets` raises `AssetsChanged`, and a dispatch that changes nothing raises none.
fn changes(app: &mut meldframe_app::App, a: Action) -> usize {
    app.dispatch(a);
    app.take_events()
        .iter()
        .filter(|e| matches!(e, Event::AssetsChanged {}))
        .count()
}

#[test]
fn every_change_of_the_picker_state_raises_assets_changed_and_a_dispatch_that_changes_nothing_raises_none()
 {
    let mut l = Loopback::live();
    select(&mut l, "r1");
    let app = &mut l.app;
    app.take_events();
    let open = Action::OpenAssets { id: "r1".into() };
    assert_eq!(changes(app, open.clone()), 1, "opening");
    assert_eq!(
        changes(app, Action::LoadMoreAssets {}),
        0,
        "nothing to load more while the first page is out"
    );
    let n = app.state().assets.request;
    assert_eq!(
        changes(app, loaded_for(n, None, page(&[1], Some(1)))),
        1,
        "a page"
    );
    assert_eq!(
        changes(app, loaded_for(n, None, page(&[1], Some(1)))),
        0,
        "the same answer again"
    );
    assert_eq!(changes(app, Action::LoadMoreAssets {}), 1, "load more");
    let n = app.state().assets.request;
    assert_eq!(
        changes(app, loaded_for(n, Some(9), page(&[2], None))),
        1,
        "a page that does not follow fails the request"
    );
    let failure = |request| Action::AssetsRequestFailed {
        request,
        code: "unavailable".into(),
    };
    assert_eq!(changes(app, failure(n)), 0, "the request has ended");
    assert_eq!(changes(app, Action::RetryAssets {}), 1, "a retry");
    let n = app.state().assets.request;
    assert_eq!(
        changes(app, failure(n - 1)),
        0,
        "a failure of an older request"
    );
    assert_eq!(changes(app, failure(n)), 1, "a failure");
    assert_eq!(changes(app, Action::CloseAssets {}), 1, "closing");
    assert_eq!(
        changes(app, Action::CloseAssets {}),
        0,
        "closing what is closed"
    );
    // A refused opening of a closed picker changes nothing; one that closes an open picker does.
    assert_eq!(changes(app, Action::OpenAssets { id: "nope".into() }), 0);
    changes(app, open);
    assert_eq!(changes(app, Action::OpenAssets { id: "nope".into() }), 1);
    // The selection moving off the node closes it.
    changes(app, Action::OpenAssets { id: "r1".into() });
    assert_eq!(
        changes(
            app,
            Action::Select {
                ids: ids(&["root"])
            }
        ),
        1,
        "the selection moved"
    );
}
