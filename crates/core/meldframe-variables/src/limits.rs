/// The ceilings the variable checks use. `meldframe-check` fills them from its `Limits`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarLimits {
    /// Longest id, in characters.
    pub max_id: usize,
    /// Longest name, in characters.
    pub max_name: usize,
    /// Bound on the absolute value of a number.
    pub max_size: f64,
    /// Most collections in a file.
    pub max_collections: usize,
    /// Most modes in one collection.
    pub max_modes: usize,
    /// Most variables in a file.
    pub max_variables: usize,
}
