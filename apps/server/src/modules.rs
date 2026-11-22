//! The feature modules of a build.

use meldframe_module::Module;

/// The feature modules compiled into this build: one entry per enabled cargo feature.
pub(crate) fn modules() -> Vec<Box<dyn Module>> {
    vec![
        #[cfg(feature = "example")]
        Box::new(example::ExampleModule),
        #[cfg(feature = "mcp")]
        Box::new(meldframe_http::mcp::McpModule),
        #[cfg(feature = "capture")]
        Box::new(capture::CaptureModule),
        #[cfg(feature = "assets")]
        Box::new(assets::AssetsModule),
        #[cfg(feature = "design-review")]
        Box::new(design_review::DesignReviewModule),
        #[cfg(feature = "export")]
        Box::new(export::ExportModule),
        #[cfg(feature = "variables-import")]
        Box::new(variables_import::ImportModule),
        #[cfg(feature = "variant")]
        Box::new(meldframe_http_variant::VariantModule),
        #[cfg(feature = "activity")]
        Box::new(meldframe_http_activity::ActivityModule),
        #[cfg(feature = "checkpoints")]
        Box::new(meldframe_http_checkpoints::CheckpointsModule),
    ]
}
