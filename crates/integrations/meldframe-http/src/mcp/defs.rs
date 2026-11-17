//! The tool definitions: name, description, risk tier and input schema of each tool, in one place. The
//! registry (`McpModule`), the list a client gets and the catalogue `docs/mcp-tools.json` all read these, and a
//! test compares them.

use meldframe_module::RiskTier;
use serde_json::{Value, json};

/// The most commands one `apply_commands` call takes, and the most ids one `get_nodes` call takes.
pub const MAX_BATCH: usize = 100;

/// The longest idempotency key of a call. Each command's own key is the call's key, a colon and its index.
pub const MAX_KEY: usize = 100;

/// One tool as a client sees it.
#[derive(Debug, Clone, Copy)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub tier: RiskTier,
    schema: fn() -> Value,
}

impl ToolDef {
    /// The JSON Schema of the tool's arguments.
    pub fn input_schema(&self) -> Value {
        (self.schema)()
    }
}

/// Every tool, in the order a client lists them.
pub const TOOLS: [ToolDef; 5] = [
    ToolDef {
        name: "whoami",
        description: "Report who you are to this server and what limits your calls have: the workspace and actor your token stands for, the batch limit (at most 100 commands per apply_commands call), the limits of a file (nodes, depth, text length) and your call rate. Call it first; it changes nothing.",
        tier: RiskTier::Read,
        schema: whoami_schema,
    },
    ToolDef {
        name: "get_tree",
        description: "Read the node tree of a file: every node with its id, name, kind, position and size, the actor that last changed it and the sequence number of that change, nested under its parent in sibling order. The result carries seq, the file's current sequence number: pass it as ifSeq to apply_commands so that your edit is refused with conflict if a node it touches changed after this read. A file that does not exist yet reads as seq 0 with no nodes. The result lists under warnings every layer that still has a default name (Frame 3, Text): rename it to a semantic name that says what the layer is. Changes nothing.",
        tier: RiskTier::Read,
        schema: get_tree_schema,
    },
    ToolDef {
        name: "get_nodes",
        description: "Read the full data of up to 100 nodes of a file by id: every property of each node, as the commands set them. Ids that are not in the file are listed under missing. The result carries seq, the file's current sequence number, for ifSeq. Changes nothing.",
        tier: RiskTier::Read,
        schema: get_nodes_schema,
    },
    ToolDef {
        name: "apply_commands",
        description: "Change a file by applying commands in order: addNode {node}, updateNode {id, patch}, removeNode {id}, reparent {id, parentId, pos?}, reorder {id, before? | after? | pos?}. One call takes 1 to 100 commands (the batch limit is 100 commands). The call is atomic by default: it is checked as a whole before anything is written, so if any command would be refused, none is written and the error names the command by index. Two rare cases can still leave the first commands written (the store fails part way, or ifSeq is older than the snapshot the file was opened from): send the same call again with the same key and check it with batch_status. Set atomic to false to keep the commands before the first failing one. Every call needs an idempotencyKey (also accepted as idempotency_key): send the same call again with the same key after a timeout and nothing is written twice. Pass ifSeq (also accepted as if_seq), the seq of your last read, to refuse the call with conflict when a node it touches changed since (ifSeq must be within 2000 changes of the file's seq, or the call is refused as invalid; with atomic false, a command that touches a node an earlier command of the call touched and also a new node is refused as invalid, because ifSeq cannot guard it); the error then lists the changes (seq and actor) and the node's current data. Removing a frame needs the owner's approval and is refused as forbidden. Every command is checked against the file's limits; the error code says what to fix. Give every layer a semantic name that says what it is (Hero heading, Pricing card), not a default one (Frame 3, Rectangle, Text); a default name does not fail the call, but it comes back under warnings.",
        tier: RiskTier::Change,
        schema: apply_commands_schema,
    },
    ToolDef {
        name: "batch_status",
        description: "Find out what became of an apply_commands call from its idempotencyKey (also accepted as idempotency_key), for example after a timeout: how many of its commands were written, and the sequence number of each. state is none when nothing was written, applied when commands were written, and partial when you pass count (the number of commands the call had) and fewer were written; then send the call again with the same key to write the rest. Looks at the first 100 commands of a call. Changes nothing.",
        tier: RiskTier::Read,
        schema: batch_status_schema,
    },
];

const FILE: &str = "The name of the file: 1 to 128 characters, no slashes or control characters. A file belongs to the workspace of your token; a name that does not exist yet is a new, empty file.";

fn file() -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": 128, "description": FILE})
}

fn key(spelling: &str, other: &str) -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": MAX_KEY,
        "description": format!("The idempotency key of the call (1 to 100 characters). Send {spelling} or {other}, not both.")})
}

fn whoami_schema() -> Value {
    json!({"type": "object", "properties": {}, "additionalProperties": false})
}

fn get_tree_schema() -> Value {
    json!({"type": "object", "required": ["file"], "additionalProperties": false, "properties": {
        "file": file(),
        "maxDepth": {"type": "integer", "minimum": 0,
            "description": "Do not list nodes nested deeper than this (0 lists only the nodes at the top). Default: all."}
    }})
}

fn get_nodes_schema() -> Value {
    json!({"type": "object", "required": ["file", "ids"], "additionalProperties": false, "properties": {
        "file": file(),
        "ids": {"type": "array", "minItems": 1, "maxItems": MAX_BATCH, "items": {"type": "string"},
            "description": "The ids of the nodes to read: 1 to 100."}
    }})
}

fn apply_commands_schema() -> Value {
    json!({"type": "object", "required": ["file", "commands"], "additionalProperties": false, "properties": {
        "file": file(),
        "commands": {"type": "array", "minItems": 1, "maxItems": MAX_BATCH,
            "items": {"type": "object", "required": ["type"], "properties": {
                "type": {"enum": ["addNode", "updateNode", "removeNode", "reparent", "reorder"]}},
                "description": "One command; its fields depend on its type, as listed in the tool description. addNode takes node {id, type (frame, rect, ellipse, text or shader), name, parentId, x, y, w, h, fill, text, ...}; updateNode takes a patch of the fields to change."},
            "description": "The commands to apply in order: 1 to 100."},
        "idempotencyKey": key("idempotencyKey", "idempotency_key"),
        "idempotency_key": key("idempotency_key", "idempotencyKey"),
        "ifSeq": {"type": "integer", "minimum": 0,
            "description": "The seq of your last read. The call is refused with conflict if a node it touches changed after it. Send ifSeq or if_seq, not both."},
        "if_seq": {"type": "integer", "minimum": 0, "description": "The same as ifSeq."},
        "atomic": {"type": "boolean", "default": true,
            "description": "True (the default): write all commands or none. False: write the commands before the first one that fails."}
    }})
}

fn batch_status_schema() -> Value {
    json!({"type": "object", "required": ["file"], "additionalProperties": false, "properties": {
        "file": file(),
        "idempotencyKey": key("idempotencyKey", "idempotency_key"),
        "idempotency_key": key("idempotency_key", "idempotencyKey"),
        "count": {"type": "integer", "minimum": 1, "maximum": MAX_BATCH,
            "description": "How many commands the call had; with it, state can be partial."}
    }})
}
