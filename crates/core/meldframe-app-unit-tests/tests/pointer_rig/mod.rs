//! The rig of the pointer machine tests: two nodes in world space, `a` at (0, 0) and `b` at (200, 0), both 100 by 100, a made-up
//! selection box, and a host that applies the selection and pan actions as the app does, so the next event sees them.
#![allow(dead_code)]

use meldframe_app_model::{
    Action, Camera, Modifiers, PointerButton, PointerInput, PointerPhase, Tool,
};
use meldframe_app_pointer::{Env, Machine};
use meldframe_layout::Rect;

pub struct Rig {
    pub machine: Machine,
    pub camera: Camera,
    pub selection: Vec<String>,
    pub tool: Tool,
    pub log: Vec<Action>,
    /// The world box of the selection, for the handles.
    pub rect: Option<Rect>,
    /// The clock of the next events, in milliseconds.
    pub now: Option<u64>,
}

pub fn rig(camera: Camera, selection: &[&str]) -> Rig {
    Rig {
        machine: Machine::default(),
        camera,
        selection: selection.iter().map(|s| s.to_string()).collect(),
        tool: Tool::Select,
        log: Vec::new(),
        rect: None,
        now: None,
    }
}

pub fn home() -> Camera {
    Camera::default()
}

pub fn hit(x: f64, y: f64) -> Option<String> {
    let inside = |lo: f64| (lo..=lo + 100.0).contains(&x) && (0.0..=100.0).contains(&y);
    match (inside(0.0), inside(200.0)) {
        (true, _) => Some("a".into()),
        (_, true) => Some("b".into()),
        _ => None,
    }
}

impl Rig {
    pub fn send(&mut self, phase: PointerPhase, x: f64, y: f64, shift: bool) {
        let i = PointerInput {
            phase,
            x,
            y,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            button: PointerButton::Primary,
            touch: false,
            space: false,
            at_ms: self.now,
        };
        self.event(i);
    }

    pub fn event(&mut self, i: PointerInput) {
        let rect = self.rect;
        let sel_rect = move || rect;
        let within = |r: Rect| -> Vec<String> {
            [("a", 0.0), ("b", 200.0)]
                .into_iter()
                .filter(|(_, x)| {
                    *x < r.x + r.w && r.x < x + 100.0 && 0.0 < r.y + r.h && r.y < 100.0
                })
                .map(|(id, _)| id.to_string())
                .collect()
        };
        let env = Env {
            tool: self.tool,
            camera: self.camera,
            selection: &self.selection,
            editing: false,
            halted: false,
            hit: &hit,
            selection_rect: &sel_rect,
            within: &within,
        };
        let acts = self.machine.step(&i, &env);
        for a in acts {
            match &a {
                Action::Select { ids } => self.selection = ids.clone(),
                Action::ToggleSelect { id } if self.selection.contains(id) => {
                    self.selection.retain(|s| s != id);
                }
                Action::ToggleSelect { id } => self.selection.push(id.clone()),
                Action::Camera { op } => {
                    let meldframe_app_model::CameraOp::Pan { dx, dy } = op else {
                        panic!("a pan")
                    };
                    self.camera.x += dx;
                    self.camera.y += dy;
                }
                _ => {}
            }
            self.log.push(a);
        }
    }

    pub fn down(&mut self, x: f64, y: f64) {
        self.send(PointerPhase::Down, x, y, false);
    }
    pub fn mv(&mut self, x: f64, y: f64) {
        self.send(PointerPhase::Move, x, y, false);
    }
    pub fn up(&mut self, x: f64, y: f64) {
        self.send(PointerPhase::Up, x, y, false);
    }

    pub fn json(&self) -> Vec<serde_json::Value> {
        self.log
            .iter()
            .map(|a| serde_json::to_value(a).expect("an action is JSON"))
            .collect()
    }
}

pub fn select(ids: &[&str]) -> serde_json::Value {
    serde_json::json!({"type": "select", "ids": ids})
}
