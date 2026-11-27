//! The modes of the pointer and what each event does in each.

use crate::{DRAG_THRESHOLD, Env};
use meldframe_app_model::camera::to_world;
use meldframe_app_model::{
    Action, CameraOp, Point, PointerButton, PointerCursor, PointerGesture, PointerInput,
    PointerPhase, PointerState, Tool,
};

#[derive(Debug, Default)]
enum Mode {
    #[default]
    Idle,
    /// The camera follows the pointer; `last` is where it was.
    Pan { last: (f64, f64) },
    /// Pressed and not yet moved past the threshold.
    Press(Press),
    /// A press on a node became a drag; `from` is the world point of the press.
    Move { ids: Vec<String>, from: Point },
}

#[derive(Debug)]
struct Press {
    /// The node under the press, none for empty canvas.
    id: Option<String>,
    was_selected: bool,
    additive: bool,
    start: (f64, f64),
    /// The world point of the press: a drag is measured from it, so a zoom in the middle of the drag moves nothing.
    from: Point,
    touch: bool,
}

/// The state machine. One per app; `view` is what a front end reads.
#[derive(Debug, Default)]
pub struct Machine {
    mode: Mode,
    view: PointerState,
    space: bool,
    /// Where the pointer was at the last event, none once it left.
    pos: Option<(f64, f64)>,
}

fn world(env: &Env, (x, y): (f64, f64)) -> Option<Point> {
    let p = to_world(&env.camera, Point { x, y });
    (p.x.is_finite() && p.y.is_finite()).then_some(p)
}

impl Machine {
    /// The hover, the cursor and the gesture as of the last event.
    pub fn view(&self) -> &PointerState {
        &self.view
    }

    /// Take a pointer event and say which actions it makes, in order. An event with a point that is not a number, or whose
    /// world point is not one, changes nothing, except `Cancel` and `Leave`: they do not read the point, and end the gesture.
    pub fn step(&mut self, i: &PointerInput, env: &Env) -> Vec<Action> {
        let here = (i.x, i.y);
        let mut acts = Vec::new();
        if matches!(i.phase, PointerPhase::Cancel | PointerPhase::Leave) {
            self.mode = Mode::Idle;
            self.view.hover = None;
            self.pos = (i.phase == PointerPhase::Cancel)
                .then_some(self.pos)
                .flatten();
        } else if i.x.is_finite() && i.y.is_finite() && world(env, here).is_some() {
            self.space = i.space;
            self.pos = Some(here);
            match i.phase {
                PointerPhase::Down => self.down(i, env, &mut acts),
                PointerPhase::Move => self.moved(here, env, &mut acts),
                _ => self.up(here, env, &mut acts),
            }
        } else {
            return acts;
        }
        self.show(env);
        acts
    }

    /// The event that shows the hover and the cursor again at the last point, for a change the pointer did not make (the tool or
    /// the selection): none while a gesture goes on, and none while the pointer is outside the canvas.
    pub fn refresh(&self) -> Option<PointerInput> {
        let (x, y) = self.pos.filter(|_| matches!(self.mode, Mode::Idle))?;
        Some(PointerInput {
            phase: PointerPhase::Move,
            x,
            y,
            modifiers: Default::default(),
            button: PointerButton::Primary,
            touch: false,
            space: self.space,
        })
    }

    /// A node of the file went: the hover forgets it, and a drag that holds it is over (what is let go would be refused).
    /// `exists` asks the file. True when the view changed.
    pub fn forget(&mut self, exists: &dyn Fn(&str) -> bool) -> bool {
        let before = self.view.clone();
        if self.view.hover.as_deref().is_some_and(|id| !exists(id)) {
            self.view.hover = None;
        }
        if matches!(&self.mode, Mode::Move { ids, .. } if ids.iter().any(|id| !exists(id))) {
            self.mode = Mode::Idle;
            self.view.gesture = PointerGesture::Idle;
        }
        self.view != before
    }

    /// The gesture and the cursor from the mode.
    fn show(&mut self, env: &Env) {
        self.view.cursor = match self.mode {
            Mode::Pan { .. } => PointerCursor::Grabbing,
            Mode::Idle if self.space => PointerCursor::Grab,
            Mode::Idle if env.tool != Tool::Select => PointerCursor::Crosshair,
            _ => PointerCursor::Default,
        };
        if !matches!(self.mode, Mode::Move { .. }) {
            self.view.gesture = match self.mode {
                Mode::Pan { .. } => PointerGesture::Pan,
                _ => PointerGesture::Idle,
            };
        }
    }

