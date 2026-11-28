//! The limits and tables of the app layer that a front end shows or sizes a control with, carried in `AppState.limits`
//! so that none is copied into a front end (audit finding T3, rule D9: one authority). Each value is the constant of the
//! rule that uses it; a front end reads this block and keeps no number of its own. The limits of the Variables manage
//! view are in `AppState.manage.limits` (`ManageLimits`).

use crate::preset::{FramePreset, PresetRow};
use crate::raster::{EXPORT_MAX_PIXELS, SCALES};
use crate::variables_io::IMPORT_MAX_VARIABLES;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The limits and tables a front end shows, each the constant of the rule that uses it (`AppState.limits`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppLimits {
    /// The most variables the Import variables dialog takes at a time (`IMPORT_MAX_VARIABLES`).
    pub import_variables: usize,
    /// The most pixels (width times height) of an exported image (`EXPORT_MAX_PIXELS`).
    #[ts(type = "number")]
    pub export_max_pixels: u64,
    /// The scales of a PNG export, smallest first (`SCALES`).
    pub export_scales: Vec<f64>,
    /// The frame presets, in the order a menu lists them (`FramePreset::table`).
    pub presets: Vec<PresetRow>,
}

impl Default for AppLimits {
    fn default() -> Self {
        AppLimits {
            import_variables: IMPORT_MAX_VARIABLES,
            export_max_pixels: EXPORT_MAX_PIXELS,
            export_scales: SCALES.to_vec(),
            presets: FramePreset::table(),
        }
    }
}
