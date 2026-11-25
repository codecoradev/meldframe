//! The answer of the tool: the nodes, what was skipped, the assets, the warnings and the report, as JSON a client
//! reads (`docs/import-html.md` section 4.2). Every string that came from the text is cleaned again here, so a control
//! or bidi character never reaches the answer, and everything is bounded: at most 100 nodes, the report's own bounds.

use crate::plan::Planned;
use html_import::html::{Entry, Report, clean};
use meldframe_protocol::{Command, Node, NodeKind, SizeMode};
use serde_json::{Value, json};

/// The longest string of the text echoed in the answer, in characters.
const ECHO: usize = 240;

/// The most ids the `text_size_estimated` message lists.
const LISTED_IDS: usize = 8;

fn echo(s: &str) -> String {
    clean(s, ECHO)
}

/// One entry of the report.
pub fn entry(e: &Entry) -> Value {
    json!({"kind": e.kind.as_str(), "code": e.code, "name": echo(&e.name),
        "message": echo(&e.message), "at": e.at})
}

/// The report: how often each code happened, and the entries the bounds kept.
fn report(r: &Report) -> Value {
    json!({"counts": r.counts, "entries": r.entries.iter().map(entry).collect::<Vec<_>>(),
        "omitted": r.omitted})
}

fn kind(k: NodeKind) -> Value {
    serde_json::to_value(k).unwrap_or(Value::Null)
}

/// The nodes the commands add, in order.
fn added(p: &Planned) -> Vec<&Node> {
    (p.plan.commands.iter())
        .filter_map(|c| match c {
            Command::AddNode { node, .. } => Some(node),
            _ => None,
        })
        .collect()
}

/// What the file's own checks say about the nodes (default layer names, the layout rules of the contract) and, as
/// the export says, that the size of a text the measurer decides is an estimate. At most `MAX_NAME_WARNINGS`.
fn warnings(nodes: &[&Node]) -> Vec<Value> {
    let owned: Vec<Node> = nodes.iter().map(|n| (*n).clone()).collect();
    let mut out: Vec<Value> =
        meldframe_layout::layout_findings(&owned, &meldframe_layout::StubMeasurer)
            .into_iter()
            .map(|w| json!({"code": w.code, "nodeId": w.id, "message": w.message}))
            .collect();
    out.extend(meldframe_check::name_warnings_of_nodes(&owned));
    let guessed: Vec<&str> = (nodes.iter())
        .filter(|n| n.kind == NodeKind::Text && (n.sw == SizeMode::Hug || n.sh != SizeMode::Fixed))
        .map(|n| n.id.as_str())
        .collect();
    if !guessed.is_empty() {
        let more = guessed.len().saturating_sub(LISTED_IDS);
        let tail = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        out.push(json!({"code": "text_size_estimated", "message": format!(
            "{} text boxes get their size from an estimate of the text, not from the font the browser uses, so the size may differ from the canvas and the Export dialog: {}{tail}",
            guessed.len(), guessed[..guessed.len() - more].join(", "))}));
    }
    out.truncate(meldframe_check::MAX_NAME_WARNINGS);
    out
}

/// The part of the answer that depends on the plan alone, so it is the same for a dry run, a write and a replay.
pub fn describe(p: &Planned) -> Value {
    let plan = &p.plan;
    let nodes: Vec<Value> = plan
        .nodes
        .iter()
        .map(|n| {
            json!({"id": n.id, "name": echo(&n.name), "type": kind(n.kind), "source": echo(&n.source),
                "approximated": n.approximated.iter().map(|a| json!({"property": echo(&a.property),
                    "from": echo(&a.from), "to": echo(&a.to)})).collect::<Vec<_>>(),
                "unsupported": n.unsupported.iter().map(|u| json!({"property": echo(&u.property),
                    "value": echo(&u.value), "code": u.code})).collect::<Vec<_>>()})
        })
        .collect();
    // An element that makes no node: the report keeps its tag and where it starts.
    let skipped: Vec<Value> = (plan.report.entries.iter())
        .filter(|e| matches!(e.code, "display_none" | "element_skipped"))
        .map(|e| json!({"source": echo(&e.name), "code": e.code, "at": e.at}))
        .collect();
    let assets: Vec<Value> = (plan.assets.iter())
        .map(|a| json!({"node": a.node, "use": "image", "src": clean(&a.src, 200)}))
        .collect();
    json!({"root": p.root, "commands": plan.commands.len(), "nodes": nodes, "skipped": skipped,
        "assets": assets, "warnings": warnings(&added(p)), "report": report(&plan.report)})
}
