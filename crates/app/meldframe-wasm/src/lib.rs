//! Thin wasm-bindgen glue over core and the app layer.
//!
//! The boundary is JSON text in and out: the browser sends an `Action` and gets the `Effect`s back, and
//! reads `AppState`, the nodes, the painted boxes (the scene) and the `Event`s as JSON. The rules stay in `meldframe-app`; this crate
//! only converts. [`Bridge`] is plain Rust (tested natively); [`WasmApp`] is its wasm-bindgen face.

mod measure;

use meldframe_app::{Action, App, MeasurerBox};
use meldframe_protocol::{Doc, Node, Variables};
use wasm_bindgen::prelude::*;

pub use measure::{GuardedMeasurer, MAX_MEASURE, MeasureQuery, is_size};

/// The largest whole number a JS number holds exactly (2^53 - 1).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// `value` as an integer, if it is a whole number from 0 to `max`. The error names the argument.
pub fn whole_number(name: &str, value: f64, max: f64) -> Result<u64, String> {
    if value >= 0.0 && value.fract() == 0.0 && value <= max {
        Ok(value as u64)
    } else {
        Err(format!("{name}: not a whole number from 0 to {max}"))
    }
}

/// The app behind a JSON boundary. Errors are plain strings (a stable shape for the shell to show).
#[derive(Debug)]
pub struct Bridge {
    app: App,
}

impl Bridge {
    /// `nodes_json` is the file's node list as of server sequence number `seq`; `variables_json` holds its
    /// collections and variables (`None`: the file has none).
    pub fn new(
        actor: &str,
        key_prefix: &str,
        session: u64,
        nodes_json: &str,
        seq: u64,
        variables_json: Option<&str>,
    ) -> Result<Self, String> {
        let nodes: Vec<Node> =
            serde_json::from_str(nodes_json).map_err(|e| format!("nodes: {e}"))?;
        let vars: Variables = variables_json
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| format!("variables: {e}"))?
            .unwrap_or_default();
        let app = App::with_doc(actor, key_prefix, session, Doc { nodes, vars }, seq)
            .map_err(|e| format!("load: {}", e.code()))?;
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

    /// The collections and variables of the document (JSON; `meldframe_protocol::Variables`).
    pub fn variables_json(&self) -> String {
        to_json(self.app.vars())
    }

    /// The painted boxes of the document, back to front (JSON array; `meldframe_render::PaintedBox`).
    pub fn scene_json(&self) -> String {
        to_json(&self.app.scene())
    }

    /// Where in a flow a drop would put the node: `null`, or `{frame, index, line: {x, y, w, h}}` (JSON; `meldframe_app::DropSlot`;
    /// `at` is the pointer and `(dx, dy)` the drag, world space). It changes nothing.
    pub fn drop_preview_json(&self, id: &str, at: (f64, f64), by: (f64, f64)) -> String {
        to_json(&self.app.drop_preview(id, at, by))
    }

    /// The events since the last call (JSON array); empties the queue.
    pub fn take_events_json(&mut self) -> String {
        to_json(&self.app.take_events())
    }

    /// Host configuration, not an action: whether the scene is laid out (hug, fill, auto layout) or the stored boxes.
    /// Returns the effects to carry out (JSON array; a redraw of everything when the flag changed, `[]` when it did
    /// not). It raises no event, so the caller reads the scene again itself; the properties of the selected node follow
    /// the flag (they show the drawn box), so the state is read again too.
    pub fn set_layout_on_canvas(&mut self, on: bool) -> String {
        to_json(&self.app.set_layout_on_canvas(on))
    }

    /// Host configuration, not an action: the measurer that sizes texts in the laid-out scene (the stub until a host
    /// gives one). Answers like [`Bridge::set_layout_on_canvas`]: a redraw of everything while the flag is on, `[]`
    /// while it is off, no event, no state.
    pub fn set_measurer(&mut self, measurer: MeasurerBox) -> String {
        to_json(&self.app.set_measurer(measurer))
    }
}

fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).expect("plain data serialises")
}

/// The text of a panic for the browser console.
pub fn describe_panic(message: &str, at: Option<(&str, u32)>) -> String {
    match at {
        Some((file, line)) => format!("meldframe-wasm panicked: {message} ({file}:{line})"),
        None => format!("meldframe-wasm panicked: {message}"),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(text: &str);
}

/// Makes a panic a readable error in the browser console instead of "unreachable executed".
#[cfg(target_arch = "wasm32")]
fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "(no message)".into());
            let at = info.location().map(|l| (l.file(), l.line()));
            console_error(&describe_panic(&message, at));
        }));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn install_panic_hook() {}

/// The app for the browser: the same calls as [`Bridge`].
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
        session: f64,
        nodes_json: &str,
        seq: f64,
        variables_json: Option<String>,
    ) -> Result<WasmApp, JsValue> {
        install_panic_hook();
        // JS numbers are coerced silently by wasm-bindgen for integer parameters (-1 becomes 4294967295, NaN
        // becomes 0), so both numbers arrive as f64 and are checked here.
        let session = whole_number("session", session, f64::from(u32::MAX))
            .map_err(|e| JsValue::from_str(&e))?;
        let seq = whole_number("seq", seq, MAX_SAFE_INTEGER).map_err(|e| JsValue::from_str(&e))?;
        let bridge = Bridge::new(
            actor,
            key_prefix,
            session,
            nodes_json,
            seq,
            variables_json.as_deref(),
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

    #[wasm_bindgen(js_name = variablesJson)]
    pub fn variables_json(&self) -> String {
        self.bridge.variables_json()
    }

    #[wasm_bindgen(js_name = nodesJson)]
    pub fn nodes_json(&self) -> String {
        self.bridge.nodes_json()
    }

    #[wasm_bindgen(js_name = sceneJson)]
    pub fn scene_json(&self) -> String {
        self.bridge.scene_json()
    }

    #[wasm_bindgen(js_name = dropPreviewJson)]
    pub fn drop_preview_json(&self, id: &str, x: f64, y: f64, dx: f64, dy: f64) -> String {
        self.bridge.drop_preview_json(id, (x, y), (dx, dy))
    }

    #[wasm_bindgen(js_name = takeEventsJson)]
    pub fn take_events_json(&mut self) -> String {
        self.bridge.take_events_json()
    }

    #[wasm_bindgen(js_name = setLayoutOnCanvas)]
    pub fn set_layout_on_canvas(&mut self, on: bool) -> String {
        self.bridge.set_layout_on_canvas(on)
    }

    /// Gives the canvas a text measurer: `(text, fontSize, fontWeight, wrapWidth | null) => [width, height]` (see the
    /// `measure` module); a bad answer falls back to the stub. Calling it again with the same function makes the app
    /// measure again (the host does that when its fonts finish loading). Returns the effects like `setLayoutOnCanvas`.
    /// Only on wasm: a JavaScript function cannot be held by a native build.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = setMeasurer)]
    pub fn set_measurer(&mut self, function: JsValue) -> Result<String, JsValue> {
        let function: js_sys::Function = function
            .dyn_into()
            .map_err(|_| JsValue::from_str("measurer: not a function"))?;
        Ok(self
            .bridge
            .set_measurer(Box::new(measure::js_measurer(function))))
    }
}
