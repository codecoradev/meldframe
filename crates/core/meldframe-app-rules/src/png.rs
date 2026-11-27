//! The PNG export of a frame as state transitions (finding T13 of the thin UI audit, decision 43): the scale, the plan for the
//! frame of the section and of the open dialog (the rectangle with the reach of its paint, the size in pixels, the file name,
//! or why there is none) and the export itself as an `Effect::Raster`. View state: nothing is written to the file and the undo
//! history does not change. `meldframe-app` hands each step the scene, the state of the export and where the effects and the
//! events go (`Env`); the pure rules are in `raster`, and the drawing and the save are the shell's.

use crate::raster::{Scene, is_export_scale, png_plan};
use meldframe_app_model::{Effect, Event, ExportState, PngOp, PngPlan, PngSink, RasterSink};

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

/// The plans of the PNG export, and (below) the requests for an image that are out.
#[derive(Debug, Default)]
pub struct Rasters {
    key: Option<PlanKey>,
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
        match plan {
            PngPlan::Ready {
                rect,
                scale,
                width,
                height,
                file,
            } => env.fx.push(Effect::Raster {
                rect,
                scale,
                width,
                height,
                file,
                to: match to {
                    PngSink::Download => RasterSink::Download {},
                    PngSink::Clipboard => RasterSink::Clipboard {},
                },
            }),
            PngPlan::Refused { code, .. } => env.refuse(&code),
        }
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
