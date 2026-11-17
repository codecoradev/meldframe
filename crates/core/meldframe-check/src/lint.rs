//! The layer-name lint. Agents and tools often leave the name a node was created with (`Frame 12`, `Rectangle`,
//! `Text`), which says nothing in the layers panel or in an activity entry. The lint reports such names; it
//! refuses nothing, because a default name is allowed (it is only unhelpful).

use meldframe_protocol::Node;
use serde_json::{Value, json};

/// Words a tool uses for a layer it has just made. A name is a default name when it is one of these words,
/// in any case, alone or followed by a number.
const DEFAULT_WORDS: [&str; 14] = [
    "frame",
    "rectangle",
    "rect",
    "ellipse",
    "text",
    "group",
    "layer",
    "shader",
    "vector",
    "image",
    "untitled",
    "component",
    "line",
    "auto layout",
];

/// True when `name` is empty, or is a default word with nothing but an optional number after it.
pub fn is_default_layer_name(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    if lower.is_empty() {
        return true;
    }
    let base = lower
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .trim_end();
    // A number alone is not a default name (`2024` can be a label); the base must be one of the words.
    DEFAULT_WORDS.contains(&base)
        && (base.len() == lower.len()
            || lower[base.len()..]
                .trim()
                .chars()
                .all(|c| c.is_ascii_digit()))
}

/// A layer whose name says nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameFinding {
    pub id: String,
    pub name: String,
}

/// The nodes whose name is a default name or only their own id, in the order given.
pub fn default_name_findings(nodes: &[Node]) -> Vec<NameFinding> {
    nodes
        .iter()
        .filter(|n| is_default_layer_name(&n.name) || n.name == n.id)
        .map(|n| NameFinding {
            id: n.id.clone(),
            name: n.name.clone(),
        })
        .collect()
}

/// The warning for a layer that keeps a default name, as the agent tools return it. A lint only: the call it comes
/// with is written.
fn name_warning(id: &str, name: &str) -> Value {
    json!({"code": "default_layer_name", "nodeId": id, "name": name,
        "message": format!("\"{name}\" is a default layer name. Give each layer a semantic name that says what it is \
            (for example \"Hero heading\"), and rename it with updateNode.")})
}

fn is_default(id: &str, name: &str) -> bool {
    is_default_layer_name(name) || name == id
}

/// The warnings for the names a call sets: the name of each `addNode` (an absent name counts as empty) and of each
/// `updateNode` that has a `name` in its patch. Commands that are not JSON objects of that shape are skipped; the
/// validation of the call reports them.
pub fn name_warnings_of_commands(commands: &[Value]) -> Vec<Value> {
    commands
        .iter()
        .filter_map(|c| {
            let (id, name) = match c["type"].as_str()? {
                "addNode" => (
                    c["node"]["id"].as_str()?,
                    c["node"]["name"].as_str().unwrap_or(""),
                ),
                "updateNode" => (c["id"].as_str()?, c["patch"]["name"].as_str()?),
                _ => return None,
            };
            is_default(id, name).then(|| name_warning(id, name))
        })
        .collect()
}

/// Most warnings a read returns: a big file with default names everywhere would otherwise answer with thousands.
pub const MAX_NAME_WARNINGS: usize = 100;

/// The first warnings (at most `MAX_NAME_WARNINGS`) for the layers of a file that have a default name.
pub fn name_warnings_of_nodes(nodes: &[Node]) -> Vec<Value> {
    default_name_findings(nodes)
        .iter()
        .take(MAX_NAME_WARNINGS)
        .map(|f| name_warning(&f.id, &f.name))
        .collect()
}
