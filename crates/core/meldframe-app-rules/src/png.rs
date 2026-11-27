//! The PNG export of a frame and an agent's capture as state transitions (finding T13 of the thin UI audit, decision 43): the scale, the plan for the
//! frame of the section and of the open dialog (the rectangle with the reach of its paint, the size in pixels, the file name,
//! or why there is none), the export itself as an `Effect::Raster` and what comes of it: the shell reports the image it drew
//! (`Wire::Drawn`), the app checks it against the plan and the limits and the node as it is now, and the shell is told to
//! deliver the bytes or to drop them (`Effect::FinishRaster`); a request is answered once. A capture request is decided at
//! once (the scale, the node, the pixel limit), waits for the file to reach `min_seq` until its timeout, plans again when the
//! wait ends and ends in the upload or in `Effect::FailCapture`, never in silence. View state: nothing is written to the file and the undo
//! history does not change. `meldframe-app` hands each step the scene, the state of the export and where the effects and the
//! events go (`Env`); the pure rules are in `raster`, and the drawing and the save are the shell's.

use crate::code::code as safe;
use crate::jobs::{Job, Jobs, Kind, MAX_CAPTURES, MAX_WAIT_MS, Stage, message};
use crate::raster::{
    Planned, Refusal, Scene, drawn_refusal, is_capture_scale, is_export_scale, plan, png_plan,
};
use meldframe_app_model::{
    CAPTURE_MAX_BYTES, CaptureAsk, EXPORT_MAX_PIXELS, Effect, Event, ExportState, PngOp, PngPlan,
    PngSink,
};

/// What the plans of the export were worked out from: the epoch of the scene (the document, the measurer, the flag and the
/// modes), the scale and the two frames. While it is the same, so are the plans.
#[derive(Debug, Clone, PartialEq)]
struct PlanKey {
    epoch: u64,
    scale: f64,
    /// A frame and the stem of its files.
    target: Option<(String, String)>,
    dialog: Option<(String, String)>,
}

impl PlanKey {
    fn of(epoch: u64, export: &ExportState) -> Self {
        let frame = |id: &str, stem: &str| (id.to_owned(), stem.to_owned());
        PlanKey {
            epoch,
            scale: export.png.scale,
            target: (export.target.as_ref()).map(|t| frame(&t.id, &t.file_stem)),
            dialog: (export.dialog.as_ref()).map(|d| frame(&d.frame, &d.file_stem)),
        }
    }
}

/// What a step reads and writes besides the scene: the state of the export, whether the app is halted, and where the effects
/// and the events it raises go.
#[derive(Debug)]
pub struct Env<'a> {
    pub halted: bool,
    /// The file number the app has confirmed.
    pub seq: u64,
    pub export: &'a mut ExportState,
    pub fx: &'a mut Vec<Effect>,
    pub events: &'a mut Vec<Event>,
}

impl Env<'_> {
    /// The same environment for a step that is not the last of the call.
    fn again(&mut self) -> Env<'_> {
        Env {
            halted: self.halted,
            seq: self.seq,
            export: &mut *self.export,
            fx: &mut *self.fx,
            events: &mut *self.events,
        }
    }

    fn refuse(&mut self, code: &str) {
        self.events.push(Event::Refused {
            code: code.to_owned(),
        });
    }
}

/// The plans of the PNG export, and the requests for an image that are out.
#[derive(Debug, Default)]
pub struct Rasters {
    key: Option<PlanKey>,
    jobs: Jobs,
}

impl Rasters {
    /// How many captures wait or are being made.
    pub fn captures(&self) -> usize {
        self.jobs.captures()
    }

    /// Whether a capture waits for the file to reach its number.
    pub fn waits(&self) -> bool {
        self.jobs.waits()
    }

    /// Whether the plans are not the ones the scene of `epoch`, the scale, the target and the open dialog call for. Asking
    /// costs a comparison, and nothing is worked out while no frame is the target and no dialog is open.
    pub fn is_stale(&self, epoch: u64, export: &ExportState) -> bool {
        self.key.as_ref() != Some(&PlanKey::of(epoch, export))
    }

