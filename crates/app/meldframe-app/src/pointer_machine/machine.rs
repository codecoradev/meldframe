//! The modes of the pointer and what each event does in each.

use crate::pointer_machine::screen::{
    Screen, dist, handle_under, kind_of, resize_cursor, travel, world,
};
use crate::pointer_machine::{DOUBLE_CLICK_MS, DOUBLE_CLICK_PX, DRAG_THRESHOLD, Env};
use meldframe_app_model::{
    Action, CameraOp, CreateKind, GestureOp, Handle, Point, PointerButton, PointerCursor,
    PointerGesture, PointerInput, PointerPhase, PointerState, TextEditOp, Tool,
};
use meldframe_layout::Rect;

#[derive(Debug, Default)]
enum Mode {
    #[default]
    Idle,
    /// The camera follows the pointer; `last` is where it was.
    Pan { last: Screen },
    /// Pressed and not yet moved past the threshold.
    Press(Press),
    /// A press on a node became a drag; `from` is the world point of the press.
    Move {
        ids: Vec<String>,
        from: Point,
        last: Screen,
    },
    /// A press on empty canvas became a drag.
    Marquee {
        from: Point,
        last: Screen,
        additive: bool,
    },
    /// A press on a handle of the selection box. `id` is the node when exactly one is selected: the handles of several nodes do
    /// nothing yet (the press is still the handle's, and selects nothing).
    Resize {
        id: Option<String>,
        handle: Handle,
        start: Screen,
        from: Point,
        last: Screen,
        began: bool,
    },
    /// A press with a drawing tool.
    Draw {
        kind: CreateKind,
        start: Screen,
        from: Point,
        last: Screen,
        began: bool,
    },
}

#[derive(Debug)]
struct Press {
    /// The node under the press, none for empty canvas.
    id: Option<String>,
    was_selected: bool,
    additive: bool,
    start: Screen,
    /// The world point of the press: a drag is measured from it, so a zoom in the middle of the drag moves nothing.
    from: Point,
    touch: bool,
    /// The clock at the press, for a double click.
    at_ms: Option<u64>,
}

/// A click that may be the first of a double click: where, when and on which node.
#[derive(Debug)]
struct Click {
    at: Screen,
    ms: u64,
    id: String,
}

