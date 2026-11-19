use meldframe_protocol::{Bind, Variable, Variables};
use std::collections::HashMap;

/// What a chain through a cycle counts as: more than any limit a caller compares it with. Corrupt data only.
const CYCLE: usize = usize::MAX / 4;

/// The alias graph of a file: an edge from a variable to each variable it aliases in some mode. It is built
/// in one pass over the variables and answers every question in time linear in the size of the file
/// (variables plus aliases), however many paths there are: the longest chain at each variable is worked out
/// once, in dependency order, not once per path.
#[derive(Debug)]
pub struct Aliases<'a> {
    vars: &'a Variables,
    index: HashMap<&'a str, usize>,
    out: Vec<Vec<usize>>,
    depth: Vec<usize>,
    height: Vec<usize>,
    /// The variables that reach a cycle (only corrupt data has one).
    cyclic: Vec<bool>,
}

impl<'a> Aliases<'a> {
    pub fn new(vars: &'a Variables) -> Self {
        let n = vars.variables.len();
        let mut index = HashMap::with_capacity(n);
        for (i, v) in vars.variables.iter().enumerate() {
            index.entry(v.id.as_str()).or_insert(i);
        }
        let mut out = vec![Vec::new(); n];
        let mut inn = vec![Vec::new(); n];
        for (i, v) in vars.variables.iter().enumerate() {
            for t in v.values.values().filter_map(Bind::var_id) {
                if let Some(&j) = index.get(t) {
                    out[i].push(j);
                    inn[j].push(i);
                }
            }
        }
        let (depth, cyclic) = longest(&out, &inn);
        let (height, _) = longest(&inn, &out);
        Aliases {
            vars,
            index,
            out,
            depth,
            height,
            cyclic,
        }
    }

    /// The variable with this id (the first, should a corrupt file repeat one).
    pub fn variable(&self, id: &str) -> Option<&'a Variable> {
        self.index.get(id).map(|&i| &self.vars.variables[i])
    }

    /// The longest chain of aliases that starts at `id` (literals only is 0), in any mode.
    pub fn depth(&self, id: &str) -> usize {
        self.index.get(id).map_or(0, |&i| self.depth[i])
    }

    /// The longest chain of aliases that ends at `id` (a variable nobody aliases is 0).
    pub fn height(&self, id: &str) -> usize {
        self.index.get(id).map_or(0, |&i| self.height[i])
    }

    /// Whether `id` reaches a cycle of aliases: corrupt data, only a stored file can hold one.
    pub fn reaches_a_cycle(&self, id: &str) -> bool {
        self.index.get(id).is_some_and(|&i| self.cyclic[i])
    }

    /// Whether `from` reaches `target` through aliases in any mode (a variable reaches itself).
    pub fn reaches(&self, from: &str, target: &str) -> bool {
        let (Some(&a), Some(&b)) = (self.index.get(from), self.index.get(target)) else {
            return from == target;
        };
        let mut seen = vec![false; self.out.len()];
        let mut stack = vec![a];
        seen[a] = true;
        while let Some(cur) = stack.pop() {
            if cur == b {
                return true;
            }
            for &t in &self.out[cur] {
                if !seen[t] {
                    seen[t] = true;
                    stack.push(t);
                }
            }
        }
        false
    }
}

/// For each node the longest path along `next`, and whether it reaches a cycle. Nodes are settled in order of
/// dependency (those with no edge left first), so each is looked at once; a node that is never settled lies on
/// or leads into a cycle and counts as [`CYCLE`]. `back` is `next` the other way round.
fn longest(next: &[Vec<usize>], back: &[Vec<usize>]) -> (Vec<usize>, Vec<bool>) {
    let n = next.len();
    let mut left: Vec<usize> = next.iter().map(Vec::len).collect();
    let mut best = vec![0usize; n];
    let mut stack: Vec<usize> = (0..n).filter(|&i| left[i] == 0).collect();
    let mut done = vec![false; n];
    while let Some(u) = stack.pop() {
        done[u] = true;
        best[u] = next[u]
            .iter()
            .map(|&t| best[t].saturating_add(1))
            .max()
            .unwrap_or(0);
        for &p in &back[u] {
            left[p] -= 1;
            if left[p] == 0 {
                stack.push(p);
            }
        }
    }
    let cyclic: Vec<bool> = done.iter().map(|d| !d).collect();
    for i in 0..n {
        if cyclic[i] {
            best[i] = CYCLE;
        }
    }
    (best, cyclic)
}

/// Whether `from` reaches `target` through aliases in any mode (a variable reaches itself). Used to refuse
/// an alias that would close a cycle.
pub fn alias_reaches(vars: &Variables, from: &str, target: &str) -> bool {
    Aliases::new(vars).reaches(from, target)
}

/// The longest chain of aliases that starts at `id` (a variable with literals only is 0), in any mode. A
/// chain that runs into a cycle in corrupt data is longer than any limit, so a caller that compares it with
/// one refuses it.
pub fn alias_depth(vars: &Variables, id: &str) -> usize {
    Aliases::new(vars).depth(id)
}

/// The longest chain of aliases that ends at `id`: how many steps of variables that alias it, and those that
/// alias them, lie above it (a variable nobody aliases is 0). Cycles count as in [`alias_depth`].
pub fn alias_height(vars: &Variables, id: &str) -> usize {
    Aliases::new(vars).height(id)
}
