//! Runs a script against a [`Shell`] and writes down what it saw as plain text, one block a step.

use crate::script::{Expect, Form, Step, parse};
use crate::shell::{Seen, Shell};
use meldframe_app::{Action, PngOp, PngSink};
use meldframe_protocol::Node;
use serde_json::Value;

/// Reads a file the script names (`open`): the text of it, or why it cannot.
pub type Files<'a> = &'a dyn Fn(&str) -> Result<String, String>;

/// What a run came to.
#[derive(Debug)]
pub struct Report {
    /// What the steps saw, as text.
    pub out: String,
    /// Why the run stopped, with the line; `None` when every step went through.
    pub failure: Option<String>,
    /// The refusals the steps raised, in order.
    pub refusals: Vec<String>,
    /// The shell at the end, for a caller that wants to read the app.
    pub shell: Option<Shell>,
}

/// Numbers compare as numbers, the rest as JSON.
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

struct Run<'a> {
    files: Files<'a>,
    shell: Option<Shell>,
    strict: bool,
    out: String,
    refusals: Vec<String>,
    /// The refusals of the last step that drove the app.
    last: Vec<String>,
    /// A refusal of a strict run, with its line, waiting for the next step: an `expect refused` takes it, any other step
    /// fails the run at the line that was refused.
    held: Option<(usize, String)>,
    /// The line being run.
    line: usize,
}

impl Run<'_> {
    fn say(&mut self, line: impl AsRef<str>) {
        self.out.push_str(line.as_ref());
        self.out.push('\n');
    }

    fn shell(&mut self) -> Result<&mut Shell, String> {
        self.shell
            .as_mut()
            .ok_or_else(|| "no file is open".to_string())
    }

    /// Write what the app returned for a step; a refusal in a strict run fails it.
    fn note(&mut self, seen: &Seen) -> Result<(), String> {
        for (label, tags) in [
            ("effects", seen.effect_tags()),
            ("events", seen.event_tags()),
        ] {
            if !tags.is_empty() {
                self.say(format!("  {label}: {}", tags.join(" ")));
            }
        }
        self.last = seen.refusals();
        for code in &self.last.clone() {
            self.say(format!("  refused: {code}"));
        }
        self.refusals.extend(self.last.clone());
        if let (true, Some(code)) = (self.strict, self.last.first()) {
            self.held = Some((self.line, code.clone()));
        }
        Ok(())
    }

    /// Before the next step: a held refusal is excused by an `expect refused` and fails the run for any other step.
    fn release(&mut self, next: Option<&Step>) -> Result<(), (usize, String)> {
        match (self.held.take(), next) {
            (Some(_), Some(Step::Expect(Expect::Refused(_)))) | (None, _) => Ok(()),
            (Some((n, code)), _) => Err((n, format!("refused: {code}"))),
        }
    }

    fn step(&mut self, step: Step) -> Result<(), String> {
        match step {
            Step::Open(path) => {
                let text = (self.files)(&path)?;
                let nodes: Vec<Node> =
                    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
                let count = nodes.len();
                self.shell = Some(Shell::open(nodes)?);
                self.say(format!("  opened {path}: {count} nodes"));
                Ok(())
            }
            Step::Act(action) => {
                let seen = self.shell()?.act(*action);
                self.note(&seen)
            }
            Step::Palette { query, run: None } => {
                let entries = self.shell()?.palette(&query);
                for e in entries {
                    let on = if e.enabled { "" } else { " (disabled)" };
                    self.say(format!("  {}: {}{on}", e.id, e.label));
                }
                self.last.clear();
                Ok(())
            }
            Step::Palette {
                run: Some((id, centre)),
                ..
            } => match self.shell()?.run_palette(&id, centre) {
                Some(seen) => self.note(&seen),
                None => {
                    let seen = Seen {
                        events: vec![meldframe_app::Event::Refused {
                            code: "palette_refused".into(),
                        }],
                        ..Seen::default()
                    };
                    self.note(&seen)
                }
            },
            Step::Export { id, form } => {
                let select = Action::Select {
                    ids: vec![id.clone()],
                };
                let mut seen = self.shell()?.act(select);
                let more = match form {
                    Form::Png(scale) => {
                        let s = self.shell()?;
                        let mut seen = s.act(Action::Png {
                            op: PngOp::SetScale { scale },
                        });
                        // A scale the app refuses stops the step: no export at the scale that was there before.
                        if seen.refusals().is_empty() {
                            let op = PngOp::Export {
                                id,
                                to: PngSink::Download,
                            };
                            let export = s.act(Action::Png { op });
                            seen.effects.extend(export.effects);
                            seen.events.extend(export.events);
                        }
                        seen
                    }
                    Form::Html => self.shell()?.act(Action::OpenExport {}),
                };
                seen.effects.extend(more.effects);
                seen.events.extend(more.events);
                self.note(&seen)?;
                let dialog = self.shell()?.state().pointer("/export/dialog").cloned();
                if let Some(d) = dialog.filter(|d| !d.is_null()) {
                    let (file, bytes) = (&d["htmlFile"], &d["htmlBytes"]);
                    self.say(format!("  html: {file} {bytes} bytes"));
                }
                Ok(())
            }
            Step::State => {
                let state = self.shell()?.state();
                for (label, pointer) in [
                    ("selection", "/selection"),
                    ("tool", "/tool"),
                    ("can undo", "/history/canUndo"),
                    ("can redo", "/history/canRedo"),
                    ("pending", "/pending"),
                    ("confirmed seq", "/confirmedSeq"),
                    ("overlay", "/overlay"),
                    ("camera", "/camera"),
                ] {
                    let v = state.pointer(pointer).cloned().unwrap_or(Value::Null);
                    self.say(format!("  {label}: {v}"));
                }
                Ok(())
            }
            Step::Nodes => {
                let nodes = self.shell()?.nodes();
                for n in nodes.as_array().into_iter().flatten() {
                    self.say(format!("  {n}"));
                }
                Ok(())
            }
            Step::Strict => {
                self.strict = true;
                Ok(())
            }
            Step::Expect(check) => self.expect(&check),
        }
    }

    fn expect(&mut self, check: &Expect) -> Result<(), String> {
        match check {
            Expect::Refused(code) if self.last.contains(code) => Ok(()),
            Expect::Refused(code) => Err(format!("expected refused {code}, saw {:?}", self.last)),
            Expect::Clean if self.last.is_empty() => Ok(()),
            Expect::Clean => Err(format!("expected no refusal, saw {:?}", self.last)),
            Expect::Node { id, field, value } => {
                let nodes = self.shell()?.nodes();
                let node = nodes
                    .as_array()
                    .and_then(|l| l.iter().find(|n| n["id"] == id.as_str()))
                    .ok_or_else(|| format!("no node {id}"))?;
                let got = &node[field.as_str()];
                match same(got, value) {
                    true => Ok(()),
                    false => Err(format!("node {id} {field}: expected {value}, got {got}")),
                }
            }
            Expect::State { pointer, value } => {
                let state = self.shell()?.state();
                let got = state.pointer(pointer).cloned().unwrap_or(Value::Null);
                match same(&got, value) {
                    true => Ok(()),
                    false => Err(format!("state {pointer}: expected {value}, got {got}")),
                }
            }
            Expect::Delivered { width, height } => {
                let got = self.shell()?.delivered();
                match got {
                    Some((w, h)) if u64::from(w) == *width && u64::from(h) == *height => Ok(()),
                    _ => Err(format!(
                        "expected an image of {width} by {height}, delivered {got:?}"
                    )),
                }
            }
        }
    }
}