    fn down(&mut self, i: &PointerInput, env: &Env, acts: &mut Vec<Action>) {
        // A press while a gesture is going on means its release was lost: that gesture is over, and this one starts.
        self.mode = Mode::Idle;
        self.view = PointerState::default();
        let here = (i.x, i.y);
        if env.editing || i.button == PointerButton::Secondary {
            return;
        }
        if i.button == PointerButton::Middle || i.space {
            self.mode = Mode::Pan { last: here };
            return;
        }
        if env.tool != Tool::Select {
            return;
        }
        let Some(w) = world(env, here) else { return };
        let id = (env.hit)(w.x, w.y);
        let was_selected = id.as_ref().is_some_and(|id| env.selection.contains(id));
        let additive = i.modifiers.shift;
        if let (Some(id), false) = (&id, was_selected) {
            acts.push(if additive {
                Action::ToggleSelect { id: id.clone() }
            } else {
                Action::Select {
                    ids: vec![id.clone()],
                }
            });
        }
        self.mode = Mode::Press(Press {
            id,
            was_selected,
            additive,
            start: here,
            from: w,
            touch: i.touch,
        });
    }

    fn moved(&mut self, here: (f64, f64), env: &Env, acts: &mut Vec<Action>) {
        match &mut self.mode {
            Mode::Idle => self.hover(here, env),
            Mode::Pan { last } => {
                let (dx, dy) = (here.0 - last.0, here.1 - last.1);
                *last = here;
                acts.push(Action::Camera {
                    op: CameraOp::Pan { dx, dy },
                });
            }
            Mode::Press(p) => {
                if dist(here, p.start) < DRAG_THRESHOLD {
                    return;
                }
                let Some(id) = p.id.clone() else {
                    // A finger that drags on empty canvas pans; a mouse has nothing to drag there yet.
                    if p.touch {
                        self.mode = Mode::Pan { last: p.start };
                        self.moved(here, env, acts);
                    }
                    return;
                };
                if env.halted {
                    return;
                }
                let sel = env.selection;
                let ids = if p.was_selected {
                    sel.to_vec()
                } else if p.additive && !sel.contains(&id) {
                    [sel, std::slice::from_ref(&id)].concat()
                } else if p.additive {
                    sel.to_vec()
                } else {
                    vec![id]
                };
                self.mode = Mode::Move { ids, from: p.from };
                self.moved(here, env, acts);
            }
            Mode::Move { ids, from } => {
                let Some(at) = world(env, here) else { return };
                let by = Point {
                    x: at.x - from.x,
                    y: at.y - from.y,
                };
                self.view.gesture = PointerGesture::Move {
                    ids: ids.clone(),
                    at,
                    by,
                };
            }
        }
    }

    fn hover(&mut self, here: (f64, f64), env: &Env) {
        self.view.hover = match world(env, here) {
            Some(w) if env.tool == Tool::Select => (env.hit)(w.x, w.y),
            _ => None,
        };
    }

    fn up(&mut self, here: (f64, f64), env: &Env, acts: &mut Vec<Action>) {
        let mode = std::mem::take(&mut self.mode);
        self.view.gesture = PointerGesture::Idle;
        self.view.hover = None;
        match mode {
            Mode::Press(p) => match p.id {
                None if !p.additive => acts.push(Action::Select { ids: Vec::new() }),
                Some(id) if p.was_selected && p.additive => acts.push(Action::ToggleSelect { id }),
                Some(id) if p.was_selected && env.selection.len() > 1 => {
                    acts.push(Action::Select { ids: vec![id] });
                }
                _ => {}
            },
            Mode::Move { ids, from } => {
                if let Some(at) = world(env, here)
                    && (at.x != from.x || at.y != from.y)
                {
                    let (dx, dy) = (at.x - from.x, at.y - from.y);
                    acts.push(Action::DropNodes {
                        ids,
                        x: at.x,
                        y: at.y,
                        dx,
                        dy,
                    });
                }
            }
            Mode::Idle | Mode::Pan { .. } => {}
        }
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
