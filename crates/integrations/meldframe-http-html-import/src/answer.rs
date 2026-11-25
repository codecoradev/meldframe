//! The answer of the tool: the nodes, what was skipped, the assets, the warnings and the report, as JSON a client
//! reads (`docs/import-html.md` section 4.2). Every string that came from the text is cleaned again here, so a control
//! or bidi character never reaches the answer, and everything is bounded: at most 100 nodes, the report's own bounds.

use crate::plan::Planned;
use html_import::html::{Entry, Report, clean};
use meldframe_protocol::NodeKind;
use serde_json::{Value, json};

/// The longest string of the text echoed in the answer, in characters.
const ECHO: usize = 240;

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
        "assets": assets, "report": report(&plan.report)})
}
