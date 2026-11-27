//! The requests for an image that are out (finding T13 of the thin UI audit, decision 43): one job for each request, from the
//! plan to the bytes reaching their sink. A table with no `App` in it: a job is taken out of it by the first answer for its
//! stage, so a repeated or late answer finds nothing, and every request is answered once. What decides each step is
//! `png`.

use crate::raster::{Planned, is_capture_scale, is_export_scale};
use meldframe_app_model::{EXPORT_MAX_PIXELS, PngSink, RasterSink};
use std::fmt;

/// The longest a capture waits for the file, whatever the request says: the server gives an editor 20 seconds.
pub const MAX_WAIT_MS: u64 = 60_000;

/// The most captures that wait or are being made at once (the server holds 4 for one editor).
pub const MAX_CAPTURES: usize = 8;

/// What the image is for.
pub enum Kind {
    /// A PNG export of a person: where it goes and the name to save it under.
    Export { sink: PngSink, file: String },
    /// An agent's capture: the request it answers, the secret that lets the upload in, and the file number it waits for.
    Capture {
        request_id: String,
        token: String,
        min_seq: u64,
    },
}

/// The upload token is a secret (`App` is `Debug`, and a log must not carry it): it prints as `<redacted>`, as the server's does.
impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Export { sink, file } => f
                .debug_struct("Export")
                .field("sink", sink)
                .field("file", file)
                .finish(),
            Kind::Capture {
                request_id,
                min_seq,
                ..
            } => f
                .debug_struct("Capture")
                .field("request_id", request_id)
                .field("token", &"<redacted>")
                .field("min_seq", min_seq)
                .finish(),
        }
    }
}

/// Where a request is: waiting (a capture, for the file to reach `min_seq`), being drawn, or its bytes on their way to the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Waiting,
    Drawing,
    Delivering,
}

#[derive(Debug)]
pub struct Job {
    pub request: u32,
    pub kind: Kind,
    /// The node, or none for the whole page.
    pub node: Option<String>,
    pub scale: f64,
    pub planned: Planned,
    pub max_pixels: u64,
    pub stage: Stage,
    /// The clock at which a capture is given up on; an export has none (a new one replaces it).
    pub deadline: Option<u64>,
}

impl Job {
    /// Where the bytes go; `seq` is the file number the image is of (the upload says so).
    pub fn sink(&self, seq: u64) -> RasterSink {
        match &self.kind {
            Kind::Export {
                sink: PngSink::Download,
                ..
            } => RasterSink::Download {},
            Kind::Export {
                sink: PngSink::Clipboard,
                ..
            } => RasterSink::Clipboard {},
            Kind::Capture {
                request_id, token, ..
            } => RasterSink::Upload {
                request_id: request_id.clone(),
                upload_token: token.as_str().into(),
                seq,
            },
        }
    }

    /// The name to save a download under; none for an upload.
    pub fn file(&self) -> Option<String> {
        match &self.kind {
            Kind::Export { file, .. } => Some(file.clone()),
            Kind::Capture { .. } => None,
        }
    }

    /// The scales the request may ask for and the most pixels it may have.
    pub fn limits(&self) -> (fn(f64) -> bool, u64) {
        match self.kind {
            Kind::Export { .. } => (is_export_scale, EXPORT_MAX_PIXELS),
            Kind::Capture { .. } => (is_capture_scale, self.max_pixels),
        }
    }
}

/// The jobs, the number of the next request and the latest clock the shell gave (a capture request, a tick, a frame): a
/// capture past its deadline is not drawn.
#[derive(Debug, Default)]
pub struct Jobs {
    next: u32,
    jobs: Vec<Job>,
    clock: u64,
}

impl Jobs {
    pub fn number(&mut self) -> u32 {
        self.next = self.next.wrapping_add(1);
        self.next
    }

    /// The clock moved on (it never goes back).
    pub fn tick(&mut self, now_ms: u64) {
        self.clock = self.clock.max(now_ms);
    }

    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// How many captures wait or are being made.
    pub fn captures(&self) -> usize {
        (self.jobs.iter())
            .filter(|j| matches!(j.kind, Kind::Capture { .. }))
            .count()
    }

    /// Whether a job waits for the file.
    pub fn waits(&self) -> bool {
        self.jobs.iter().any(|j| j.stage == Stage::Waiting)
    }

    /// Whether the capture request `id` is already a job.
    pub fn has_capture(&self, id: &str) -> bool {
        (self.jobs.iter())
            .any(|j| matches!(&j.kind, Kind::Capture { request_id, .. } if request_id == id))
    }

    pub fn push(&mut self, job: Job) {
        self.jobs.push(job);
    }

    /// The job of `request` when it is at `stage`, taken out of the table.
    pub fn take(&mut self, request: u32, stage: Stage) -> Option<Job> {
        let at = (self.jobs.iter()).position(|j| j.request == request && j.stage == stage)?;
        Some(self.jobs.remove(at))
    }

    /// Every job `pick` says yes to, taken out of the table in the order they were made.
    pub fn take_where(&mut self, pick: impl Fn(&Job) -> bool) -> Vec<Job> {
        let (taken, kept) = std::mem::take(&mut self.jobs).into_iter().partition(pick);
        self.jobs = kept;
        taken
    }
}

/// The sentence a refused capture carries to the agent for a code.
pub fn message(code: &str) -> &'static str {
    match code {
        "invalid_scale" => "The scale must be above 0 and at most 4.",
        "node_not_found" => "There is no such node on the canvas.",
        "too_large" => "The image would be larger than the limit.",
        "stale_client" => "The editor has not received the file up to that change yet.",
        "node_changed" => "The node changed while the editor drew it.",
        "busy" => "The editor has too many images to make.",
        _ => "The image could not be made.",
    }
}
