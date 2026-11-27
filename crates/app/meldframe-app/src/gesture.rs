//! Drawing a node and sizing one by a handle, as intents in world space (T8 of the thin UI audit, decision 43, Q3). The
//! geometry is `meldframe-app-rules` (`draw`, `handle`); this reads where the canvas draws things (the index of the scene,
//! `crate::canvas`), turns a drag into the action the file already has (`CreateNode`, `Resize`), and so a gesture is one
//! step of the history with the same checks, selection and tool change as a node made from a menu.
//!
//! Two queries sit beside the drop preview: [`App::draw_preview`] and [`App::resize_preview`]. They are made of the same
//! plan as the action and they check what the release would write against the limits of the file, so they answer none where
//! the release is refused. Neither changes the app.

use crate::action::Action;
use crate::app::App;
use crate::document::refuse;
use crate::effect::Effect;
use crate::state::Connection;
use meldframe_app_model::{CreateKind, GestureOp, Handle, Modifiers, Point};
use meldframe_app_parts::document::{code, new_node};
use meldframe_app_rules::draw::{DrawPreview, container_for, drawn_box};
use meldframe_app_rules::handle::{ResizePreview, resize_plan};
use meldframe_check::ApplyError;
use meldframe_layout::{Rect, is_layout_frame};
use meldframe_protocol::{Command, Node};
use meldframe_render::HitIndex;
use meldframe_sync::SubmitError;

/// A draw worked out: the frame, the position in its space and the size (none: the default of the kind).
struct DrawPlan {
    parent: Option<String>,
    at: (f64, f64),
    size: Option<(f64, f64)>,
    preview: DrawPreview,
}

/// A handle drag worked out: the box the node is drawn at, what the drag leaves, and the fields `Action::Resize` gets (none
/// when the drag changes nothing).
struct ResizePlan {
    preview: ResizePreview,
    change: Option<(f64, f64, Option<f64>, Option<f64>)>,
}

impl App {
    /// `Action::Gesture`: the intent becomes the action of the file that makes the change.
    pub(crate) fn gesture(&mut self, op: GestureOp, fx: &mut Vec<Effect>) {
        let planned = match op {
            GestureOp::Draw { kind, from, to, .. } => self.plan_draw(kind, from, to).map(|p| {
                Some(Action::CreateNode {
                    kind,
                    x: p.at.0,
                    y: p.at.1,
                    parent: p.parent,
                    w: p.size.map(|s| s.0),
                    h: p.size.map(|s| s.1),
                })
            }),
            GestureOp::ResizeHandle { id, handle, by, .. } => {
                self.plan_resize(&id, handle, by).map(|p| {
                    p.change
                        .map(|(w, h, x, y)| Action::Resize { id, w, h, x, y })
                })
            }
        };
        match planned {
            Ok(Some(action)) => self.document(action, fx),
            Ok(None) => {}
            Err(e) => drop(refuse(&mut self.events, code(&SubmitError::from(e)))),
        }
    }

    /// What drawing a node of `kind` from `from` to `to` (world points) would make: its frame, its box in the frame's space
    /// and on the canvas, and whether it is a click. None where the release is refused or dropped: a halted app, a point that is not a number, a node
    /// outside the limits of the file or a full queue of ops. Reads and changes nothing, and no state of the editor (not the
    /// selection, not the tool). Cost: the index of the scene is kept for a revision of the document, so a call while the
    /// pointer moves over a file that has not changed is a few passes over the boxes (`App::hit`), and the file is checked
    /// once against its limits.
    pub fn draw_preview(
        &self,
        kind: CreateKind,
        from: Point,
        to: Point,
        _mods: Modifiers,
    ) -> Option<DrawPreview> {
        if self.halted() {
            return None;
        }
        let p = self.plan_draw(kind, from, to).ok()?;
        let draft = new_node(
            &self.spare_id(),
            kind,
            p.at.0,
            p.at.1,
            p.parent,
            p.size.map(|s| s.0),
            p.size.map(|s| s.1),
        );
        let add = Command::AddNode {
            node: draft,
            index: None,
        };
        self.validate(&[add]).ok()?;
        Some(p.preview)
    }

    /// What dragging `handle` of node `id` by `by` (world units) would draw: the box the node is left at and whether its origin
    /// is the layout's (then the box keeps it). None for an unknown node and where the release is refused or dropped (a halted app, a number that
    /// is not one, a size outside the limits of the file). Reads and changes nothing. It is the box `GestureOp::ResizeHandle`
    /// writes, and the cost is that of [`App::draw_preview`].
    pub fn resize_preview(
        &self,
        id: &str,
        handle: Handle,
        by: Point,
        _mods: Modifiers,
    ) -> Option<ResizePreview> {
        if self.halted() {
            return None;
        }
        let p = self.plan_resize(id, handle, by).ok()?;
        if let Some((w, h, x, y)) = p.change {
            let cmd = self.resize_command(id, (w, h), (x, y)).ok()?;
            self.validate(&[cmd]).ok()?;
        }
        Some(p.preview)
    }

