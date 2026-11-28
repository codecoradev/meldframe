//! The script: one step a line, read into a [`Step`]. A step that drives the app is read into the
//! [`Action`] it stands for (through the action's own JSON shape, so no rule is written here); the rest
//! of the steps are the harness's own (open a file, ask the palette, check what was seen).

use meldframe_app::{Action, KeyFocus, KeyInput, Modifiers};
use serde_json::{Value, json};

/// How a frame is exported.
#[derive(Debug, Clone, PartialEq)]
pub enum Form {
    /// The PNG, at a scale: the app plans it and the harness answers the drawing (`Effect::Raster`) as a shell would.
    Png(f64),
    /// The HTML of the open dialog.
    Html,
}

/// A check of what the last steps made. A failed check fails the run, whatever the mode.
#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    /// The last step raised this refusal code.
    Refused(String),
    /// The last step raised no refusal.
    Clean,
    /// A field of a node of the file as the person sees it.
    Node {
        id: String,
        field: String,
        value: Value,
    },
    /// A value of `AppState` as JSON, by JSON pointer (`/tool`, `/history/canUndo`).
    State { pointer: String, value: Value },
    /// The last image the harness delivered had this size in pixels.
    Delivered { width: u64, height: u64 },
    /// A value of what the gesture going on would do (`App::pointer_preview`) as JSON, by JSON pointer: `null` when
    /// nothing is going on or the pointer names nothing (`/kind`, `/slot/index`, `/rect/w`).
    Preview { pointer: String, value: Value },
    /// The children of a node in sibling order (the order the canvas draws a flow in).
    Order { parent: String, ids: Vec<String> },
    /// The last step returned an effect of this kind and, when a field is given, with this value in it.
    Effect {
        kind: String,
        field: Option<(String, Value)>,
    },
}

/// One line of a script.
#[derive(Debug, Clone)]
pub enum Step {
    /// Load the nodes of a file (a JSON array of nodes) and connect the app.
    Open(String),
    /// Send one action (boxed: an action is large).
    Act(Box<Action>),
    /// List the palette entries for a query, and run one by its id when `run` is given (with the world point at the
    /// middle of the view, where a new frame is centred).
    Palette {
        query: String,
        run: Option<(String, (f64, f64))>,
    },
    /// Export the frame `id`.
    Export {
        id: String,
        form: Form,
    },
    /// From here on the shell says each image it drew is this many bytes large (the default is the smallest a PNG can be), so a
    /// script can hand the app an image over a limit as data.
    Bytes(u64),
    /// Print the summary of the state.
    State,
    /// Print the nodes of the file.
    Nodes,
    /// From here on a refusal fails the run.
    Strict,
    Expect(Expect),
}

fn number(token: Option<&str>, what: &str) -> Result<f64, String> {
    let t = token.ok_or_else(|| format!("missing {what}"))?;
    t.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("{what} is not a number: {t}"))
}

fn word<'a>(token: Option<&'a str>, what: &str) -> Result<&'a str, String> {
    token.ok_or_else(|| format!("missing {what}"))
}

/// A value of an `expect` line: JSON when it is JSON, else the text.
fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))
}

fn action(v: Value) -> Result<Step, String> {
    serde_json::from_value(v)
        .map(|a: Action| Step::Act(Box::new(a)))
        .map_err(|e| format!("not an action: {e}"))
}

/// A key as a person writes it: `r`, `Delete`, `ctrl+z`, `shift+?`, `ctrl++`.
pub fn key(combo: &str) -> Result<Action, String> {
    let (mods, key) = match combo.strip_suffix("++") {
        Some(m) => (format!("{m}+"), "+"),
        None => match combo.rsplit_once('+') {
            Some((m, k)) if !k.is_empty() => (m.to_string(), k),
            _ => (String::new(), combo),
        },
    };
    let mut modifiers = Modifiers::default();
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" => modifiers.ctrl = true,
            "meta" | "cmd" => modifiers.meta = true,
            "alt" | "option" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            other => return Err(format!("unknown modifier: {other}")),
        }
    }
    if key.is_empty() {
        return Err("missing key".into());
    }
    Ok(Action::Key {
        input: KeyInput {
            key: key.into(),
            code: String::new(),
            modifiers,
            repeat: false,
            composing: false,
            focus: KeyFocus::Page,
        },
    })
}

