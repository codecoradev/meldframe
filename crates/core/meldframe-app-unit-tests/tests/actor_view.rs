//! One actor view (#320, finding T2 of the thin-UI audit): the kind, the name and the tone an actor wears are built once
//! in the app layer and carried by the layer row, the lane, the touch ring and the checkpoint, so one actor has one
//! colour everywhere. The snapshot's attribution (who changed each node before the page loaded) enters the same way.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Event, Frame};
use meldframe_app_model::{ActorTone, ActorView, CheckpointKind, CheckpointRow, LaneKind};
use meldframe_protocol::{Command, NodePatch};
use std::collections::BTreeMap;

const T: u64 = 1_700_000_000_000;

fn paint(id: &str, x: f64) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            x: Some(x),
            ..Default::default()
        },
    }
}

fn touch(l: &mut Loopback, actor: &str) {
    let frame = Frame::Touch {
        actor: actor.into(),
        ids: ids(&["r1"]),
        seq: 1,
    };
    l.app.dispatch(Action::Frame { now_ms: T, frame });
}

fn layer_actor(l: &Loopback, id: &str) -> Option<ActorView> {
    let rows = &l.app.state().layers;
    rows.iter().find(|r| r.id == id).unwrap().actor.clone()
}

#[test]
fn actor_view_is_the_same_for_lane_row_and_ring() {
    // A character outside the BMP: the web client's two derivations gave this actor two colours.
    let actor = "agent:\u{1F300}";
    let mut l = Loopback::live();
    l.remote(actor, "b1:0", T, paint("r1", 5.0));
    touch(&mut l, actor);
    let state = l.app.state();
    let row = layer_actor(&l, "r1").expect("the row names the actor");
    let lane = state
        .activity
        .lanes
        .iter()
        .find(|l| l.actor == actor)
        .unwrap();
    let ring = state
        .activity
        .touches
        .iter()
        .find(|t| t.actor == actor)
        .unwrap();
    assert_eq!(row, ActorView::of(actor));
    assert_eq!(lane.view, row);
    assert_eq!(ring.view, row);
    assert_eq!(
        (row.kind, row.name.as_str(), row.tone),
        (LaneKind::Agent, "\u{1F300}", ActorTone::Agent3)
    );
}

#[test]
fn a_person_wears_the_person_tone_on_every_surface() {
    let mut l = Loopback::live();
    l.remote("person:ana", "b1:0", T, paint("r1", 5.0));
    touch(&mut l, "person:ana");
    let state = l.app.state();
    let row = layer_actor(&l, "r1").unwrap();
    assert_eq!(
        (row.kind, row.name.as_str(), row.tone),
        (LaneKind::Person, "ana", ActorTone::Person)
    );
    let lane = state
        .activity
        .lanes
        .iter()
        .find(|l| l.actor == "person:ana")
        .unwrap();
    let ring = state
        .activity
        .touches
        .iter()
        .find(|t| t.actor == "person:ana")
        .unwrap();
    assert_eq!((&lane.view, &ring.view), (&row, &row));
}

#[test]
fn the_name_of_a_hostile_actor_is_clean_on_every_surface() {
    // The actor is made safe where the frame enters (rule U9); the view is built from what is left.
    let mut l = Loopback::live();
    l.remote("agent:ev\u{202E}i\u{200B}l", "b1:0", T, paint("r1", 5.0));
    let row = layer_actor(&l, "r1").unwrap();
    assert_eq!(
        (row.actor.as_str(), row.name.as_str()),
        ("agent:evil", "evil")
    );
    let lane = &l.app.state().activity.lanes[0];
    assert_eq!(lane.view, row);
}

#[test]
fn a_checkpoint_names_its_actors_with_the_same_view() {
    let mut l = Loopback::live();
    let rows = vec![
        CheckpointRow {
            n: 1,
            seq: 1,
            kind: CheckpointKind::RunStart,
            label: None,
            about: Some("agent:\u{1F300}".into()),
            by: None,
            at_ms: 1,
        },
        CheckpointRow {
            n: 2,
            seq: 2,
            kind: CheckpointKind::Manual,
            label: Some("v1".into()),
            about: None,
            by: Some("person:ana".into()),
            at_ms: 2,
        },
    ];
    l.send(Action::CheckpointsLoaded {
        checkpoints: rows,
        can_create: true,
    });
    let list = &l.app.state().checkpoints;
    let run = list.iter().find(|c| c.n == 1).unwrap();
    let manual = list.iter().find(|c| c.n == 2).unwrap();
    assert_eq!(run.about, Some(ActorView::of("agent:\u{1F300}")));
    assert_eq!(manual.by, Some(ActorView::of("person:ana")));
    assert_eq!(run.by, None);
}

fn attribute(l: &mut Loopback, pairs: &[(&str, &str)]) {
    let actors: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    l.app.dispatch(Action::Attribute { actors });
    l.events.extend(l.app.take_events());
}

#[test]
fn the_snapshot_attribution_names_who_changed_a_node_before_the_page_loaded() {
    let mut l = Loopback::live();
    assert_eq!(layer_actor(&l, "r1"), None);
    attribute(&mut l, &[("r1", "agent:claude"), ("t1", "person:ana")]);
    assert_eq!(layer_actor(&l, "r1"), Some(ActorView::of("agent:claude")));
    assert_eq!(layer_actor(&l, "t1"), Some(ActorView::of("person:ana")));
    assert!(
        l.events
            .iter()
            .any(|e| matches!(e, Event::AttributionChanged {}))
    );
    assert_eq!(l.app.state().last_actor["r1"], "agent:claude");
}

#[test]
fn the_snapshot_attribution_is_made_safe_like_any_actor_and_skips_what_is_not_a_node() {
    let mut l = Loopback::live();
    attribute(
        &mut l,
        &[
            ("r1", "agent:x\u{202E}\u{200B}"),
            // Nothing is left of this one: attributed to nobody.
            ("t1", "\u{200B}\u{2800}"),
            ("gone", "agent:y"),
        ],
    );
    assert_eq!(l.app.state().last_actor.len(), 1);
    let row = layer_actor(&l, "r1").unwrap();
    assert_eq!((row.actor.as_str(), row.name.as_str()), ("agent:x", "x"));
    assert_eq!(layer_actor(&l, "t1"), None);
}

#[test]
fn the_snapshot_attribution_never_overrides_a_change_seen_live() {
    let mut l = Loopback::live();
    l.remote("agent:live", "b1:0", T, paint("r1", 5.0));
    l.events.clear();
    attribute(&mut l, &[("r1", "agent:old")]);
    assert_eq!(l.app.state().last_actor["r1"], "agent:live");
    assert!(
        !l.events
            .iter()
            .any(|e| matches!(e, Event::AttributionChanged {})),
        "nothing changed, so nothing is said"
    );
    // And a later live change still wins over the attribution that was taken.
    attribute(&mut l, &[("t1", "agent:old")]);
    l.remote("agent:live", "b1:1", T, paint("t1", 6.0));
    assert_eq!(l.app.state().last_actor["t1"], "agent:live");
}

#[test]
fn the_attribution_of_a_removed_node_goes_with_it() {
    let mut l = Loopback::live();
    attribute(&mut l, &[("r1", "agent:claude")]);
    l.send(Action::Delete { ids: ids(&["r1"]) });
    assert!(!l.app.state().last_actor.contains_key("r1"));
}