/// The state machine. One per app; `view` is what a front end reads.
#[derive(Debug, Default)]
pub struct Machine {
    mode: Mode,
    view: PointerState,
    space: bool,
    /// The handle of the selection box under the pointer while nothing is going on.
    handle: Option<Handle>,
    /// Where the pointer was at the last event, none once it left.
    pos: Option<Screen>,
    /// The last click, until it is a double click or something else happens.
    click: Option<Click>,
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
            self.handle = None;
            self.click = None;
            self.view.hover = None;
            if i.phase == PointerPhase::Leave {
                self.pos = None;
            }
        } else if i.x.is_finite() && i.y.is_finite() && world(env, here).is_some() {
            self.space = i.space;
            self.pos = Some(here);
            match i.phase {
                PointerPhase::Down => self.down(i, env, &mut acts),
                PointerPhase::Move => self.moved(here, env, &mut acts),
                _ => self.up(here, i, env, &mut acts),
            }
        } else {
            return acts;
        }
        self.show(env);
        acts
    }

    /// Whether a press is down: from the press, before the drag has begun, to the release. What Escape and an arrow key ask: the
    /// gesture of `view` is `Idle` until a press has moved past the threshold, and a press can be abandoned before that.
    pub fn busy(&self) -> bool {
        !matches!(self.mode, Mode::Idle)
    }

    /// The event that shows the hover, the handle and the cursor again at the last point, for a change the pointer did not make
    /// (the tool or the selection): none while a gesture goes on, and none while the pointer is outside the canvas.
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
            at_ms: None,
        })
    }

    /// A node of the file went: the hover forgets it, and a drag that holds it is over (what is let go would be refused).
    /// `exists` asks the file. True when the view changed.
    pub fn forget(&mut self, exists: &dyn Fn(&str) -> bool) -> bool {
        let before = self.view.clone();
        if self.view.hover.as_deref().is_some_and(|id| !exists(id)) {
            self.view.hover = None;
        }
        let held = match &self.mode {
            Mode::Move { ids, .. } => ids.iter().any(|id| !exists(id)),
            Mode::Resize { id: Some(id), .. } => !exists(id),
            _ => false,
        };
        if held {
            self.mode = Mode::Idle;
            self.view.gesture = PointerGesture::Idle;
        }
        self.view != before
    }

    /// The gesture and the cursor from the mode.
    fn show(&mut self, env: &Env) {
        self.view.cursor = match (&self.mode, self.handle.as_ref()) {
            (Mode::Pan { .. }, _) => PointerCursor::Grabbing,
            (Mode::Resize { handle, .. }, _) | (Mode::Idle, Some(handle)) => resize_cursor(*handle),
            (Mode::Draw { .. }, _) => PointerCursor::Crosshair,
            (Mode::Idle, None) if self.space => PointerCursor::Grab,
            (Mode::Idle, None) if env.tool != Tool::Select => PointerCursor::Crosshair,
            _ => PointerCursor::Default,
        };
        self.view.gesture = self.gesture(env).unwrap_or_default();
    }

    fn gesture(&self, env: &Env) -> Option<PointerGesture> {
        Some(match &self.mode {
            Mode::Pan { .. } => PointerGesture::Pan,
            Mode::Move { ids, from, last } => {
                let at = world(env, *last)?;
                PointerGesture::Move {
                    ids: ids.clone(),
                    at,
                    by: travel(*from, at),
                }
            }
            Mode::Marquee { from, last, .. } => PointerGesture::Marquee {
                from: *from,
                to: world(env, *last)?,
            },
            Mode::Resize {
                id: Some(id),
                handle,
                from,
                last,
                began: true,
                ..
            } => PointerGesture::Resize {
                id: id.clone(),
                handle: *handle,
                by: travel(*from, world(env, *last)?),
            },
            Mode::Draw {
                kind,
                from,
                last,
                began: true,
                ..
            } => PointerGesture::Draw {
                kind: *kind,
                from: *from,
                to: world(env, *last)?,
            },
            _ => return None,
        })
    }

    fn down(&mut self, i: &PointerInput, env: &Env, acts: &mut Vec<Action>) {
        // A press while a gesture is going on means its release was lost: that gesture is over, and this one starts.
        self.mode = Mode::Idle;
        self.view = PointerState::default();
        self.handle = None;
        // Only a press that stays a click can be the second of a double click: the others forget the first.
        let click = self.click.take();
        let here = (i.x, i.y);
        let Some(w) = world(env, here) else { return };
        if env.editing || i.button == PointerButton::Secondary {
            return;
        }
        if i.button == PointerButton::Middle || i.space {
            self.mode = Mode::Pan { last: here };
            return;
        }
        if let Some(kind) = kind_of(env.tool) {
            if !env.halted {
                self.mode = Mode::Draw {
                    kind,
                    start: here,
                    from: w,
                    last: here,
                    began: false,
                };
            }
            return;
        }
        if let Some(handle) = handle_under(env, here) {
            if !env.halted {
                let id = (env.selection.len() == 1).then(|| env.selection[0].clone());
                self.mode = Mode::Resize {
                    id,
                    handle,
                    start: here,
                    from: w,
                    last: here,
                    began: false,
                };
            }
            return;
        }
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
        self.click = click;
        self.mode = Mode::Press(Press {
            id,
            was_selected,
            additive,
            start: here,
            from: w,
            touch: i.touch,
            at_ms: i.at_ms,
        });
    }

    fn moved(&mut self, here: Screen, env: &Env, acts: &mut Vec<Action>) {
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
                let start = p.start;
                self.click = None;
                let Some(id) = p.id.clone() else {
                    // A finger that drags on empty canvas pans; a mouse draws a marquee.
                    self.mode = if p.touch {
                        Mode::Pan { last: start }
                    } else {
                        Mode::Marquee {
                            from: p.from,
                            last: start,
                            additive: p.additive,
                        }
                    };
                    return self.moved(here, env, acts);
                };
                if env.halted {
                    return;
                }
                let sel = env.selection;
                let ids = if p.was_selected || (p.additive && sel.contains(&id)) {
                    sel.to_vec()
                } else if p.additive {
                    [sel, std::slice::from_ref(&id)].concat()
                } else {
                    vec![id]
                };
                self.mode = Mode::Move {
                    ids,
                    from: p.from,
                    last: start,
                };
                self.moved(here, env, acts);
            }
            Mode::Move { last, .. } | Mode::Marquee { last, .. } => *last = here,
            Mode::Resize {
                start, last, began, ..
            }
            | Mode::Draw {
                start, last, began, ..
            } => {
                if *began || dist(here, *start) >= DRAG_THRESHOLD {
                    *began = true;
                    *last = here;
                }
            }
        }
    }

    fn hover(&mut self, here: Screen, env: &Env) {
        self.handle = None;
        self.view.hover = None;
        let Some(w) = world(env, here) else { return };
        if env.tool == Tool::Select {
            self.handle = handle_under(env, here);
            if self.handle.is_none() {
                self.view.hover = (env.hit)(w.x, w.y);
            }
        }
    }

    /// A press let go without a drag: the second of a double click begins a text edit on the node, any other is remembered as the
    /// first. A click with no clock, or on empty canvas, is nobody's first.
    fn clicked(&mut self, p: Press, acts: &mut Vec<Action>) {
        let (Some(id), Some(ms)) = (p.id, p.at_ms) else {
            self.click = None;
            return;
        };
        let double = self.click.take().is_some_and(|c| {
            c.id == id
                && ms >= c.ms
                && ms - c.ms <= DOUBLE_CLICK_MS
                && dist(c.at, p.start) <= DOUBLE_CLICK_PX
        });
        if double {
            acts.push(Action::TextEdit {
                op: TextEditOp::Begin { id },
            });
        } else {
            self.click = Some(Click {
                at: p.start,
                ms,
                id,
            });
        }
    }

    fn up(&mut self, here: Screen, i: &PointerInput, env: &Env, acts: &mut Vec<Action>) {
        let mode = std::mem::take(&mut self.mode);
        self.view.hover = None;
        let mods = i.modifiers;
        match mode {
            Mode::Press(p) => {
                match &p.id {
                    None if !p.additive => acts.push(Action::Select { ids: Vec::new() }),
                    Some(id) if p.was_selected && p.additive => {
                        acts.push(Action::ToggleSelect { id: id.clone() });
                    }
                    Some(id) if p.was_selected && env.selection.len() > 1 => {
                        acts.push(Action::Select {
                            ids: vec![id.clone()],
                        });
                    }
                    _ => {}
                }
                self.clicked(p, acts);
            }
            Mode::Move { ids, from, .. } => {
                if let Some(at) = world(env, here) {
                    let by = travel(from, at);
                    if by.x != 0.0 || by.y != 0.0 {
                        acts.push(Action::DropNodes {
                            ids,
                            x: at.x,
                            y: at.y,
                            dx: by.x,
                            dy: by.y,
                        });
                    }
                }
            }
            Mode::Marquee { from, additive, .. } => {
                if let (a, Some(b)) = (from, world(env, here)) {
                    let rect = Rect {
                        x: a.x.min(b.x),
                        y: a.y.min(b.y),
                        w: (a.x - b.x).abs(),
                        h: (a.y - b.y).abs(),
                    };
                    let mut ids = if additive {
                        env.selection.to_vec()
                    } else {
                        Vec::new()
                    };
                    let fresh: Vec<String> = (env.within)(rect)
                        .into_iter()
                        .filter(|id| !ids.contains(id))
                        .collect();
                    let added = fresh.len();
                    ids.extend(fresh);
                    if !additive || added > 0 {
                        acts.push(Action::Select { ids });
                    }
                }
            }
            Mode::Resize {
                id: Some(id),
                handle,
                from,
                began: true,
                ..
            } => {
                if let Some(by) = world(env, here)
                    .map(|at| travel(from, at))
                    .filter(|b| b.x != 0.0 || b.y != 0.0)
                {
                    acts.push(Action::Gesture {
                        op: GestureOp::ResizeHandle {
                            id,
                            handle,
                            by,
                            mods,
                        },
                    });
                }
            }
            Mode::Draw {
                kind, from, began, ..
            } => {
                let to = world(env, here).filter(|_| began).unwrap_or(from);
                acts.push(Action::Gesture {
                    op: GestureOp::Draw {
                        kind,
                        from,
                        to,
                        mods,
                    },
                });
            }
            Mode::Idle | Mode::Pan { .. } | Mode::Resize { .. } => {}
        }
    }
}
