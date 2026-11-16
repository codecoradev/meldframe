//! The touch hint is an ephemeral frame of a live stream: who touched which nodes, at which sequence number.
//! It carries no document content.

use meldframe_protocol::ServerMsg;

#[test]
fn a_touch_frame_has_the_documented_wire_shape() {
    let m = ServerMsg::Touch {
        actor: "agent:a".into(),
        node_ids: vec!["n1".into(), "n2".into()],
        seq: 9,
    };
    assert_eq!(
        serde_json::to_string(&m).unwrap(),
        r#"{"type":"touch","actor":"agent:a","nodeIds":["n1","n2"],"seq":9}"#
    );
    let back: ServerMsg =
        serde_json::from_str(r#"{"type":"touch","actor":"agent:a","nodeIds":["n1"],"seq":9}"#)
            .unwrap();
    assert!(matches!(back, ServerMsg::Touch { seq: 9, .. }));
    assert!(
        serde_json::from_str::<ServerMsg>(
            r#"{"type":"touch","actor":"a","nodeIds":[],"seq":1,"x":1}"#
        )
        .is_err()
    );
}