    /// Bring the plans in line with the selection, the open dialog, the scale and the document (`ExportChanged` when one moved).
    /// There is no scene while no frame is the target and no dialog is open: nothing is planned then.
    pub fn refresh(&mut self, epoch: u64, scene: Option<Scene>, env: Env) {
        let key = PlanKey::of(epoch, env.export);
        let plan = |f: &Option<(String, String)>| {
            (f.as_ref()).and_then(|(id, stem)| Some(png_plan(scene?, id, stem, key.scale)))
        };
        let (target, dialog) = (plan(&key.target), plan(&key.dialog));
        self.key = Some(key);
        let png = &mut env.export.png;
        if png.target != target || png.dialog != dialog {
            png.target = target;
            png.dialog = dialog;
            env.events.push(Event::ExportChanged {});
        }
    }

    pub fn apply(&mut self, op: PngOp, scene: Scene, mut env: Env) {
        match op {
            PngOp::SetScale { scale } if !is_export_scale(scale) => env.refuse("invalid_scale"),
            PngOp::SetScale { scale } => {
                if env.export.png.scale != scale {
                    env.export.png.scale = scale;
                    // The plans follow in `refresh`, and say so when they changed.
                    env.events.push(Event::ExportChanged {});
                }
            }
            PngOp::Export { id, to } => self.export(&id, to, scene, env),
            PngOp::Capture { now_ms, ask } => self.capture(now_ms, ask, scene, env),
            PngOp::Drawn {
                request,
                width,
                height,
                bytes,
            } => self.drawn(request, (width, height, bytes), scene, env),
            PngOp::DrawFailed { request, code } => {
                if let Some(job) = self.jobs.take(request, Stage::Drawing) {
                    close(job, &safe(&code), false, env.fx, env.events);
                }
            }
            PngOp::Delivered { request } => drop(self.jobs.take(request, Stage::Delivering)),
            PngOp::DeliveryFailed { request, code } => {
                if let Some(job) = self.jobs.take(request, Stage::Delivering) {
                    close(job, &safe(&code), false, env.fx, env.events);
                }
            }
        }
    }

    /// The PNG of frame `id`, which is the target of the section or the frame of the open dialog.
    fn export(&mut self, id: &str, to: PngSink, scene: Scene, mut env: Env) {
        let export = &*env.export;
        let stem = (export.target.as_ref())
            .filter(|t| t.id == id)
            .map(|t| &t.file_stem)
            .or_else(|| {
                (export.dialog.as_ref())
                    .filter(|d| d.frame == id)
                    .map(|d| &d.file_stem)
            });
        // A halted app may hold a view that is out of date: it exports nothing (as `OpenExport`).
        let plan = match stem {
            _ if env.halted => return env.refuse("out_of_sync"),
            None => return env.refuse("not_exportable"),
            Some(stem) => png_plan(scene, id, stem, export.png.scale),
        };
        let PngPlan::Ready {
            rect,
            scale,
            width,
            height,
            file,
        } = plan
        else {
            let PngPlan::Refused { code, .. } = plan else {
                unreachable!("a plan is ready or refused")
            };
            return env.refuse(&code);
        };
        // A new export replaces the one still being made; the shell drops the bytes it holds for it.
        for old in (self.jobs).take_where(|j| matches!(j.kind, Kind::Export { .. })) {
            env.fx.push(Effect::FinishRaster {
                request: old.request,
                deliver: false,
            });
        }
        let job = Job {
            request: self.jobs.number(),
            kind: Kind::Export { sink: to, file },
            node: Some(id.to_owned()),
            scale,
            planned: Planned {
                rect,
                width,
                height,
            },
            max_pixels: EXPORT_MAX_PIXELS,
            stage: Stage::Waiting,
            deadline: None,
        };
        self.draw(job, env.seq, env.fx);
    }

