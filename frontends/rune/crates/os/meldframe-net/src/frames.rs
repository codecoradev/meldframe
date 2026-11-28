//! A text frame of the live socket, read into what the app layer takes. The shell keeps no rule: an op, a heartbeat and a
//! resync marker become `Action::Frame` (the app layer's own `Frame`), the ephemeral hints and the refusal of a `subscribe`
//! are handed over as the text the socket sent (`Wire`), and a frame that is not a frame of the protocol means the connection
//! cannot be trusted any more.

use meldframe_app_model::{Action, Frame, Wire};
use meldframe_protocol::ServerMsg;
use serde_json::Value;

/// What a frame comes to.
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "short-lived values; boxing would leak into every consumer pattern"
)]
pub enum Heard {
    /// The server accepted the `subscribe` that opened the socket (or the one sent on it): the app is connected.
    Subscribed,
    /// An action to dispatch.
    Do(Action),
    /// Not a frame of the protocol: drop the connection and start again from the last confirmed number.
    Lost,
    /// A frame that needs no action (a pong, a capture request this client never asked for, a type of a newer server).
    Ignore,
}

/// Read `text`. `last_sub` is the id of the last `subscribe` the shell sent on this socket: only an `error` frame that
/// answers it is the app's to read (the ids are the shell's, so the shell matches them).
pub fn hear(text: &str, now_ms: u64, last_sub: Option<&str>) -> Heard {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Heard::Lost;
    };
    let Some(kind) = value.get("type").and_then(Value::as_str) else {
        return Heard::Lost;
    };
    match kind {
        "subscribed" => Heard::Subscribed,
        "heartbeat" => frame(now_ms, Frame::Heartbeat {}),
        "event" | "resync" => match serde_json::from_value::<ServerMsg>(value) {
            Ok(ServerMsg::Event {
                seq,
                actor,
                key,
                command,
                ..
            }) => frame(
                now_ms,
                Frame::Event {
                    seq,
                    actor,
                    key,
                    command,
                },
            ),
            Ok(ServerMsg::Resync {
                after_seq,
                latest_seq,
                ..
            }) => frame(
                now_ms,
                Frame::Resync {
                    after_seq,
                    latest_seq,
                },
            ),
            _ => Heard::Lost,
        },
        "batch" | "touch" | "checkpoint" => wire(Wire::Hint {
            now_ms,
            text: text.to_string(),
        }),
        "error" => match (value.get("id").and_then(Value::as_str), last_sub) {
            (Some(id), Some(last)) if id == last => wire(Wire::SubscribeRefused {
                now_ms,
                text: text.to_string(),
            }),
            _ => Heard::Ignore,
        },
        _ => Heard::Ignore,
    }
}

fn frame(now_ms: u64, frame: Frame) -> Heard {
    Heard::Do(Action::Frame { now_ms, frame })
}

fn wire(wire: Wire) -> Heard {
    Heard::Do(Action::Wire { wire })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(h: &Heard) -> &'static str {
        match h {
            Heard::Subscribed => "subscribed",
            Heard::Do(Action::Frame {
                frame: Frame::Heartbeat {},
                ..
            }) => "heartbeat",
            Heard::Do(Action::Frame {
                frame: Frame::Event { .. },
                ..
            }) => "event",
            Heard::Do(Action::Frame {
                frame: Frame::Resync { .. },
                ..
            }) => "resync",
            Heard::Do(Action::Wire {
                wire: Wire::Hint { .. },
            }) => "hint",
            Heard::Do(Action::Wire {
                wire: Wire::SubscribeRefused { .. },
            }) => "refused",
            Heard::Do(_) => "other",
            Heard::Lost => "lost",
            Heard::Ignore => "ignore",
        }
    }

    #[test]
    fn each_frame_of_the_protocol_becomes_what_the_app_layer_takes() {
        let event = r#"{"type":"event","room":"f","seq":3,"actor":"a","key":"k","command":{"type":"removeNode","id":"x"}}"#;
        let cases = [
            (
                r#"{"type":"subscribed","id":"s1","room":"f"}"#,
                "subscribed",
            ),
            (r#"{"type":"heartbeat"}"#, "heartbeat"),
            (event, "event"),
            (
                r#"{"type":"resync","room":"f","afterSeq":2,"latestSeq":9}"#,
                "resync",
            ),
            (
                r#"{"type":"touch","actor":"a","nodeIds":["x"],"seq":3}"#,
                "hint",
            ),
            (r#"{"type":"batch","anything":1}"#, "hint"),
            (r#"{"type":"checkpoint","n":1}"#, "hint"),
            (r#"{"type":"pong"}"#, "ignore"),
            (r#"{"type":"somethingNew","x":1}"#, "ignore"),
        ];
        for (text, want) in cases {
            assert_eq!(kind(&hear(text, 7, Some("s1"))), want, "{text}");
        }
    }

    #[test]
    fn a_frame_that_is_not_the_protocol_loses_the_connection() {
        for text in [
            "not json",
            "[]",
            r#"{"seq":1}"#,
            r#"{"type":7}"#,
            r#"{"type":"event","seq":"x"}"#,
            r#"{"type":"resync","afterSeq":-1,"latestSeq":2}"#,
        ] {
            assert_eq!(kind(&hear(text, 0, None)), "lost", "{text}");
        }
    }

    #[test]
    fn an_error_frame_is_the_apps_only_when_it_answers_the_last_subscribe() {
        let err = |id: &str| {
            format!(
                r#"{{"type":"error","id":"{id}","message":"m","retryable":true,"code":"rate_limited"}}"#
            )
        };
        assert_eq!(kind(&hear(&err("s2"), 0, Some("s2"))), "refused");
        assert_eq!(kind(&hear(&err("s1"), 0, Some("s2"))), "ignore");
        assert_eq!(kind(&hear(&err("s1"), 0, None)), "ignore");
        assert_eq!(
            kind(&hear(
                r#"{"type":"error","message":"m","retryable":false}"#,
                0,
                Some("s1")
            )),
            "ignore"
        );
    }
}
