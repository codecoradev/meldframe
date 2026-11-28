//! An export imported back (`docs/import-html.md` section 3.7 and 5, point 4): the HTML of a frame goes through
//! `html_import::map::import`, the imported file is laid out with the same measurer as the original and exported again.

use crate::Exported;
use crate::page::kind;
use export::{ExportOptions, Format};
use html_import::map::{Options, import};
use meldframe_layout::{Rect, compute_rects};
use meldframe_protocol::Variables;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::table::TableMeasurer;

/// One node of the imported file, paired with the node of the original that its element came from.
#[derive(Debug, Serialize)]
pub struct BackNode {
    /// The id of the original node (the `data-node` of the element, in the order of the export).
    pub id: String,
    /// The id the import gave it.
    pub imported: String,
    pub kind: String,
    /// The layout core's box of the imported node.
    pub rect: Rect,
}

/// What the import of an export gave.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Back {
    pub nodes: Vec<BackNode>,
    /// `code:property:node` of each property the import could not take (an export has none).
    pub unsupported: Vec<String>,
    /// The code of a refusal, or the numbers when the export and the import do not hold the same count of nodes.
    pub error: Option<String>,
    /// The imported file exported again in the same form, to be loaded as the original's export is.
    pub again: Option<Box<Exported>>,
    /// The core's boxes of the imported file by imported id: what the page of `again` is compared with.
    pub again_rects: BTreeMap<String, Rect>,
}

fn node_ids(html: &str) -> Vec<String> {
    html.split(" data-node=\"")
        .skip(1)
        .map(|r| r.split('"').next().unwrap_or_default().to_owned())
        .collect()
}

/// Import `exported` (the Tailwind form comes back with the stylesheet it returns) and lay the result out where the original frame sits.
pub(crate) fn reimport(
    exported: &Exported,
    format: Format,
    origin: (f64, f64),
    measurer: &TableMeasurer,
) -> Back {
    let css = if format == Format::Tailwind {
        exported.css.as_str()
    } else {
        ""
    };
    let options = Options {
        origin,
        ..Options::default()
    };
    let imported = match import(&exported.html, css, &options) {
        Ok(i) => i,
        Err(e) => {
            return Back {
                error: Some(e.code().to_owned()),
                ..Back::default()
            };
        }
    };
    let nodes = imported.added_nodes();
    let rects: BTreeMap<String, Rect> = compute_rects(&nodes, measurer).into_iter().collect();
    let ids = node_ids(&exported.html);
    let mut back = Back::default();
    if ids.len() != nodes.len() {
        back.error = Some(format!(
            "count:exported {}, imported {}",
            ids.len(),
            nodes.len()
        ));
    }
    for (id, n) in ids.iter().zip(&nodes) {
        back.nodes.push(BackNode {
            id: id.clone(),
            imported: n.id.clone(),
            kind: kind(n.kind).to_owned(),
            rect: rects[&n.id],
        });
    }
    for n in &imported.nodes {
        for l in &n.unsupported {
            back.unsupported
                .push(format!("{}:{}:{}", l.code, l.property, n.id));
        }
    }
    let options = ExportOptions::new(format, measurer);
    if let Ok(e) = export::export_frame(&nodes, &Variables::default(), &imported.root, &options) {
        back.again = Some(Box::new(Exported::of(e)));
        back.again_rects = rects;
    }
    back
}
