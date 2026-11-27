//! The plain data of the exports of the app layer: the PNG of a frame (`raster`) and the Export dialog (`export`). Data only; the
//! rules are `meldframe-app-rules` (`raster`, `png`) and `meldframe-app-parts`, and `meldframe-app-model` re-exports both modules.

pub mod export;
pub mod raster;
