//! The batch frame is an ephemeral frame of a live stream: the header of a numbered batch as the server read it back,
//! with the count of headers that are not accepted (ADR-0021).

use meldframe_protocol::ServerMsg;

#[test]
fn a_batch_frame_has_the_documented_wire_shape() {
    let m = ServerMsg::Batch {
        room: "f".into(),
        n: 2,
        rev: 5,
        actor: "agent:a".into(),
        key_prefix: "k1".into(),
        tool: "apply_commands".into(),
        commands: 3,
        written: 2,
        atomic: false,
        first_seq: 7,
        last_seq: 8,
        at_ms: 1000,
        note: Some("why".into()),
        review: "unreviewed".into(),
        reviewed_by: None,
        reviewed_at_ms: None,
        unreviewed_count: 4,
    };
    let text = r#"{"type":"batch","room":"f","n":2,"rev":5,"actor":"agent:a","keyPrefix":"k1","tool":"apply_commands","commands":3,"written":2,"atomic":false,"firstSeq":7,"lastSeq":8,"atMs":1000,"note":"why","review":"unreviewed","unreviewedCount":4}"#;
    assert_eq!(serde_json::to_string(&m).unwrap(), text);
    assert!(matches!(
        serde_json::from_str::<ServerMsg>(text).unwrap(),
        ServerMsg::Batch { n: 2, rev: 5, .. }
    ));
    let extra = text.replace("\"n\":2", "\"n\":2,\"x\":1");
    assert!(serde_json::from_str::<ServerMsg>(&extra).is_err());
}
