use meldframe_protocol::{Bind, Variables};
use std::collections::HashSet;

/// The variables that `id` aliases in any of its modes.
fn targets<'a>(vars: &'a Variables, id: &str) -> impl Iterator<Item = &'a str> {
    vars.variable(id)
        .into_iter()
        .flat_map(|v| v.values.values())
        .filter_map(Bind::var_id)
}

/// Whether `from` reaches `target` through aliases in any mode (a variable reaches itself). Used to refuse
/// an alias that would close a cycle.
pub fn alias_reaches(vars: &Variables, from: &str, target: &str) -> bool {
    let mut seen: HashSet<&str> = HashSet::from([from]);
    let mut stack = vec![from];
    while let Some(cur) = stack.pop() {
        if cur == target {
            return true;
        }
        for t in targets(vars, cur) {
            if seen.insert(t) {
                stack.push(t);
            }
        }
    }
    false
}

/// The longest chain of aliases that starts at `id` (a variable with literals only is 0), in any mode. A
/// cycle in corrupt data counts as the number of variables, so a caller that compares it with a limit
/// refuses it.
pub fn alias_depth(vars: &Variables, id: &str) -> usize {
    fn go<'a>(vars: &'a Variables, id: &'a str, path: &mut Vec<&'a str>, cap: usize) -> usize {
        if path.contains(&id) || path.len() > cap {
            return cap;
        }
        path.push(id);
        let d = targets(vars, id)
            .map(|t| 1 + go(vars, t, path, cap))
            .max()
            .unwrap_or(0);
        path.pop();
        d
    }
    go(vars, id, &mut Vec::new(), vars.variables.len())
}

/// The longest chain of aliases that ends at `id`: how many steps of variables that alias it, and those that
/// alias them, lie above it (a variable nobody aliases is 0). Corrupt cycles are cut, as in [`alias_depth`].
pub fn alias_height(vars: &Variables, id: &str) -> usize {
    fn go<'a>(vars: &'a Variables, id: &'a str, path: &mut Vec<&'a str>, cap: usize) -> usize {
        if path.contains(&id) || path.len() > cap {
            return cap;
        }
        path.push(id);
        let h = vars
            .variables
            .iter()
            .filter(|v| v.values.values().any(|b| b.var_id() == Some(id)))
            .map(|v| 1 + go(vars, &v.id, path, cap))
            .max()
            .unwrap_or(0);
        path.pop();
        h
    }
    go(vars, id, &mut Vec::new(), vars.variables.len())
}