    /// Ask the shell to draw the job's image (`Effect::Raster`) and wait for its report.
    fn draw(&mut self, mut job: Job, seq: u64, fx: &mut Vec<Effect>) {
        fx.push(Effect::Raster {
            request: job.request,
            rect: job.planned.rect,
            scale: job.scale,
            width: job.planned.width,
            height: job.planned.height,
            file: job.file(),
            to: job.sink(seq),
        });
        job.stage = Stage::Drawing;
        self.jobs.push(job);
    }

    /// An agent's request for an image (`now_ms` is the shell's clock). It is ignored when it is a request already held
    /// (the same one again is the same one), refused at once when the app is halted (`stale_client`), when too many are
    /// being made (`busy`) or when it cannot be drawn (`invalid_scale`, `node_not_found`, `too_large`); it waits while the file
    /// is behind `min_seq`, up to the timeout, else the shell is asked to draw it.
    fn capture(&mut self, now_ms: u64, ask: CaptureAsk, scene: Scene, mut env: Env) {
        // The clock is the latest the app has seen (a shell's clock that went back changes nothing), and a request that is
        // out of time ends before a new one is taken.
        self.expire(now_ms, env.again());
        let now = self.jobs.clock();
        if self.jobs.has_capture(&ask.request_id) {
            return;
        }
        let fail = |env: Env, code: &str| {
            env.fx.push(Effect::FailCapture {
                request_id: ask.request_id.clone(),
                code: code.to_owned(),
                message: message(code).to_owned(),
            })
        };
        if env.halted {
            return fail(env, "stale_client");
        }
        if self.jobs.captures() >= MAX_CAPTURES {
            return fail(env, "busy");
        }
        let node = ask.node_id.as_deref();
        // The request's own limit may be lower; it is never above what an export of the same node is allowed.
        let max_pixels = ask.max_pixels.min(EXPORT_MAX_PIXELS);
        let planned = match plan(scene, node, ask.scale, is_capture_scale, max_pixels) {
            Ok(p) => p,
            Err(refusal) => return fail(env, refusal.code()),
        };
        let wait = ask.timeout_ms.min(MAX_WAIT_MS);
        let job = Job {
            request: self.jobs.number(),
            kind: Kind::Capture {
                request_id: ask.request_id.clone(),
                token: ask.upload_token.expose().to_owned(),
                min_seq: ask.min_seq,
            },
            node: ask.node_id,
            scale: ask.scale,
            planned,
            max_pixels,
            stage: Stage::Waiting,
            deadline: Some(now.saturating_add(wait)),
        };
        if env.seq >= ask.min_seq {
            self.draw(job, env.seq, env.fx);
        } else {
            self.jobs.push(job);
            env.fx.push(Effect::WakeAfter { after_ms: wait });
        }
    }

    /// The file moved: the captures that waited for it plan again on what is there now and go on, or end (one whose time was
    /// up while it waited is not drawn).
    pub fn resume(&mut self, scene: Scene, env: Env) {
        let (seq, clock) = (env.seq, self.jobs.clock());
        let ready = |j: &Job| {
            j.stage == Stage::Waiting
                && matches!(j.kind, Kind::Capture { min_seq, .. } if min_seq <= seq)
        };
        for mut job in self.jobs.take_where(ready) {
            if job.deadline.is_some_and(|d| d <= clock) {
                close(job, "stale_client", false, &mut *env.fx, &mut *env.events);
                continue;
            }
            match replan(&job, scene) {
                Ok(planned) => {
                    job.planned = planned;
                    self.draw(job, seq, &mut *env.fx);
                }
                Err(refusal) => close(job, refusal.code(), false, &mut *env.fx, &mut *env.events),
            }
        }
    }

