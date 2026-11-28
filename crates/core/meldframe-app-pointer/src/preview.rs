//! What a front end draws while a gesture goes on, as data: the answer of `App::pointer_preview`.

use meldframe_app_model::{CreateKind, Handle, Point, PointerGesture};
use meldframe_app_rules::draw::DrawPreview;
use meldframe_app_rules::drop::Slot;
use meldframe_app_rules::handle::ResizePreview;
use meldframe_app_rules::multi::Preview;
use meldframe_layout::Rect;
use serde::Serialize;

/// What the gesture that is going on would do if it ended now. The file changes only when it ends.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PointerPreview {
    /// A drag of nodes. `follow` are the nodes drawn moving with the pointer by `(dx, dy)` (world units, since the press);
    /// `slot` is the place in a flow the flow children would land in, drawn as a line (the insertion line), and their own
    /// boxes stay where they are drawn.
    #[serde(rename_all = "camelCase")]
    Move {
        follow: Vec<String>,
        slot: Option<Slot>,
        dx: f64,
        dy: f64,
    },
    /// The rubber band, in world space, normalised to a positive size.
    Marquee { rect: Rect },
    /// The box a handle drag leaves the node at (`App::resize_preview`).
    Resize(ResizePreview),
    /// The box the node being drawn would have (`App::draw_preview`).
    Draw(DrawPreview),
}

/// The app's drop query: the ids dragged, where the pointer is and how far it travelled in, the drop rule's answer out.
type DropQuery<'a> = &'a dyn Fn(&[String], Point, Point) -> Option<Preview>;

/// The questions a preview asks of the app, each the app's own query for the same decision as the release.
pub struct Queries<'a> {
    /// `App::drop_preview_many`: the ids, where the pointer is and how far it travelled.
    pub drop: DropQuery<'a>,
    /// `App::resize_preview`: the node, the handle and how far it was dragged.
    pub resize: &'a dyn Fn(&str, Handle, Point) -> Option<ResizePreview>,
    /// `App::draw_preview`: the kind and the two points of the drag.
    pub draw: &'a dyn Fn(CreateKind, Point, Point) -> Option<DrawPreview>,
}

/// The preview of `gesture`; none while nothing is going on, and where the release would be refused (a resize or a draw that the
/// app's query refuses). A drag of nodes with no answer of the drop rule (the layout is not on the canvas) has every dragged node
/// follow the pointer; where the answer holds no slot only the nodes it names do.
pub fn preview(gesture: &PointerGesture, q: &Queries) -> Option<PointerPreview> {
    match gesture {
        PointerGesture::Idle | PointerGesture::Pan => None,
        PointerGesture::Move { ids, at, by } => {
            let (follow, slot) = match (q.drop)(ids, *at, *by) {
                None => (ids.clone(), None),
                Some(p) => (p.follow, p.slot),
            };
            Some(PointerPreview::Move {
                follow,
                slot,
                dx: by.x,
                dy: by.y,
            })
        }
        PointerGesture::Marquee { from, to } => Some(PointerPreview::Marquee {
            rect: Rect {
                x: from.x.min(to.x),
                y: from.y.min(to.y),
                w: (from.x - to.x).abs(),
                h: (from.y - to.y).abs(),
            },
        }),
        PointerGesture::Resize { id, handle, by } => {
            (q.resize)(id, *handle, *by).map(PointerPreview::Resize)
        }
        PointerGesture::Draw { kind, from, to } => {
            (q.draw)(*kind, *from, *to).map(PointerPreview::Draw)
        }
    }
}

impl std::fmt::Debug for Queries<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Queries").finish_non_exhaustive()
    }
}
