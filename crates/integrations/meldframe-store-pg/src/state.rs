//! The `state` column of a snapshot. A file without variables keeps the plain array of nodes that every
//! snapshot was before E3.3, so those rows and the rows written for such files stay byte for byte as they were
//! and an older server reads them; a file with variables is the object `{"nodes": [...], "variables": {...}}`
//! (ADR-0017). Reading takes either shape.

use meldframe_host::Snapshot;
use meldframe_protocol::{Node, Variables};
use serde::Deserialize;
use serde_json::{Value, json};

/// The JSON to keep in `snapshots.state`.
pub fn state_json(s: &Snapshot) -> Value {
    if s.variables.is_empty() {
        json!(s.nodes)
    } else {
        json!({"nodes": s.nodes, "variables": s.variables})
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Nodes(Vec<Node>),
    Doc {
        nodes: Vec<Node>,
        #[serde(default)]
        variables: Variables,
    },
}

/// The snapshot at `seq` from a `state` value; an error text when it is neither shape.
pub fn snapshot_from_state(seq: u64, state: Value) -> Result<Snapshot, String> {
    let (nodes, variables) =
        match serde_json::from_value::<Stored>(state).map_err(|e| e.to_string())? {
            Stored::Nodes(nodes) => (nodes, Variables::default()),
            Stored::Doc { nodes, variables } => (nodes, variables),
        };
    Ok(Snapshot {
        seq,
        nodes,
        variables,
    })
}
