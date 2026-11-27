//! The PNG export of a frame as state transitions (finding T13 of the thin UI audit, decision 43): the scale, the plan for the
//! frame of the section and of the open dialog (the rectangle with the reach of its paint, the size in pixels, the file name,
//! or why there is none), the export itself as an `Effect::Raster` and what comes of it: the shell reports the image it drew
//! (`Wire::Drawn`), the app checks it against the plan and the limits and the node as it is now, and the shell is told to
//! deliver the bytes or to drop them (`Effect::FinishRaster`); a request is answered once. View state: nothing is written to the file and the undo
//! history does not change. `meldframe-app` hands each step the scene, the state of the export and where the effects and the
//! events go (`Env`); the pure rules are in `raster`, and the drawing and the save are the shell's.

use crate::code::code as safe;
use crate::jobs::{Job, Jobs, Kind, Stage};
use crate::raster::{Planned, Refusal, Scene, drawn_refusal, is_export_scale, plan, png_plan};
use meldframe_app_model::{
    EXPORT_MAX_PIXELS, Effect, Event, ExportState, PngOp, PngPlan, PngSink, RasterSink,
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
    pub export: &'a mut ExportState,
    pub fx: &'a mut Vec<Effect>,
    pub events: &'a mut Vec<Event>,
}

impl Env<'_> {
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
            PngOp::Drawn {
                request,
                width,
                height,
                bytes,
            } => self.drawn(request, (width, height, bytes), scene, env),
            PngOp::DrawFailed { request, code } => {
                if let Some(job) = self.jobs.take(request, Stage::Drawing) {
                    close(job, &safe(&code), false, env);
                }
            }
            PngOp::Delivered { request } => drop(self.jobs.take(request, Stage::Delivering)),
            PngOp::DeliveryFailed { request, code } => {
                if let Some(job) = self.jobs.take(request, Stage::Delivering) {
                    close(job, &safe(&code), false, env);
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
        };
        self.draw(job, env.fx);
    }

    /// Ask the shell to draw the job's image (`Effect::Raster`) and wait for its report.
    fn draw(&mut self, mut job: Job, fx: &mut Vec<Effect>) {
        let Kind::Export { sink, file } = &job.kind;
        fx.push(Effect::Raster {
            request: job.request,
            rect: job.planned.rect,
            scale: job.scale,
            width: job.planned.width,
            height: job.planned.height,
            file: Some(file.clone()),
            to: match sink {
                PngSink::Download => RasterSink::Download {},
                PngSink::Clipboard => RasterSink::Clipboard {},
            },
        });
        job.stage = Stage::Drawing;
        self.jobs.push(job);
    }

    /// The shell drew the image of `request` and holds `bytes`. It goes on only when the report meets the plan and the limits,
    /// and the node is still there as it was planned; otherwise the request ends here, once.
    fn drawn(&mut self, request: u32, drawn: (u32, u32, u64), scene: Scene, env: Env) {
        let Some(mut job) = self.jobs.take(request, Stage::Drawing) else {
            return;
        };
        let planned = (job.planned.width, job.planned.height);
        let code = if let Some(code) = drawn_refusal(drawn, planned, job.max_pixels, None) {
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
        close(job, code, true, env);
    }
}

/// The plan of the job on the document as it is now.
fn replan(job: &Job, scene: Scene) -> Result<Planned, Refusal> {
    plan(
        scene,
        job.node.as_deref(),
        job.scale,
        is_export_scale,
        EXPORT_MAX_PIXELS,
    )
}

/// End the request with `code`, which the person gets as `Event::Refused`. `discard` when the shell holds the bytes, so it
/// drops them. The job is already out of the table.
fn close(job: Job, code: &str, discard: bool, env: Env) {
    if discard {
        env.fx.push(Effect::FinishRaster {
            request: job.request,
            deliver: false,
        });
    }
    env.events.push(Event::Refused {
        code: code.to_owned(),
    });
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
