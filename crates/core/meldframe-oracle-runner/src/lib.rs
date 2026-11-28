//! The runner of the fidelity oracle (E4.2; `docs/fidelity-contract.md` section 7).
//!
//! The Node harness in `tools/fidelity-oracle` sends fixture files and a table of text measurements it recorded in
//! a browser. For each fixture this lays the whole file out with the layout core (`meldframe_layout::compute_rects`)
//! and exports each top-level frame with the export feature in both forms (`export::export_frame`), and returns
//! the rects and the two exports. The harness loads the exports in Chromium and compares the boxes the browser makes
//! with these rects.
//!
//! Text is the one thing the core cannot measure by itself. [`TableMeasurer`] answers from the recorded table; a
//! query the table lacks is answered by the stub measurer and returned as a miss, so that the harness can measure it
//! in the browser, add it to the table and run again until a run has no misses. The core asks for wrap widths that
//! depend on earlier answers, which is why one pass cannot know every query.

mod back;
mod page;
mod table;

pub use back::{Back, BackNode};
pub use page::{ImportAnswer, ImportCase, PageNode};
pub use table::{Entry, Query, TableMeasurer};

use export::{ExportOptions, Format};
use meldframe_layout::{Rect, compute_rects};
use meldframe_protocol::{Node, NodeKind, Variables};
use meldframe_variables::ModeChoice;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One fixture file: a whole file as a flat node list, with the variables its nodes bind to and the mode picked for each
/// collection (collection id to mode id; the others show their default mode).
#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub name: String,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub variables: Variables,
    #[serde(default)]
    pub modes: BTreeMap<String, String>,
}

/// What the harness sends.
#[derive(Debug, Deserialize)]
pub struct Input {
    pub fixtures: Vec<Fixture>,
    /// Hand-written pages to import and lay out (E4.4 slice 8).
    #[serde(default)]
    pub imports: Vec<ImportCase>,
    /// The text measurements recorded in the browser so far.
    #[serde(default)]
    pub table: Vec<Entry>,
}

/// One export of a frame: the HTML page, the stylesheet (the input of the Tailwind compiler in the Tailwind form) and
/// the codes of the report.
#[derive(Debug, Serialize)]
pub struct Exported {
    pub html: String,
    pub css: String,
    pub unsupported: Vec<String>,
    pub warnings: Vec<String>,
    /// The export imported back and laid out again (`back.rs`); `None` on the export an import made, which is not imported again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back: Option<Back>,
}

impl Exported {
    pub(crate) fn of(e: export::Export) -> Self {
        Exported {
            html: e.html,
            css: e.css,
            unsupported: codes(&e.report.unsupported),
            warnings: codes(&e.report.warnings),
            back: None,
        }
    }
}

/// A top-level frame and its two exports.
#[derive(Debug, Serialize)]
pub struct FrameExport {
    pub id: String,
    pub css: Option<Exported>,
    pub tailwind: Option<Exported>,
}

/// The answer for one fixture.
#[derive(Debug, Serialize)]
pub struct Answer {
    pub name: String,
    /// The core's box of every node, in file coordinates.
    pub rects: BTreeMap<String, Rect>,
    /// What the table lacked in this run; empty when the answer is final.
    pub misses: Vec<Query>,
    pub frames: Vec<FrameExport>,
    /// Why a root was not exported (the error code and the node), or an export failed.
    pub errors: Vec<String>,
}

/// What the runner returns.
#[derive(Debug, Serialize)]
pub struct Output {
    pub fixtures: Vec<Answer>,
    pub imports: Vec<ImportAnswer>,
}

fn codes(list: &[export::Finding]) -> Vec<String> {
    list.iter()
        .map(|f| match &f.node {
            Some(n) => format!("{}:{n}", f.code),
            None => f.code.to_owned(),
        })
        .collect()
}

fn one(
    f: &Fixture,
    frame: &str,
    format: Format,
    measurer: &TableMeasurer,
) -> Result<Exported, export::ExportError> {
    let choice = ModeChoice::of(f.modes.iter().map(|(c, m)| (c.as_str(), m.as_str())));
    let options = ExportOptions::new(format, measurer).modes(choice);
    let mut e = Exported::of(export::export_frame(
        &f.nodes,
        &f.variables,
        frame,
        &options,
    )?);
    let at = f
        .nodes
        .iter()
        .find(|n| n.id == frame)
        .map_or((0.0, 0.0), |n| (n.x, n.y));
    e.back = Some(back::reimport(&e, format, at, measurer));
    Ok(e)
}

fn answer(f: &Fixture, table: &[Entry]) -> Answer {
    let measurer = TableMeasurer::new(table);
    let rects: BTreeMap<String, Rect> = compute_rects(&f.nodes, &measurer).into_iter().collect();
    let mut frames = Vec::new();
    let mut errors = Vec::new();
    for root in f.nodes.iter().filter(|n| n.parent_id.is_none()) {
        if root.kind != NodeKind::Frame {
            errors.push(format!("E_NOT_A_FRAME:{}", root.id));
            continue;
        }
        let mut fe = FrameExport {
            id: root.id.clone(),
            css: None,
            tailwind: None,
        };
        for (format, slot) in [
            (Format::Css, &mut fe.css),
            (Format::Tailwind, &mut fe.tailwind),
        ] {
            match one(f, &root.id, format, &measurer) {
                Ok(e) => *slot = Some(e),
                Err(e) => errors.push(format!("{}:{}", e.code(), root.id)),
            }
        }
        frames.push(fe);
    }
    Answer {
        name: f.name.clone(),
        rects,
        misses: measurer.misses(),
        frames,
        errors,
    }
}

/// Run every fixture of the input.
pub fn run(input: &Input) -> Output {
    Output {
        fixtures: input
            .fixtures
            .iter()
            .map(|f| answer(f, &input.table))
            .collect(),
        imports: input
            .imports
            .iter()
            .map(|c| page::answer(c, &input.table))
            .collect(),
    }
}