    /// A halted app drops every action that writes (`App::dispatch`), so what a release would make is nothing.
    fn halted(&self) -> bool {
        self.state.connection == Connection::Halted
    }

    /// A node id that no node of the file has, for a draft that is checked and never applied.
    fn spare_id(&self) -> String {
        let mut id = format!("{}-draft", self.prefix);
        while self.replica.nodes().iter().any(|n| n.id == id) {
            id.push('_');
        }
        id
    }

    fn plan_draw(&self, kind: CreateKind, from: Point, to: Point) -> Result<DrawPlan, ApplyError> {
        let drawn = drawn_box((from.x, from.y), (to.x, to.y))?;
        let nodes = self.replica.nodes();
        // The frame is the one at the press, `from`: where the drag ends decides the box and nothing else.
        let under = self.hit(from.x, from.y);
        let frame = container_for(nodes, under.as_deref());
        let parent = frame.map(|n| n.id.clone());
        let origin = parent
            .as_deref()
            .and_then(|p| self.world_rect(p))
            .map_or((0.0, 0.0), |r| (r.x, r.y));
        let (w, h) = drawn.size.unwrap_or_else(|| {
            let n = new_node("preview", kind, 0.0, 0.0, None, None, None);
            (n.w, n.h)
        });
        let world = Rect {
            x: drawn.x,
            y: drawn.y,
            w,
            h,
        };
        // The position is the pointer's minus the origin of the frame, as the web editor worked it out: the difference of two
        // numbers, which can be off by the rounding of the last digit when the frame is at a fractional place.
        let at = (drawn.x - origin.0, drawn.y - origin.1);
        Ok(DrawPlan {
            parent: parent.clone(),
            at,
            size: drawn.size,
            preview: DrawPreview {
                parent,
                local: Rect {
                    x: at.0,
                    y: at.1,
                    ..world
                },
                world,
                click: drawn.size.is_none(),
                placed_by_layout: self.canvas.enabled && frame.is_some_and(is_layout_frame),
            },
        })
    }

    /// The box the node is drawn at in its parent's space, as the scene holds it, and what the drag leaves of it.
    fn plan_resize(&self, id: &str, handle: Handle, by: Point) -> Result<ResizePlan, ApplyError> {
        let nodes = self.replica.nodes();
        let n = nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| ApplyError::NotFound(id.to_string()))?;
        let parent: Option<&Node> = n
            .parent_id
            .as_deref()
            .and_then(|p| nodes.iter().find(|x| x.id == p));
        let origin = parent
            .and_then(|p| self.world_rect(&p.id))
            .map_or((0.0, 0.0), |r| (r.x, r.y));
        // The box the canvas draws, as the scene holds it (so `x` is the number the file holds, plus the drag, exactly); the
        // stored one for a node the scene does not have (a file with a cycle).
        let from = self.local_rect(id).unwrap_or(Rect {
            x: n.x,
            y: n.y,
            w: n.w,
            h: n.h,
        });
        let drawn = self.layout_box(id);
        let preview = resize_plan(n, from, origin, (drawn, parent), (handle, by.x, by.y))?;
        let to = preview.local;
        // An origin is sent on the axis that moved only: the other keeps what it has.
        let change = (to != from).then(|| {
            (
                to.w,
                to.h,
                (to.x != from.x).then_some(to.x),
                (to.y != from.y).then_some(to.y),
            )
        });
        Ok(ResizePlan { preview, change })
    }

    /// The box of `id` on the canvas, as the scene draws it (the laid-out box with the flag on): the rectangle the hit rule
    /// reads, from the index `App::hit` keeps.
    fn world_rect(&self, id: &str) -> Option<Rect> {
        self.canvas.with_hits(
            self.replica.revision(),
            || self.with_scene(|_, boxes| HitIndex::from_boxes(boxes)),
            |index| index.rect_of(id),
        )
    }

    /// The box of `id` in the space of its parent, as the scene holds it (the numbers of the painted box, not a difference of
    /// two world boxes).
    fn local_rect(&self, id: &str) -> Option<Rect> {
        self.with_scene(|_, boxes| {
            boxes.iter().find(|b| b.id == id).map(|b| Rect {
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
            })
        })
    }
}
