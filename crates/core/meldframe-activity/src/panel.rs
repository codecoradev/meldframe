//! The panel as the app layer shows it: rows, lanes and rings with every fact the reducer cannot know alone (what the
//! record holds, what a revert would still change, which nodes a working lane is on). Plain data; the app layer's model
//! only turns it into the serialisable state.

use crate::{Feed, InEffect, Lane, MAX_ROWS, Row};
use meldframe_protocol::Node;
use meldframe_revert::ConfirmedLog;
use std::collections::HashSet;

/// The ids a persistent ring of a working lane covers, at most.
pub const RING_IDS: usize = 16;

/// A batch row with what the app's record says of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRow {
    pub row: Row,
    /// The record holds every op of the batch: the app saw an op of it (`has_ops`), the record still replays and the
    /// batch was not folded into its base. A batch with nothing to show (a delete alone, variable commands only) counts.
    pub revertable: bool,
    /// Asked for the selected row only.
    pub in_effect: InEffect,
}

/// A touch ring: nodes an actor is on. Not `persistent`: a hint that is gone 1.5 seconds after it came. Persistent: the
/// top-level nodes of the newest batch of a lane that is working (at most [`RING_IDS`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRing {
    pub actor: String,
    pub ids: Vec<String>,
    pub at_ms: u64,
    pub persistent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub rows: Vec<ViewRow>,
    pub lanes: Vec<Lane>,
    pub rings: Vec<ViewRing>,
    /// Older batches may be fetched: the numbers reach below the lowest held, and the rows are not at the cap.
    pub has_older: bool,
    pub unreviewed: u32,
    /// The selected batch, when it is still a row.
    pub selected: Option<(String, String)>,
    pub can_review: bool,
}

impl Feed {
    /// The panel now. A picked row that is gone is dropped; `log` and `seq` are the app's
    /// confirmed record and its newest number; `nodes` are the document as the person sees it.
    pub fn view(&mut self, log: &ConfirmedLog, seq: u64, nodes: &[Node]) -> View {
        let selected = self.selected.clone();
        let mut picked = None;
        let mut rows = Vec::new();
        for row in self.red.rows() {
            let mut in_effect = InEffect::Unknown;
            if selected
                .as_ref()
                .is_some_and(|s| (&s.0, &s.1) == (&row.actor, &row.key_prefix))
            {
                picked = selected.clone();
                in_effect = self.in_effect(log, seq, &row.actor, &row.key_prefix);
            }
            // `batch_is_partial` is a lookup; `crate::revertable` says the same by a scan of the record, too slow for
            // every row on every op.
            let revertable = row.has_ops
                && !log.diverged()
                && !log.batch_is_partial(&row.actor, &row.key_prefix);
            rows.push(ViewRow {
                row,
                revertable,
                in_effect,
            });
        }
        self.selected = picked.clone();
        let red = &self.red;
        let lanes = red.lanes();
        let mut rings: Vec<ViewRing> = red
            .rings()
            .iter()
            .map(|r| ViewRing {
                actor: r.actor.clone(),
                ids: r.ids.clone(),
                at_ms: r.at_ms,
                persistent: false,
            })
            .collect();
        for l in lanes
            .iter()
            .filter(|l| l.state == crate::LaneState::Working && l.actor != red.me())
        {
            let shown = l.batches.first().map(|p| red.show_ids(&l.actor, p));
            let set: HashSet<&str> = shown.iter().flatten().map(String::as_str).collect();
            let ids: Vec<String> = nodes
                .iter()
                .filter(|n| set.contains(n.id.as_str()))
                .filter(|n| n.parent_id.as_deref().is_none_or(|p| !set.contains(p)))
                .map(|n| n.id.clone())
                .take(RING_IDS)
                .collect();
            if !ids.is_empty() {
                let ring = ViewRing {
                    actor: l.actor.clone(),
                    ids,
                    at_ms: l.last_at_ms,
                    persistent: true,
                };
                rings.push(ring);
            }
        }
        View {
            has_older: red.lowest_n().is_some_and(|n| n > 1) && rows.len() < MAX_ROWS,
            unreviewed: red.unreviewed_count(),
            selected: picked,
            can_review: self.can_review,
            rows,
            lanes,
            rings,
        }
    }
}
