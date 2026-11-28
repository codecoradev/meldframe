//! Loading the file: `GET /v1/files/{file}` is the snapshot the live stream continues after. The app layer has no raw form
//! for a snapshot yet (`Wire` has none), so the shell reads it as the web host does (`loadFile`): the sequence number, the
//! nodes and the variables, and who last changed each node. A file the server does not know is an empty file at number 0.

use crate::http::{Answer, Http, Method, NoAnswer};
use crate::routes::segment;
use meldframe_app_model::Action;
use meldframe_protocol::{Node, Variables};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// What is wrong with loading, as a status the shell can word without showing any text of the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The server did not accept the token (401) or the token may not read the file (403).
    Refused { status: u16 },
    /// No answer (status 0), another status, or an answer that is not a snapshot (status 200).
    Server { status: u16 },
}

impl Problem {
    /// The problem a status stands for.
    pub fn of_status(status: u16) -> Problem {
        match status {
            401 | 403 => Problem::Refused { status },
            _ => Problem::Server { status },
        }
    }
}

/// The file as the server holds it.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub seq: u64,
    pub nodes: Vec<Node>,
    pub variables: Variables,
    /// Node id to the actor that changed it last.
    pub actors: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Body {
    seq: u64,
    nodes: Vec<Value>,
    #[serde(default)]
    variables: Option<Variables>,
}

impl Loaded {
    /// The snapshot in the text of an answer. Each node carries `lastActor` and `lastSeq`, which are not part of a `Node`.
    pub fn read(text: &str) -> Option<Loaded> {
        let body: Body = serde_json::from_str(text).ok()?;
        let mut actors = BTreeMap::new();
        let mut nodes = Vec::with_capacity(body.nodes.len());
        for mut raw in body.nodes {
            let object = raw.as_object_mut()?;
            let actor = object.remove("lastActor");
            object.remove("lastSeq");
            let node: Node = serde_json::from_value(raw).ok()?;
            if let Some(actor) = actor.as_ref().and_then(Value::as_str) {
                actors.insert(node.id.clone(), actor.to_string());
            }
            nodes.push(node);
        }
        Some(Loaded {
            seq: body.seq,
            nodes,
            variables: body.variables.unwrap_or_default(),
            actors,
        })
    }

    /// The actions that put this file in a running app: the app starts again from the snapshot (`Reload`, which also asks
    /// for the subscription after `seq`), then learns who changed each node.
    pub fn actions(self, now_ms: u64) -> Vec<Action> {
        let mut actions = vec![Action::Reload {
            now_ms,
            nodes: self.nodes,
            variables: self.variables,
            seq: self.seq,
            applied: Vec::new(),
        }];
        if !self.actors.is_empty() {
            actions.push(Action::Attribute {
                actors: self.actors,
            });
        }
        actions
    }
}

/// What a snapshot answer means: the file, an empty file (404), or the problem.
pub fn of_answer(answer: Result<Answer, NoAnswer>) -> Result<Loaded, Problem> {
    match answer {
        Err(NoAnswer) => Err(Problem::Server { status: 0 }),
        Ok((404, _)) => Ok(Loaded {
            seq: 0,
            nodes: Vec::new(),
            variables: Variables::default(),
            actors: BTreeMap::new(),
        }),
        Ok((200, text)) => Loaded::read(&text).ok_or(Problem::Server { status: 200 }),
        Ok((status, _)) => Err(Problem::of_status(status)),
    }
}

/// Fetch the file.
pub fn load(http: &Http, file: &str) -> Result<Loaded, Problem> {
    of_answer(http.call(Method::Get, &format!("/v1/files/{}", segment(file)), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_gives_its_nodes_without_the_attribution_and_the_actors_apart() {
        let text = r##"{"seq":4,"nodes":[
            {"id":"a","type":"frame","lastActor":"agent:x","lastSeq":3},
            {"id":"b","type":"rect","parentId":"a","lastSeq":4}],
            "variables":{"collections":[],"variables":[]}}"##;
        let loaded = Loaded::read(text).expect("a snapshot");
        assert_eq!(loaded.seq, 4);
        assert_eq!(
            loaded
                .nodes
                .iter()
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(loaded.actors.len(), 1);
        assert_eq!(loaded.actors["a"], "agent:x");
        let actions = loaded.actions(9);
        assert!(matches!(
            actions[0],
            Action::Reload {
                now_ms: 9,
                seq: 4,
                ..
            }
        ));
        assert!(matches!(actions[1], Action::Attribute { .. }));
    }

    #[test]
    fn an_answer_is_a_file_an_empty_file_or_a_problem() {
        assert_eq!(
            of_answer(Ok((404, String::new()))).map(|l| (l.seq, l.nodes.len())),
            Ok((0, 0))
        );
        assert_eq!(
            of_answer(Ok((401, "x".into()))).unwrap_err(),
            Problem::Refused { status: 401 }
        );
        assert_eq!(
            of_answer(Ok((403, String::new()))).unwrap_err(),
            Problem::Refused { status: 403 }
        );
        assert_eq!(
            of_answer(Ok((503, String::new()))).unwrap_err(),
            Problem::Server { status: 503 }
        );
        assert_eq!(
            of_answer(Err(NoAnswer)).unwrap_err(),
            Problem::Server { status: 0 }
        );
        for text in [
            "",
            "[]",
            r#"{"seq":-1,"nodes":[]}"#,
            r#"{"seq":1,"nodes":[{"nope":1}]}"#,
            r#"{"seq":1}"#,
        ] {
            assert_eq!(
                of_answer(Ok((200, text.into()))).unwrap_err(),
                Problem::Server { status: 200 },
                "{text}"
            );
        }
    }
}
