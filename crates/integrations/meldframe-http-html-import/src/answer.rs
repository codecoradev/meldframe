//! The answer of the tool as JSON: the nodes written with what each could not take, the skipped elements, the
//! addresses listed, the warnings and the report of the readers. Every string that comes from the document was made
//! safe where the importer made it (rule U9), so this only lays it out.

use html_import::map::{Imported, NodeInfo};
use meldframe_layout::{
    StubMeasurer, TEXT_OUTSIDE_CONTRACT_MESSAGE, layout_findings, text_outside_contract,
};
use meldframe_protocol::{Command, Node, NodeKind, SizeMode};
use serde_json::{Value, json};

/// The most text boxes the `text_size_estimated` warning names.
const LISTED: usize = 8;

pub(crate) fn nodes_of(i: &Imported) -> Vec<Node> {
    i.commands
        .iter()
        .filter_map(|c| match c {
            Command::AddNode { node, .. } => Some(node.clone()),
            _ => None,
        })
        .collect()
}

fn kind(k: NodeKind) -> Value {
    serde_json::to_value(k).unwrap_or(Value::Null)
}

fn node(n: &NodeInfo) -> Value {
    json!({"id": n.id, "name": n.name, "type": kind(n.kind), "source": n.source,
        "approximated": n.approximated.iter()
            .map(|c| json!({"property": c.property, "from": c.from, "to": c.to})).collect::<Vec<_>>(),
        "unsupported": n.unsupported.iter()
            .map(|l| json!({"property": l.property, "value": l.value, "code": l.code})).collect::<Vec<_>>()})
}

/// What `apply_commands` and `export_frame` already warn about, for the nodes of this import: default layer names,
/// the findings of the layout core, text a browser breaks differently and the sizes the server only estimates.
fn warnings(i: &Imported, nodes: &[Node]) -> Vec<Value> {
    let commands: Vec<Value> = i
        .commands
        .iter()
        .filter_map(|c| serde_json::to_value(c).ok())
        .collect();
    let mut out = meldframe_check::name_warnings_of_commands(&commands);
    // The root is judged alone: the frame it goes into is not part of this import.
    let mut alone = nodes.to_vec();
    if let Some(r) = alone.first_mut() {
        r.parent_id = None;
    }
    out.extend(
        layout_findings(&alone, &StubMeasurer)
            .into_iter()
            .map(|f| json!({"code": f.code, "nodeId": f.id, "message": f.message})),
    );
    let mut estimated: Vec<&str> = Vec::new();
    for n in nodes.iter().filter(|n| n.kind == NodeKind::Text) {
        if text_outside_contract(&n.text) {
            out.push(json!({"code": "text_outside_contract", "nodeId": n.id,
                "message": TEXT_OUTSIDE_CONTRACT_MESSAGE}));
        }
        if n.sw == SizeMode::Hug || n.sh == SizeMode::Hug {
            estimated.push(&n.id);
        }
    }
    if !estimated.is_empty() {
        let more = estimated.len().saturating_sub(LISTED);
        let tail = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        let listed = estimated[..estimated.len() - more].join(", ");
        let what = match estimated.len() {
            1 => "1 text box gets its size".to_owned(),
            n => format!("{n} text boxes get their size"),
        };
        out.push(json!({"code": "text_size_estimated", "nodeId": null, "message": format!(
            "{what} from an estimate of the text (the server has no font), so the stored size may differ from the canvas: {listed}{tail}")}));
    }
    out
}

fn report(i: &Imported) -> Value {
    let r = &i.report;
    json!({"counts": r.counts,
        "entries": r.entries.iter().map(|e| json!({"kind": e.kind.as_str(), "code": e.code,
            "name": e.name, "message": e.message, "at": e.at})).collect::<Vec<_>>(),
        "omitted": r.omitted})
}

/// The answer without what the write adds (`seq`, `batch`, `replayed`).
pub(crate) fn result(i: &Imported) -> Value {
    let nodes = nodes_of(i);
    json!({"ok": true, "root": i.root, "commands": i.commands.len(),
        "nodes": i.nodes.iter().map(node).collect::<Vec<_>>(),
        "skipped": i.skipped.iter().map(|s| json!({"source": s.source, "code": s.code})).collect::<Vec<_>>(),
        "skippedOmitted": i.skipped_omitted,
        "assets": i.assets.iter().map(|a| json!({"node": a.node, "use": a.usage, "src": a.src})).collect::<Vec<_>>(),
        "warnings": warnings(i, &nodes),
        "report": report(i)})
}

/// The properties that were not carried, as the error of a `strict` call lists them (the first 100).
pub(crate) fn unsupported(i: &Imported) -> Vec<Value> {
    i.nodes
        .iter()
        .flat_map(|n| {
            n.unsupported.iter().map(move |l| {
                json!({"nodeId": n.id, "name": n.name, "source": n.source,
                    "property": l.property, "value": l.value, "code": l.code})
            })
        })
        .take(100)
        .collect()
}
