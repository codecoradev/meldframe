//! The PNG of a frame and the capture of a node, as plain data (finding T13 of the thin UI audit, decision 43). The app layer
//! decides the rectangle (from `PaintedBox::paint_bounds`), the scale, the size in pixels, the file name and the refusal;
//! the drawing and the delivery of the bytes (a download, the clipboard, the upload of a capture) are effects the shell carries
//! out, and what it reports comes back as a `Wire` answer that the app layer reads (`meldframe-app-parts`, `answers`). The
//! rules are in `meldframe-app-rules` (`raster`, `png`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The smallest and the largest scale of a PNG export.
pub const MIN_SCALE: f64 = 0.5;
pub const MAX_SCALE: f64 = 4.0;

/// The scales the Export section offers, smallest first: the largest of them that fits is what a refusal for size names.
pub const SCALES: [f64; 5] = [0.5, 1.0, 2.0, 3.0, 4.0];

/// The most pixels (width times height) of an exported image: 4096 by 4096. A copy of `CAPTURE.max_pixels` in
/// `apps/server/src/main.rs` (1 << 24), the area limit the server puts on a capture, so that an export and a capture of one
/// node agree; keep the two in step.
pub const EXPORT_MAX_PIXELS: u64 = 4096 * 4096;

/// The largest PNG a capture may upload, in bytes: 4 MiB. A copy of `CAPTURE.max_bytes` in `apps/server/src/main.rs` (the
/// capture relay's limit): the request carries the pixel limit but not this one, so the app layer keeps it; keep the two in
/// step.
pub const CAPTURE_MAX_BYTES: u64 = 4 << 20;

/// The smallest a PNG can be: the signature, the header chunk and the end chunk. Less is not an image.
pub const PNG_MIN_BYTES: u64 = 33;

/// A rectangle in world space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RasterRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The size of an image at a scale, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RasterSize {
    pub scale: f64,
    pub width: u32,
    pub height: u32,
}

/// What a PNG export of a frame would be now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PngPlan {
    /// It fits: the world rectangle (the frame with its outside border and its shadows), the scale, the size of the image
    /// (each side the rectangle's times the scale, rounded up) and the file to save it under, safe to use as it is.
    Ready {
        rect: RasterRect,
        scale: f64,
        width: u32,
        height: u32,
        file: String,
    },
    /// There is none: `code` is `node_not_found` (the frame is gone or has no size), `too_large` (`asked` is the size it
    /// would have and `fits` the largest offered scale that stays inside the limit, none when not even the smallest does) or
    /// `invalid_scale`.
    Refused {
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        asked: Option<RasterSize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        fits: Option<RasterSize>,
    },
}

/// The scale of the PNG export and the plan for the frame the section acts on and for the frame of the open dialog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PngState {
    /// 1 until `PngOp::SetScale` says another.
    pub scale: f64,
    /// For `ExportState::target`, `None` while there is none.
    pub target: Option<PngPlan>,
    /// For the frame of `ExportState::dialog`, `None` while it is closed.
    pub dialog: Option<PngPlan>,
}

impl Default for PngState {
    fn default() -> Self {
        PngState {
            scale: 1.0,
            target: None,
            dialog: None,
        }
    }
}

/// Where a person's PNG goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PngSink {
    Download,
    Clipboard,
}

/// Where the bytes of a raster go (`Effect::Raster`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RasterSink {
    /// Save them as a file of `Effect::Raster::file`.
    Download {},
    /// Put them on the clipboard as an image.
    Clipboard {},
    /// `POST /captures/{request_id}` with the bytes as the body and the headers `X-Capture-Token: {upload_token}`,
    /// `X-Capture-Seq: {seq}` and `Content-Type: image/png`, as the person's own session.
    Upload {
        request_id: String,
        upload_token: String,
        #[ts(type = "number")]
        seq: u64,
    },
}

/// An agent's request for an image of a node, as the `captureRequest` frame carries it (read from `Wire::Capture`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureAsk {
    pub request_id: String,
    /// The node, or none for the whole page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node_id: Option<String>,
    pub scale: f64,
    #[ts(type = "number")]
    pub max_pixels: u64,
    /// The image must show the file at this sequence number or later.
    #[ts(type = "number")]
    pub min_seq: u64,
    pub upload_token: String,
    #[ts(type = "number")]
    pub timeout_ms: u64,
}

/// A step of the PNG export or of a capture (`Action::Png`). Every one is view state: nothing is written to the file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PngOp {
    /// Choose the scale of the PNG export. A scale that is not one of `SCALES` (0.5, 1, 2, 3, 4) is refused as `invalid_scale` and the
    /// scale stays.
    SetScale { scale: f64 },
    /// Export the frame `id` (the one `ExportState::target` names, or the frame of the open dialog) at the chosen scale:
    /// `Effect::Raster` when the plan is ready, else `Event::Refused` with the plan's code (`out_of_sync` while the app is
    /// halted, `not_exportable` for another frame).
    Export { id: String, to: PngSink },
    /// An agent asks for an image (`now_ms` is the shell's clock). The app refuses it at once (`Effect::FailCapture`) when
    /// the scale is not above 0 and at most 4 (`invalid_scale`), the node is gone or has no size (`node_not_found`) or the
    /// image would be over the pixel limit (`too_large`); it waits while the file is behind `min_seq`, up to the timeout
    /// (`stale_client`); else it asks for the drawing (`Effect::Raster`).
    Capture {
        #[ts(type = "number")]
        now_ms: u64,
        ask: CaptureAsk,
    },
    /// The shell drew the image of `request` and holds the bytes: their size. The app checks it against the plan and the
    /// limits and answers with `Effect::FinishRaster`.
    Drawn {
        request: u32,
        width: u32,
        height: u32,
        #[ts(type = "number")]
        bytes: u64,
    },
    /// The shell could not draw the image of `request`.
    DrawFailed { request: u32, code: String },
    /// The bytes of `request` reached their sink.
    Delivered { request: u32 },
    /// The sink refused the bytes of `request` (`code` is what its answer said).
    DeliveryFailed { request: u32, code: String },
}
