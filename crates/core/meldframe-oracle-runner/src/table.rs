//! The text measurer of the oracle: answers from a table recorded in a browser and remembers what it lacked.

use meldframe_layout::{StubMeasurer, TextMeasurer};
use meldframe_protocol::Node;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

/// One question to a measurer: the text, the font size and weight, and the wrap width (`None` is one line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub text: String,
    pub font_size: f64,
    pub font_weight: u16,
    pub wrap_w: Option<f64>,
}

/// One recorded answer: the width and the height the browser measured.
#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    #[serde(flatten)]
    pub query: Query,
    pub w: f64,
    pub h: f64,
}

type Key = (String, u64, u16, Option<i64>);

/// A wrap width in 1/64 px, the grid a browser lays out on (rule G5 of the fidelity contract). `serde_json` without its
/// `float_roundtrip` feature can read a number one bit away from the one that was written, so the key may not be exact.
fn grid(wrap_w: f64) -> i64 {
    (wrap_w * 64.0).round() as i64
}

fn key(q: &Query) -> Key {
    (
        q.text.clone(),
        q.font_size.to_bits(),
        q.font_weight,
        q.wrap_w.map(grid),
    )
}

/// Replays the table. The key is the text, the size, the weight and the wrap width on a 1/64 px grid: a width that
/// differs in the last bit, as one read back from JSON may, is the same query (the fidelity contract, section 6.5, saw
/// 2 of 200 fuzz files miss the table for that reason).
#[derive(Debug)]
pub struct TableMeasurer {
    known: HashMap<Key, (f64, f64)>,
    missed: RefCell<BTreeSet<String>>,
    misses: RefCell<Vec<Query>>,
}

impl TableMeasurer {
    pub fn new(table: &[Entry]) -> Self {
        TableMeasurer {
            known: table.iter().map(|e| (key(&e.query), (e.w, e.h))).collect(),
            missed: RefCell::default(),
            misses: RefCell::default(),
        }
    }

    /// The queries the table lacked, each once, in the order they were first asked.
    pub fn misses(&self) -> Vec<Query> {
        self.misses.borrow().clone()
    }
}

impl TextMeasurer for TableMeasurer {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let q = Query {
            text: node.text.clone(),
            font_size: node.font_size,
            font_weight: node.font_weight,
            wrap_w,
        };
        if let Some(&answer) = self.known.get(&key(&q)) {
            return answer;
        }
        let id = format!("{:?}", key(&q));
        if self.missed.borrow_mut().insert(id) {
            self.misses.borrow_mut().push(q);
        }
        StubMeasurer.measure(node, wrap_w)
    }
}
