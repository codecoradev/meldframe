//! The Export variables dialog (D8): every variable of the file as CSS, built by the `export` feature from the person's
//! own view of the variables. No modes or frame come into it: all modes are written.

use export::{ExportError, Finding, MAX_OUTPUT_BYTES, export_variables};
use meldframe_app_model::{ExportFinding, VariablesExportDialog};
use meldframe_protocol::Variables;

/// The dialog for the variables of the file.
pub fn variables_dialog(vars: &Variables) -> VariablesExportDialog {
    let mut d = VariablesExportDialog {
        css: String::new(),
        collections: 0,
        variables: 0,
        blocks: 0,
        bytes: 0,
        limit: MAX_OUTPUT_BYTES,
        warnings: Vec::new(),
        error: None,
    };
    match export_variables(vars) {
        Ok(e) => {
            d.bytes = e.css.len();
            d.css = e.css;
            d.collections = e.collections;
            d.variables = e.variables;
            d.blocks = e.blocks;
            d.warnings = e.warnings.iter().map(finding).collect();
        }
        Err(e) => {
            if let ExportError::TooLarge { bytes, .. } = e {
                d.bytes = bytes;
            }
            d.error = Some(e.code().to_string());
        }
    }
    d
}

fn finding(f: &Finding) -> ExportFinding {
    ExportFinding {
        code: f.code.to_string(),
        node: None,
        node_name: None,
        message: f.message.clone(),
    }
}
