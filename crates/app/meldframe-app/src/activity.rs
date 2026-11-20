//! The activity panel in the app layer (E5.1, ADR-0021): what feeds the reducer, the fetches it asks for, the state.
use crate::{action::Action, action::Frame, app::App, document::refuse};
use crate::{effect::Effect, event::Event, state::Connection};
use meldframe_activity::{Feed, Fetch};
impl App {
    /// Bring the panel's state up to date with the reducer (once per dispatch), and say so when it moved.
    pub(crate) fn activity_changed(&mut self) {
        let (log, seq) = (&self.review.log, self.replica.seq());
        let state = &mut self.state.activity;
        if state.refresh(&mut self.feed, log, seq, self.replica.nodes()) {
            self.events.push(Event::ActivityChanged {});
        }
    }

    /// The fetch a rule of the feed wants goes out, only while live.
    pub(crate) fn fetched(
        &mut self,
        f: impl FnOnce(&mut Feed, bool) -> Option<Fetch>,
        fx: &mut Vec<Effect>,
    ) {
        let live = self.state.connection == Connection::Live;
        fx.extend(f(&mut self.feed, live).filter(|_| live).map(Effect::from));
    }

    pub(crate) fn activity_frame(&mut self, now_ms: u64, frame: Frame, fx: &mut Vec<Effect>) {
        self.fetched(|f, _| frame.observe(f, now_ms), fx);
    }

    pub(crate) fn activity_action(&mut self, action: Action, fx: &mut Vec<Effect>) {
        match action {
            Action::OpenActivity {} => self.fetched(|_, _| Some(Fetch::NEWEST), fx),
            Action::LoadOlderBatches {} => self.fetched(|f, _| f.older(), fx),
            Action::BatchesLoaded { page } => self.fetched(|f, _| f.page(page.into()), fx),
            Action::BatchReviewed { batch } => self.feed.red.observe_row(batch),
            Action::BatchRequestFailed { code } => {
                drop(refuse(&mut self.events, &code));
                self.fetched(|f, _| f.failed(), fx);
            }
            Action::SelectBatch { batch } => {
                match self.feed.select(&batch.actor, &batch.key_prefix) {
                    Some(ids) => self.select(ids),
                    None => drop(refuse(&mut self.events, "unknown_batch")),
                }
            }
            Action::SetBatchReview { batch, state } => {
                match self.feed.number(&batch.actor, &batch.key_prefix) {
                    Ok(n) => fx.push(Effect::PutBatchReview { n, state }),
                    Err(code) => drop(refuse(&mut self.events, code)),
                }
            }
            _ => {}
        }
    }
}