/// Read one line. `None` for a blank line or a comment.
pub fn parse(line: &str) -> Result<Option<Step>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let (cmd, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let t: Vec<&str> = rest.split_whitespace().collect();
    let t = t.as_slice();
    let n = |i: usize, what: &str| number(t.get(i).copied(), what);
    let w = |i: usize, what: &str| word(t.get(i).copied(), what);
    let step = match cmd {
        "open" => Step::Open(w(0, "a path")?.into()),
        "select" => return action(json!({"type": "select", "ids": t})).map(Some),
        "clear" => return action(json!({"type": "clearSelection"})).map(Some),
        "tool" => return action(json!({"type": "setTool", "tool": w(0, "a tool")?})).map(Some),
        "move" => {
            let (id, x, y) = (w(0, "an id")?, n(1, "x")?, n(2, "y")?);
            return action(json!({"type": "move", "id": id, "x": x, "y": y})).map(Some);
        }
        "moveby" => {
            return action(json!({"type": "moveBy", "dx": n(0, "dx")?, "dy": n(1, "dy")?}))
                .map(Some);
        }
        "drop" => {
            let id = w(0, "an id")?;
            let (x, y, dx, dy) = (n(1, "x")?, n(2, "y")?, n(3, "dx")?, n(4, "dy")?);
            let v = json!({"type": "dropNodes", "ids": [id], "x": x, "y": y, "dx": dx, "dy": dy});
            return action(v).map(Some);
        }
        "resize" => {
            let (id, wd, h) = (w(0, "an id")?, n(1, "w")?, n(2, "h")?);
            return action(json!({"type": "resize", "id": id, "w": wd, "h": h})).map(Some);
        }
        "handle" => {
            let (id, handle) = (w(0, "an id")?, w(1, "a handle")?);
            let by = json!({"x": n(2, "dx")?, "y": n(3, "dy")?});
            let op = json!({"type": "resizeHandle", "id": id, "handle": handle, "by": by});
            return action(json!({"type": "gesture", "op": op})).map(Some);
        }
        "draw" => {
            let kind = w(0, "a kind")?;
            let from = json!({"x": n(1, "x1")?, "y": n(2, "y1")?});
            let to = json!({"x": n(3, "x2")?, "y": n(4, "y2")?});
            let op = json!({"type": "draw", "kind": kind, "from": from, "to": to});
            return action(json!({"type": "gesture", "op": op})).map(Some);
        }
        "camera" => {
            let op = match w(0, "a camera step")? {
                "viewport" => json!({"type": "setViewport", "w": n(1, "w")?, "h": n(2, "h")?}),
                "pan" => json!({"type": "pan", "dx": n(1, "dx")?, "dy": n(2, "dy")?}),
                "zoom" => json!({"type": "zoom", "command": w(1, "a zoom command")?}),
                other => return Err(format!("unknown camera step: {other}")),
            };
            return action(json!({"type": "camera", "op": op})).map(Some);
        }
        "pointer" => {
            let phase = w(0, "a phase (down, move, up, cancel, leave)")?;
            let (x, y) = (n(1, "x")?, n(2, "y")?);
            let mut input = json!({"phase": phase, "x": x, "y": y});
            for flag in t.iter().skip(3) {
                match *flag {
                    "shift" | "ctrl" | "alt" | "meta" => input["modifiers"][*flag] = json!(true),
                    "space" | "touch" => input[*flag] = json!(true),
                    "middle" | "secondary" => input["button"] = json!(flag),
                    other => return Err(format!("unknown pointer flag: {other}")),
                }
            }
            return action(json!({"type": "pointer", "input": input})).map(Some);
        }
        "key" => Step::Act(Box::new(key(w(0, "a key")?)?)),
        "undo" => return action(json!({"type": "undo"})).map(Some),
        "redo" => return action(json!({"type": "redo"})).map(Some),
        "action" => {
            return action(serde_json::from_str(rest).map_err(|e| e.to_string())?).map(Some);
        }
        "palette" => {
            let at = t.iter().position(|x| *x == "run");
            let (query, run) = match at {
                Some(i) => {
                    let id = w(i + 1, "an entry id")?.to_string();
                    let centre = match t.len() > i + 2 {
                        true => (n(i + 2, "cx")?, n(i + 3, "cy")?),
                        false => (0.0, 0.0),
                    };
                    (t[..i].join(" "), Some((id, centre)))
                }
                None => (t.join(" "), None),
            };
            Step::Palette { query, run }
        }
        "export" => {
            let id = w(0, "an id")?.to_string();
            let form = match w(1, "png or html")? {
                "png" => Form::Png(if t.len() > 2 { n(2, "scale")? } else { 1.0 }),
                "html" => Form::Html,
                other => return Err(format!("unknown export form: {other}")),
            };
            Step::Export { id, form }
        }
        "bytes" => {
            let size = w(0, "a size")?;
            Step::Bytes(
                size.parse()
                    .map_err(|_| format!("bytes is not a size: {size}"))?,
            )
        }
        "state" => Step::State,
        "nodes" => Step::Nodes,
        "strict" => Step::Strict,
        "expect" => Step::Expect(expect(t)?),
        other => return Err(format!("unknown step: {other}")),
    };
    Ok(Some(step))
}

fn expect(t: &[&str]) -> Result<Expect, String> {
    let at = |i: usize, what: &str| word(t.get(i).copied(), what);
    let size = |i: usize| match at(i, "a size")?.parse::<u64>() {
        Ok(v) => Ok(v),
        Err(_) => Err(format!("not a size: {}", t[i])),
    };
    Ok(match at(0, "what to expect")? {
        "refused" => Expect::Refused(at(1, "a code")?.into()),
        "clean" => Expect::Clean,
        "node" => Expect::Node {
            id: at(1, "an id")?.into(),
            field: at(2, "a field")?.into(),
            value: value(at(3, "a value")?),
        },
        "state" => Expect::State {
            pointer: at(1, "a JSON pointer")?.into(),
            value: value(&t.get(2..).unwrap_or_default().join(" ")),
        },
        "delivered" => Expect::Delivered {
            width: size(1)?,
            height: size(2)?,
        },
        "preview" => Expect::Preview {
            pointer: at(1, "a JSON pointer")?.into(),
            value: value(&t.get(2..).unwrap_or_default().join(" ")),
        },
        "order" => Expect::Order {
            parent: at(1, "a parent")?.into(),
            ids: t[2..].iter().map(|s| s.to_string()).collect(),
        },
        "effect" => Expect::Effect {
            kind: at(1, "an effect")?.into(),
            field: match t.get(2) {
                Some(field) => Some((field.to_string(), value(at(3, "a value")?))),
                None => None,
            },
        },
        other => return Err(format!("unknown check: {other}")),
    })
}