/// Run every line of `script`. The run stops at the first step that fails (a line that is not a step, a failed
/// check, or, in a strict run, a refusal), and says which line and why. `strict` starts the run in strict mode,
/// and the `strict` step turns it on from its line.
pub fn run(script: &str, files: Files<'_>, strict: bool) -> Report {
    let mut r = Run {
        files,
        shell: None,
        strict,
        out: String::new(),
        refusals: Vec::new(),
        last: Vec::new(),
        held: None,
        line: 0,
    };
    let mut failure = None;
    for (i, line) in script.lines().enumerate() {
        let n = i + 1;
        let parsed = parse(line);
        if matches!(parsed, Ok(None)) {
            continue;
        }
        if let Err((at, why)) = r.release(parsed.as_ref().ok().and_then(Option::as_ref)) {
            r.say(format!("  FAILED: {why}"));
            failure = Some(format!("line {at}: {why}"));
            break;
        }
        let step = match parsed {
            Ok(Some(step)) => step,
            Ok(None) => continue,
            Err(e) => {
                failure = Some(format!("line {n}: {e}"));
                break;
            }
        };
        r.line = n;
        r.say(format!("[{n}] {}", line.trim()));
        if let Err(e) = r.step(step) {
            r.say(format!("  FAILED: {e}"));
            failure = Some(format!("line {n}: {e}"));
            break;
        }
    }
    // The end of the script lets no refusal go.
    if let (None, Err((at, why))) = (&failure, r.release(None)) {
        r.say(format!("  FAILED: {why}"));
        failure = Some(format!("line {at}: {why}"));
    }
    Report {
        out: r.out,
        failure,
        refusals: r.refusals,
        shell: r.shell,
    }
}
