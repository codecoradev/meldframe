//! Wire compatibility: what an older client may omit stays valid, and anything unknown is refused.

use meldframe_protocol::{
    ClientMsg, Command, ImageFill, ImageFit, MAX_OP_DEPS, Node, NodeKind, NodePatch,
    SCHEMA_VERSION, ServerMsg, SizeMode,
};

const OP_MIN: &str = r#"{"type":"op","id":"m","idempotencyKey":"k","room":"r","command":{"type":"removeNode","id":"a"}}"#;

#[test]
fn accepts_valid_command() {
    let c: Command =
        serde_json::from_str(r#"{"type":"updateNode","id":"n1","patch":{"x":10,"sw":"fill"}}"#)
            .unwrap();
    match c {
        Command::UpdateNode { id, patch } => {
            assert_eq!(id, "n1");
            assert_eq!(patch.x, Some(10.0));
            assert_eq!(patch.sw, Some(SizeMode::Fill));
        }
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn rejects_unknown_fields_everywhere() {
    // unknown field on a command
    assert!(
        serde_json::from_str::<Command>(r#"{"type":"removeNode","id":"a","bogus":1}"#).is_err()
    );
    // unknown field inside a patch
    assert!(
        serde_json::from_str::<Command>(r#"{"type":"updateNode","id":"a","patch":{"zzz":1}}"#)
            .is_err()
    );
    // unknown field on a node
    assert!(serde_json::from_str::<Node>(r#"{"id":"a","type":"rect","zzz":1}"#).is_err());
    // unknown message type
    assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"nuke"}"#).is_err());
    // field-less variants must not let unknown fields through
    assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"ping","extra":true}"#).is_err());
    assert!(serde_json::from_str::<ServerMsg>(r#"{"type":"pong","extra":1}"#).is_err());
    assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"ping"}"#).is_ok());
    // unknown enum value
    assert!(
        serde_json::from_str::<Command>(r#"{"type":"updateNode","id":"a","patch":{"sw":"huge"}}"#)
            .is_err()
    );
}

#[test]
fn old_node_without_optional_fields_is_valid_and_gets_defaults() {
    let n: Node = serde_json::from_str(r#"{"id":"a","type":"rect"}"#).unwrap();
    assert_eq!(n.kind, NodeKind::Rect);
    assert_eq!((n.w, n.h), (100.0, 100.0));
    assert_eq!(n.pos, "");
    assert_eq!(n.font_weight, 400);
    assert_eq!(n.border.width, 0.0);
    assert!(n.shadows.is_empty());
    assert!(n.image.is_none());
    // optional members that are unset stay off the wire
    let s = serde_json::to_string(&n).unwrap();
    assert!(!s.contains("\"pos\"") && !s.contains("\"image\""), "{s}");
}

#[test]
fn op_roundtrips_in_the_documented_wire_format() {
    let m = ClientMsg::Op {
        id: "m1".into(),
        idempotency_key: "k1".into(),
        room: "file:abc".into(),
        command: Command::RemoveNode { id: "n1".into() },
        if_seq: None,
        deps: vec![],
    };
    let s = serde_json::to_string(&m).unwrap();
    assert_eq!(
        s,
        r#"{"type":"op","id":"m1","idempotencyKey":"k1","room":"file:abc","command":{"type":"removeNode","id":"n1"}}"#
    );
    assert!(matches!(
        serde_json::from_str::<ClientMsg>(&s).unwrap(),
        ClientMsg::Op { .. }
    ));
}

#[test]
fn if_seq_and_error_code_are_optional() {
    let old: ClientMsg = serde_json::from_str(OP_MIN).unwrap();
    assert!(matches!(old, ClientMsg::Op { if_seq: None, .. }));
    let new: ClientMsg = serde_json::from_str(
        r#"{"type":"op","id":"m","idempotencyKey":"k","room":"r","ifSeq":7,"command":{"type":"removeNode","id":"a"}}"#,
    )
    .unwrap();
    assert!(matches!(
        new,
        ClientMsg::Op {
            if_seq: Some(7),
            ..
        }
    ));
    let e = ServerMsg::Error {
        id: None,
        message: "x".into(),
        retryable: false,
        code: None,
        retry_after_ms: None,
    };
    assert_eq!(
        serde_json::to_string(&e).unwrap(),
        r#"{"type":"error","message":"x","retryable":false}"#
    );
    let e2: ServerMsg = serde_json::from_str(
        r#"{"type":"error","message":"x","retryable":false,"code":"conflict"}"#,
    )
    .unwrap();
    assert!(matches!(
        e2,
        ServerMsg::Error {
            code: Some(_),
            retry_after_ms: None,
            ..
        }
    ));
}

#[test]
fn deps_are_optional_and_roundtrip() {
    let old: ClientMsg = serde_json::from_str(OP_MIN).unwrap();
    assert!(matches!(&old, ClientMsg::Op { deps, .. } if deps.is_empty()));
    assert!(!serde_json::to_string(&old).unwrap().contains("deps"));
    let new: ClientMsg = serde_json::from_str(
        r#"{"type":"op","id":"m","idempotencyKey":"k","room":"r","deps":["a-1","a-2"],"command":{"type":"removeNode","id":"a"}}"#,
    )
    .unwrap();
    assert!(matches!(&new, ClientMsg::Op { deps, .. } if deps == &["a-1", "a-2"]));
    assert!(
        serde_json::to_string(&new)
            .unwrap()
            .contains(r#""deps":["a-1","a-2"]"#)
    );
    // a wrong type is refused
    assert!(
        serde_json::from_str::<ClientMsg>(
            r#"{"type":"op","id":"m","idempotencyKey":"k","room":"r","deps":"a-1","command":{"type":"removeNode","id":"a"}}"#
        )
        .is_err()
    );
    assert_eq!(MAX_OP_DEPS, 512);
}

#[test]
fn subscribe_since_seq_is_optional() {
    let m: ClientMsg = serde_json::from_str(r#"{"type":"subscribe","id":"s","room":"r"}"#).unwrap();
    assert!(matches!(
        m,
        ClientMsg::Subscribe {
            since_seq: None,
            ..
        }
    ));
    let m: ClientMsg =
        serde_json::from_str(r#"{"type":"subscribe","id":"s","room":"r","sinceSeq":9}"#).unwrap();
    assert!(matches!(
        m,
        ClientMsg::Subscribe {
            since_seq: Some(9),
            ..
        }
    ));
}

#[test]
fn capture_messages_roundtrip() {
    let m = ServerMsg::CaptureRequest {
        request_id: "r".into(),
        room: "file:a".into(),
        node_id: None,
        scale: 1.0,
        max_pixels: 100,
        min_seq: 3,
        upload_token: "t".into(),
        timeout_ms: 1000,
    };
    let s = serde_json::to_string(&m).unwrap();
    assert!(
        s.contains(r#""type":"captureRequest""#)
            && s.contains(r#""requestId":"r""#)
            && !s.contains("nodeId")
    );
    assert!(serde_json::from_str::<ServerMsg>(&s).is_ok());
    let c: ClientMsg =
        serde_json::from_str(r#"{"type":"capabilities","room":"r","capabilities":["capture"]}"#)
            .unwrap();
    assert!(matches!(c, ClientMsg::Capabilities { .. }));
    let f: ClientMsg = serde_json::from_str(
        r#"{"type":"captureFailed","requestId":"r","code":"too_large","message":"x"}"#,
    )
    .unwrap();
    assert!(matches!(f, ClientMsg::CaptureFailed { .. }));
    assert!(
        serde_json::from_str::<ClientMsg>(
            r#"{"type":"capabilities","room":"r","capabilities":[],"x":1}"#
        )
        .is_err()
    );
}

#[test]
fn image_patch_has_three_states() {
    let absent: NodePatch = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(absent.image, None);
    let cleared: NodePatch = serde_json::from_str(r#"{"image":null}"#).unwrap();
    assert_eq!(cleared.image, Some(None));
    let set: NodePatch =
        serde_json::from_str(r#"{"image":{"assetId":"ab","fit":"contain"}}"#).unwrap();
    assert_eq!(
        set.image,
        Some(Some(ImageFill {
            asset_id: "ab".into(),
            fit: ImageFit::Contain
        }))
    );
    // the image fit defaults when omitted
    let dflt: NodePatch = serde_json::from_str(r#"{"image":{"assetId":"ab"}}"#).unwrap();
    assert_eq!(
        dflt.image,
        Some(Some(ImageFill {
            asset_id: "ab".into(),
            fit: ImageFit::Cover
        }))
    );
}

#[test]
fn patch_applies_only_present_fields() {
    let mut n: Node =
        serde_json::from_str(r#"{"id":"a","type":"rect","image":{"assetId":"ab"}}"#).unwrap();
    n.apply_patch(&NodePatch {
        w: Some(250.0),
        ..Default::default()
    });
    assert_eq!((n.w, n.h), (250.0, 100.0));
    assert!(
        n.image.is_some(),
        "an absent image key leaves the image alone"
    );
    n.apply_patch(&serde_json::from_str(r#"{"image":null}"#).unwrap());
    assert!(n.image.is_none(), "an explicit null removes the image");
}

#[test]
fn schema_version_starts_at_one() {
    assert_eq!(SCHEMA_VERSION, 1);
}
