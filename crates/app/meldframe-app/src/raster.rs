//! The PNG export of a frame, as the app calls it (finding T13 of the thin UI audit, decision 43): the rules and their state are
//! `meldframe_app_rules::png`; this hands them the scene of the epoch, the state of the export and the effects and events.

use crate::app::App;
use crate::effect::Effect;
use crate::state::Connection;
use meldframe_app_model::PngOp;
use meldframe_app_parts::answers::ask_is_safe;
use meldframe_app_rules::png::{Env, Rasters};
use meldframe_app_rules::raster::Scene;
use meldframe_render::HitIndex;

impl App {
    /// Reads the scene a plan needs: the boxes of the epoch and their index, both kept per epoch; `read` must not call `App::hit`.
    fn with_plan_scene<R>(&self, read: impl FnOnce(Scene) -> R) -> R {
        self.with_scene(|_, boxes| {
            self.canvas.with_hits(
                self.replica.revision(),
                || HitIndex::from_boxes(boxes),
                |index| read(Scene { boxes, index }),
            )
        })
    }

    /// Runs a step of the rules on the scene (when `scene` is set) and the export state; the effects it asks for are returned.
    fn png_run(
        &mut self,
        scene: bool,
        run: impl FnOnce(&mut Rasters, Option<Scene>, Env),
    ) -> Vec<Effect> {
        let (halted, mut fx, mut events) = (
            self.state.connection == Connection::Halted,
            Vec::new(),
            Vec::new(),
        );
        let (mut rasters, mut export) = (
            std::mem::take(&mut self.raster),
            std::mem::take(&mut self.state.export),
        );
        let env = Env {
            halted,
            seq: self.replica.seq(),
            export: &mut export,
            fx: &mut fx,
            events: &mut events,
        };
        if scene {
            self.with_plan_scene(|s| run(&mut rasters, Some(s), env));
        } else {
            run(&mut rasters, None, env);
        }
        (self.raster, self.state.export) = (rasters, export);
        self.events.append(&mut events);
        fx
    }

    pub(crate) fn png_action(&mut self, op: PngOp, fx: &mut Vec<Effect>) {
        // An action meets the same check as a frame from the wire (its id and token go into a path and a header).
        if matches!(&op, PngOp::Capture { ask, .. } if !ask_is_safe(ask)) {
            return;
        }
        fx.extend(self.png_run(true, |r, scene, env| {
            r.apply(op, scene.expect("asked for"), env)
        }));
    }

    /// Bring the plans of the PNG export in line with the selection, the open dialog, the scale and the document: once for
    /// each epoch of the scene, and with no scene worked out while no frame is the target and no dialog is open.
    pub(crate) fn refresh_png(&mut self) {
        let epoch = self.scene_epoch();
        let export = &self.state.export;
        if self.raster.is_stale(epoch, export) {
            let planned = export.target.is_some() || export.dialog.is_some();
            self.png_run(planned, |r, scene, env| r.refresh(epoch, scene, env));
        }
    }

    /// What the clock, the file and a halt do to the captures that are out: they end, go on or end (`meldframe_app_rules::png`).
    pub(crate) fn captures_moved(&mut self, now_ms: Option<u64>, fx: &mut Vec<Effect>) {
        if let Some(now_ms) = now_ms {
            fx.extend(self.png_run(false, |r, _, env| r.expire(now_ms, env)));
        }
        if self.state.connection == Connection::Halted && self.raster.captures() > 0 {
            fx.extend(self.png_run(false, |r, _, env| r.halt(env)));
        } else if self.raster.waits() {
            fx.extend(self.png_run(true, |r, scene, env| {
                r.resume(scene.expect("asked for"), env)
            }));
        }
    }
}