    /// The clock moved on (`now_ms` is the shell's; the app keeps the latest it has seen, so a clock that goes back changes
    /// nothing): the captures past their deadline end, `stale_client` while waiting for the file, else `render_failed` (the
    /// bytes dropped). Every call that brings a time ends them, so none is drawn or delivered after its time.
    pub fn expire(&mut self, now_ms: u64, env: Env) {
        self.jobs.tick(now_ms);
        let now = self.jobs.clock();
        for job in self
            .jobs
            .take_where(|j| j.deadline.is_some_and(|d| d <= now))
        {
            let (code, held) = match job.stage {
                Stage::Waiting => ("stale_client", false),
                _ => ("render_failed", true),
            };
            close(job, code, held, &mut *env.fx, &mut *env.events);
        }
    }

    /// The app halted (its view no longer fits the server's): every capture ends as `stale_client`, the bytes dropped.
    pub fn halt(&mut self, env: Env) {
        for job in (self.jobs).take_where(|j| matches!(j.kind, Kind::Capture { .. })) {
            let held = job.stage != Stage::Waiting;
            close(job, "stale_client", held, &mut *env.fx, &mut *env.events);
        }
    }

    /// The shell drew the image of `request` and holds `bytes`. It goes on only when the app is not halted, the report meets the
    /// plan and the limits, and the node is still there as it was planned; otherwise the request ends here, once.
    fn drawn(&mut self, request: u32, drawn: (u32, u32, u64), scene: Scene, env: Env) {
        let Some(mut job) = self.jobs.take(request, Stage::Drawing) else {
            return;
        };
        let max_bytes = matches!(job.kind, Kind::Capture { .. }).then_some(CAPTURE_MAX_BYTES);
        let planned = (job.planned.width, job.planned.height);
        let code = if env.halted {
            // The view of a halted app is out of date (as for `export`): what the shell drew is not delivered.
            "out_of_sync"
        } else if let Some(code) = drawn_refusal(drawn, planned, job.max_pixels, max_bytes) {
            code
        } else {
            match replan(&job, scene) {
                Err(refusal) => refusal.code(),
                Ok(now) if now.rect != job.planned.rect => "node_changed",
                Ok(_) => {
                    job.stage = Stage::Delivering;
                    self.jobs.push(job);
                    env.fx.push(Effect::FinishRaster {
                        request,
                        deliver: true,
                    });
                    return;
                }
            }
        };
        close(job, code, true, env.fx, env.events);
    }
}

/// The plan of the job on the document as it is now.
fn replan(job: &Job, scene: Scene) -> Result<Planned, Refusal> {
    let (scale_ok, max) = job.limits();
    plan(scene, job.node.as_deref(), job.scale, scale_ok, max)
}

/// End the request with `code`: an agent is told (`Effect::FailCapture`), a person gets `Event::Refused`. `discard` when the
/// shell holds the bytes, so it drops them. The job is already out of the table.
fn close(job: Job, code: &str, discard: bool, fx: &mut Vec<Effect>, events: &mut Vec<Event>) {
    if discard {
        fx.push(Effect::FinishRaster {
            request: job.request,
            deliver: false,
        });
    }
    match job.kind {
        Kind::Capture { request_id, .. } => fx.push(Effect::FailCapture {
            request_id,
            code: code.to_owned(),
            message: message(code).to_owned(),
        }),
        Kind::Export { .. } => events.push(Event::Refused {
            code: code.to_owned(),
        }),
    }
}

/// One `ExportChanged` among the events from `from` on, however many parts of `AppState::export` a dispatch changed (the
/// target, the dialog, the plans of the PNG): the front end reads the state again after it. A call the host makes outside a
/// dispatch (`App::set_layout_on_canvas`, `App::set_measurer`) raises its own, once.
pub fn one_export_event(events: &mut Vec<Event>, from: usize) {
    let mut seen = false;
    let mut at = from;
    while at < events.len() {
        if matches!(events[at], Event::ExportChanged {}) && std::mem::replace(&mut seen, true) {
            events.remove(at);
        } else {
            at += 1;
        }
    }
}
