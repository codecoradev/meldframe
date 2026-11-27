//! Constructors of the actions that have optional parts.

use crate::action::Action;
use crate::camera::{CameraOp, ZoomCommand};
use crate::preset::CreateKind;

impl Action {
    /// A zoom command of the camera (`CameraOp::Zoom`): the menu, the palette and the zoom keys send it.
    pub fn zoom(command: ZoomCommand) -> Self {
        Self::Camera {
            op: CameraOp::Zoom { command },
        }
    }

    /// `CreateNode` with the default size.
    pub fn create_node(kind: CreateKind, x: f64, y: f64, parent: Option<String>) -> Self {
        Self::CreateNode {
            kind,
            x,
            y,
            parent,
            w: None,
            h: None,
        }
    }

    /// `Resize` that keeps the origin of the node.
    pub fn resize(id: impl Into<String>, w: f64, h: f64) -> Self {
        Self::Resize {
            id: id.into(),
            w,
            h,
            x: None,
            y: None,
        }
    }
}
