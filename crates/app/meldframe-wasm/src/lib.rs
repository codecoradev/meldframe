//! Thin wasm-bindgen glue over core and the app layer.
//!
//! The boundary is JSON text in and out: the browser sends an `Action` and gets the `Effect`s back, and
//! reads `AppState`, the nodes and the `Event`s as JSON. The rules stay in `meldframe-app`; this crate
//! only converts. [`Bridge`] is plain Rust (tested natively); [`WasmApp`] is its wasm-bindgen face.

use meldframe_app::{Action, App};
use meldframe_protocol::Node;
use wasm_bindgen::prelude::*;

/// The app behind a JSON boundary. Errors are plain strings (a stable shape for the shell to show).
#[derive(Debug)]
pub struct Bridge {
    app: App,
}

impl Bridge {
    /// `nodes_json` is the file's node list as of server sequence number `seq`.
    pub fn new(
        actor: &str,
        key_prefix: &str,
        session: u64,
        nodes_json: &str,
        seq: u64,
    ) -> Result<Self, String> {
        let nodes: Vec<Node> =
            serde_json::from_str(nodes_json).map_err(|e| format!("nodes: {e}"))?;
        let app =
            App::new(actor, key_prefix, session, nodes, seq).map_err(|e| format!("load: {e:?}"))?;
        Ok(Self { app })
    }

    /// Applies one action (JSON) and returns the effects (JSON array).
    pub fn dispatch(&mut self, action_json: &str) -> Result<String, String> {
        let action: Action =
            serde_json::from_str(action_json).map_err(|e| format!("action: {e}"))?;
        let effects = self.app.dispatch(action);
        serde_json::to_string(&effects).map_err(|e| e.to_string())
    }

    pub fn state_json(&self) -> String {
        to_json(self.app.state())
    }

    pub fn nodes_json(&self) -> String {
        to_json(&self.app.nodes())
    }

    /// The events since the last call (JSON array); empties the queue.
    pub fn take_events_json(&mut self) -> String {
        to_json(&self.app.take_events())
    }
}

fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).expect("plain data serialises")
}

/// The app for the browser: the same five calls as [`Bridge`].
#[wasm_bindgen]
#[derive(Debug)]
pub struct WasmApp {
    bridge: Bridge,
}

#[wasm_bindgen]
impl WasmApp {
    /// `session` is a fresh random nonce per page load (see `App::new`); `seq` a JS number.
    #[wasm_bindgen(constructor)]
    pub fn new(
        actor: &str,
        key_prefix: &str,
        session: u32,
        nodes_json: &str,
        seq: f64,
    ) -> Result<WasmApp, JsValue> {
        if !(seq >= 0.0 && seq.fract() == 0.0 && seq <= 9_007_199_254_740_991.0) {
            return Err(JsValue::from_str("seq: not a whole number"));
        }
        let bridge = Bridge::new(
            actor,
            key_prefix,
            u64::from(session),
            nodes_json,
            seq as u64,
        )
        .map_err(|e| JsValue::from_str(&e))?;
        Ok(WasmApp { bridge })
    }

    pub fn dispatch(&mut self, action_json: &str) -> Result<String, JsValue> {
        self.bridge
            .dispatch(action_json)
            .map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen(js_name = stateJson)]
    pub fn state_json(&self) -> String {
        self.bridge.state_json()
    }

    #[wasm_bindgen(js_name = nodesJson)]
    pub fn nodes_json(&self) -> String {
        self.bridge.nodes_json()
    }

    #[wasm_bindgen(js_name = takeEventsJson)]
    pub fn take_events_json(&mut self) -> String {
        self.bridge.take_events_json()
    }
}
