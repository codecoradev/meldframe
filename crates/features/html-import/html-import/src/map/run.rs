//! The import itself: the text parsed, styled and walked, and the tree of nodes made a list of commands.

use super::walk::Walk;
use super::{
    AssetRef, ImportError, Imported, MAX_COMMANDS, MAX_INPUT_BYTES, NodeInfo, Options, Skipped,
    node,
};
use crate::css::{Limits as CssLimits, Report, style_tree};
use crate::html::{Limits, parse};
use meldframe_protocol::Command;

/// Read `html`, with `css` as one more style sheet, into nodes. The cascade reads `css` after every `<style>`.
pub fn import(html: &str, css: &str, options: &Options) -> Result<Imported, ImportError> {
    let bytes = html.len() + css.len();
    if bytes > MAX_INPUT_BYTES {
        return Err(ImportError::TooLarge {
            bytes,
            max: MAX_INPUT_BYTES,
        });
    }
    let limits = Limits {
        max_input_bytes: MAX_INPUT_BYTES - css.len(),
        ..Limits::default()
    };
    let parsed = parse(html, &limits)?;
    let mut report = parsed.report;
    let styles = style_tree(&parsed.tree, css, &CssLimits::default(), &mut report);
    let mut walk = Walk::new(&parsed.tree, &styles, &mut report, options);
    let root = walk.page();
    // Past the cap the walk stopped early: the page makes at least that many nodes, and the call is refused.
    if walk.made() > MAX_COMMANDS {
        return Err(ImportError::TooMany {
            nodes: walk.made(),
            max: MAX_COMMANDS,
        });
    }
    let root = root.ok_or(ImportError::Empty)?;
    let (skipped, skipped_omitted) = walk.into_skipped();
    flatten(root, options, report, skipped, skipped_omitted)
}

/// The tree of specs as a list of commands: parent first, ids from the prefix, the root at the place and in the
/// frame the caller says.
fn flatten(
    root: node::Spec,
    options: &Options,
    report: Report,
    skipped: Vec<Skipped>,
    skipped_omitted: usize,
) -> Result<Imported, ImportError> {
    let nodes = root.count();
    if nodes > MAX_COMMANDS {
        return Err(ImportError::TooMany {
            nodes,
            max: MAX_COMMANDS,
        });
    }
    let mut out = Imported {
        commands: Vec::with_capacity(nodes),
        root: String::new(),
        nodes: Vec::with_capacity(nodes),
        skipped,
        skipped_omitted,
        assets: Vec::new(),
        report,
    };
    let mut todo = vec![(root, None::<String>)];
    // Depth first with the first child on top, so siblings keep the order of the source.
    while let Some((mut spec, parent)) = todo.pop() {
        let id = format!("{}-{}", options.id_prefix, out.commands.len());
        spec.node.id.clone_from(&id);
        match &parent {
            Some(p) => spec.node.parent_id = Some(p.clone()),
            None => {
                spec.node.parent_id.clone_from(&options.parent);
                (spec.node.x, spec.node.y) = options.origin;
                out.root.clone_from(&id);
            }
        }
        out.assets
            .extend(spec.assets.iter().map(|(usage, a)| AssetRef {
                node: id.clone(),
                usage,
                src: a.src.clone(),
            }));
        out.nodes.push(NodeInfo {
            id: id.clone(),
            name: spec.node.name.clone(),
            kind: spec.node.kind,
            source: spec.source,
            approximated: spec.notes.approx,
            unsupported: spec.notes.lost,
        });
        let kids = std::mem::take(&mut spec.kids);
        out.commands.push(Command::AddNode {
            node: spec.node,
            index: None,
        });
        todo.extend(kids.into_iter().rev().map(|k| (k, Some(id.clone()))));
    }
    Ok(out)
}
