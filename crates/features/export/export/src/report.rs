//! What an export could not carry, and what the caller should look at.

/// One thing the export reports: the `code` is stable, the message is for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    /// The node it is about, or `None` for the file.
    pub node: Option<String>,
    pub message: String,
}

/// The unsupported-property report. `unsupported` lists the properties the export dropped or approximated (the
/// contract's "unsupported" rows); `warnings` lists layout findings (rules G1 to G3), text outside the contract
/// and bindings that did not resolve; `assets` lists the ids of the images the HTML refers to, sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub unsupported: Vec<Finding>,
    pub warnings: Vec<Finding>,
    pub assets: Vec<String>,
}
