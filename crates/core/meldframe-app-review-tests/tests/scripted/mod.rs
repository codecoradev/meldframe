//! A scripted server for the checkpoint tests: it keeps rows, answers the effects of the app that ask for them, and can
//! be told to fail or to say nothing. The app talks to it only through effects and
//! actions, as the shell does.
#![allow(dead_code)]

use super::common::*;
use meldframe_app::{Action, CheckpointKind, CheckpointRequest, CheckpointRow, Effect, Frame};
use meldframe_protocol::{Command, NodePatch};

pub struct Cp {
    pub l: Loopback,
    pub rows: Vec<CheckpointRow>,
    /// A code to answer the next post with, as a refusal.
    pub fail_post: Option<&'static str>,
    /// Say nothing to posts and reads: the answer has not come yet.
    pub silent: bool,
    /// Every effect the app asked for, in order.
    pub asked: Vec<Effect>,
}

impl Cp {
    /// A live app that has read an empty list and may make checkpoints.
    pub fn live() -> Self {
        let mut c = Cp {
            l: Loopback::live(),
            rows: Vec::new(),
            fail_post: None,
            silent: false,
            asked: Vec::new(),
        };
        c.l.send(Action::CheckpointsLoaded {
            checkpoints: Vec::new(),
            can_create: true,
        });
        c
    }

    /// The server makes a row at the head, with the file as the app confirmed it, and announces it by a frame.
    pub fn make(&mut self, kind: CheckpointKind, label: Option<&str>, about: Option<&str>) -> u32 {
        let row = self.row(kind, label, about, self.l.seq);
        let n = row.n;
        self.rows.push(row.clone());
        let frame = Frame::Checkpoint { checkpoint: row };
        self.l.app.dispatch(Action::Frame { now_ms: 0, frame });
        self.l.events.extend(self.l.app.take_events());
        n
    }

    fn row(
        &self,
        kind: CheckpointKind,
        label: Option<&str>,
        about: Option<&str>,
        seq: u64,
    ) -> CheckpointRow {
        let by = (kind != CheckpointKind::RunStart).then(|| "user:me".to_string());
        CheckpointRow {
            n: self.rows.len() as u32 + 1,
            seq,
            kind,
            label: label.map(String::from),
            about: about.map(String::from),
            by,
            at_ms: 1,
        }
    }

    /// Dispatch, then answer every checkpoint effect that comes out, and the effects of the answers. Returns all.
    pub fn send(&mut self, action: Action) -> Vec<Effect> {
        let mut todo = std::collections::VecDeque::from(self.l.send(action));
        let mut all = Vec::new();
        while let Some(effect) = todo.pop_front() {
            self.asked.push(effect.clone());
            if let Some(answer) = self.answer(&effect) {
                todo.extend(self.l.send(answer));
            }
            all.push(effect);
        }
        all
    }

    fn answer(&mut self, effect: &Effect) -> Option<Action> {
        if self.silent {
            return None;
        }
        match effect {
            Effect::FetchCheckpoints {} => Some(Action::CheckpointsLoaded {
                checkpoints: self.rows.iter().rev().cloned().collect(),
                can_create: true,
            }),
            Effect::PostCheckpoint {
                key,
                kind,
                label,
                about,
                seq,
            } => {
                if let Some(code) = self.fail_post {
                    let request = CheckpointRequest::Post { key: key.clone() };
                    return Some(Action::CheckpointRequestFailed {
                        request,
                        code: code.into(),
                    });
                }
                let row = self.row(*kind, label.as_deref(), about.as_deref(), *seq);
                assert_eq!(*seq, self.l.seq, "the app asks for the file it confirmed");
                self.rows.push(row.clone());
                Some(Action::CheckpointPosted {
                    key: key.clone(),
                    checkpoint: row,
                })
            }
            _ => None,
        }
    }

    /// The effects asked for so far that are posts, as `(kind, about, seq)`.
    pub fn posts(&self) -> Vec<(CheckpointKind, Option<String>, u64)> {
        let post = |e: &Effect| match e {
            Effect::PostCheckpoint {
                kind, about, seq, ..
            } => Some((*kind, about.clone(), *seq)),
            _ => None,
        };
        self.asked.iter().filter_map(post).collect()
    }
}

pub fn set(id: &str, f: impl FnOnce(&mut NodePatch)) -> Command {
    let mut patch = NodePatch::default();
    f(&mut patch);
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

pub fn add(id: &str, kind: &str, parent: &str) -> Command {
    Command::AddNode {
        node: node(id, kind, Some(parent)),
        index: None,
    }
}
