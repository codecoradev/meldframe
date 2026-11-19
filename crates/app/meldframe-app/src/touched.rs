//! Which nodes a command touches, for the redraw and for the attribution of a change.

use meldframe_protocol::Command;
use meldframe_variables::is_variable_command;

/// The nodes to redraw after these commands. Removing a node removes its subtree, which the command does
/// not list, and a variable command changes every node bound to the variable: both are an empty list, which
/// means everything.
pub(crate) fn render_set(commands: &[Command]) -> Vec<String> {
    if commands
        .iter()
        .any(|c| matches!(c, Command::RemoveNode { .. }) || is_variable_command(c))
    {
        return Vec::new();
    }
    let mut ids: Vec<String> = commands.iter().flat_map(touched).collect();
    ids.dedup();
    ids
}

/// The id of the node a command changes.
pub(crate) fn touched(command: &Command) -> Vec<String> {
    let id = match command {
        Command::UpdateNode { id, .. }
        | Command::RemoveNode { id }
        | Command::Reparent { id, .. }
        | Command::Reorder { id, .. } => id,
        Command::AddNode { node, .. } => &node.id,
        // `render_set` redraws everything after a variable command, so no node is named.
        _ => return Vec::new(),
    };
    vec![id.clone()]
}
