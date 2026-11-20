//! The command: the single vocabulary of change.

use crate::node::Node;
use crate::patch::NodePatch;
use crate::variables::{Bind, Collection, Mode, Value, Variable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A command applied to the document.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export)]
pub enum Command {
    UpdateNode {
        id: String,
        patch: NodePatch,
    },
    /// Sibling order is set by `node.pos` (empty = after the last sibling; collisions with other siblings are resolved
    /// by the core, see the fractional index rules in the document core). `index` is DEPRECATED: an index into the flat draw list; it is
    /// converted into a pos between siblings at that index and ignored when `node.pos` is set.
    AddNode {
        node: Node,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "number")]
        index: Option<usize>,
    },
    /// Remove a node together with all its descendants.
    RemoveNode {
        id: String,
    },
    /// Change the parent. `pos` (optional) = position among the new parent's children; empty = after the last child
    /// (if the parent is unchanged and `pos` is empty, the order stays). Parent and pos are applied atomically.
    Reparent {
        id: String,
        parent_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        pos: Option<String>,
    },
    /// Change the order among siblings WITHOUT changing the parent. At most one of `before`/`after`/`pos`:
    /// `before`/`after` = id of an anchor sibling (resolved against the state at apply time, so "right in front of X"
    /// survives other concurrent changes); `pos` = explicit position (used by undo). None: move to the front (end of
    /// the sibling list).
    Reorder {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        before: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        pos: Option<String>,
    },
    /// Add a collection with its modes (at least one; the first is the default mode).
    AddCollection {
        collection: Collection,
    },
    /// Remove an empty collection (remove its variables first).
    RemoveCollection {
        id: String,
    },
    /// Give a collection another name. The id is the identity: nothing that points at the collection changes.
    RenameCollection {
        id: String,
        name: String,
    },
    /// Add a mode to a collection at `index` (1 to the number of modes; absent = last; the default mode stays
    /// first). Every variable of the collection gets the value of mode `copy_from` (absent = the default mode)
    /// for the new mode, so a collection is always complete.
    AddMode {
        collection: String,
        mode: Mode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "number")]
        index: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        copy_from: Option<String>,
    },
    /// Remove a mode other than the default one, and its value of every variable of the collection.
    RemoveMode {
        collection: String,
        mode: String,
    },
    /// Give a mode another name. Its id, its place and every value of it stay.
    RenameMode {
        collection: String,
        mode: String,
        name: String,
    },
    /// Add a variable. `variable.values` holds one value for every mode of its collection.
    AddVariable {
        variable: Variable,
    },
    /// Remove a variable that no node field is bound to and no alias points at.
    RemoveVariable {
        id: String,
    },
    /// Give a variable another name. The id is the identity: every binding and alias to it stays.
    RenameVariable {
        id: String,
        name: String,
    },
    /// Set the value of a variable in one mode: a literal of its type or an alias (`{"var": id}`).
    SetVariableValue {
        id: String,
        mode: String,
        value: Bind<Value>,
    },
}
