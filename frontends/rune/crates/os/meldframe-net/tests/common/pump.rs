//! What the tests of the network half share: a pump that collects what a `Net` outputs and waits for the one a test is after.

#![allow(dead_code)]

use meldframe_app_model::state::Connection;
use meldframe_app_model::{Action, Effect, OutOp};
use meldframe_net::{Config, Net, Output};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::server::{ACTOR, Server};

pub struct Pump {
    pub net: Net,
    pub seen: Vec<Output>,
}

impl Pump {
    pub fn new(server: &Server, file: &str, token: &str) -> Pump {
        let config = Config::new(&server.url(), file, ACTOR, token).expect("a config");
        Pump {
            net: Net::start(config, Arc::new(|| {})),
            seen: Vec::new(),
        }
    }

    pub fn sync(&mut self, effects: Vec<Effect>, connection: Connection) {
        self.net.sync(effects, &connection);
    }

    /// The first output after `from` that `found` takes, within ten seconds.
    pub fn wait<T>(&mut self, from: usize, mut found: impl FnMut(&Output) -> Option<T>) -> T {
        let until = Instant::now() + Duration::from_secs(10);
        let mut at = from;
        loop {
            while at < self.seen.len() {
                if let Some(t) = found(&self.seen[at]) {
                    return t;
                }
                at += 1;
            }
            assert!(
                Instant::now() < until,
                "timed out; saw {:#?}",
                &self.seen[from..]
            );
            self.seen.extend(self.net.wait(Duration::from_millis(100)));
        }
    }

    pub fn action<T>(&mut self, from: usize, mut found: impl FnMut(&Action) -> Option<T>) -> T {
        self.wait(from, |o| match o {
            Output::Action(a) => found(a),
            Output::Problem(_) => None,
        })
    }

    /// Wait for the load and the subscription, as the app layer would ask for it.
    pub fn live(&mut self, after_seq: u64) {
        self.action(0, |a| matches!(a, Action::Reload { .. }).then_some(()));
        self.sync(
            vec![Effect::Subscribe { after_seq }],
            Connection::Connecting,
        );
        self.action(0, |a| matches!(a, Action::Connected { .. }).then_some(()));
        self.sync(vec![], Connection::Live);
    }
}

pub fn op(key: &str, command: &str) -> OutOp {
    OutOp {
        key: key.into(),
        command: serde_json::from_str(command).expect("a command"),
        deps: Vec::new(),
    }
}

pub fn add(id: &str) -> String {
    format!(
        r#"{{"type":"addNode","node":{{"id":"{id}","type":"frame","x":0,"y":0,"w":10,"h":10}}}}"#
    )
}
