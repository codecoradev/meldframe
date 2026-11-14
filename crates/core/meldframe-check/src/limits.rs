//! Ceilings that protect a file from absurd values.

/// Ceilings on what one command may put into a file. The defaults suit the editor; a caller may tighten
/// them (for example in tests).
#[derive(Debug, Clone)]
pub struct Limits {
    /// Most nodes a file may hold.
    pub max_nodes: usize,
    /// Bound on `|x|` and `|y|`.
    pub max_coord: f64,
    /// Bound on `w`, `h`, `gap`, `padding`, `radius`, `fontSize` and the decoration sizes.
    pub max_size: f64,
    /// Most characters in `text`.
    pub max_text: usize,
    /// Most characters in `name`.
    pub max_name: usize,
    /// Most characters in a node id.
    pub max_id: usize,
    /// Deepest nesting of the node tree (a root is at depth 0).
    pub max_depth: usize,
    /// Longest `pos` in characters. Repeated inserts into the same gap grow a key by about one character
    /// per six inserts, so 64 allows 300+ consecutive inserts at one spot; more is refused as
    /// `PosTooLong`.
    pub max_pos: usize,
    /// Most shadows on one node.
    pub max_shadows: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_nodes: 20_000,
            max_coord: 1.0e6,
            max_size: 100_000.0,
            max_text: 20_000,
            max_name: 256,
            max_id: 64,
            max_depth: 32,
            max_pos: 64,
            max_shadows: 4,
        }
    }
}
