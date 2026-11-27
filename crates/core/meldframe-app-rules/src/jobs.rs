//! The requests for an image that are out (finding T13 of the thin UI audit, decision 43): one job for each request, from the
//! plan to the bytes reaching their sink. A table with no `App` in it: a job is taken out of it by the first answer for its
//! stage, so a repeated or late answer finds nothing, and every request is answered once. What decides each step is
//! `png`.

use crate::raster::Planned;
use meldframe_app_model::PngSink;

/// What the image is for.
#[derive(Debug)]
pub enum Kind {
    /// A PNG export of a person: where it goes and the name to save it under.
    Export { sink: PngSink, file: String },
}

/// Where a request is: not asked of the shell yet, being drawn, or its bytes on their way to the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Waiting,
    Drawing,
    Delivering,
}

#[derive(Debug)]
pub struct Job {
    pub request: u32,
    pub kind: Kind,
    /// The node, or none for the whole page.
    pub node: Option<String>,
    pub scale: f64,
    pub planned: Planned,
    pub max_pixels: u64,
    pub stage: Stage,
}

/// The jobs and the number of the next request.
#[derive(Debug, Default)]
pub struct Jobs {
    next: u32,
    jobs: Vec<Job>,
}

impl Jobs {
    pub fn number(&mut self) -> u32 {
        self.next = self.next.wrapping_add(1);
        self.next
    }

    pub fn push(&mut self, job: Job) {
        self.jobs.push(job);
    }

    /// The job of `request` when it is at `stage`, taken out of the table.
    pub fn take(&mut self, request: u32, stage: Stage) -> Option<Job> {
        let at = (self.jobs.iter()).position(|j| j.request == request && j.stage == stage)?;
        Some(self.jobs.remove(at))
    }

    /// Every job `pick` says yes to, taken out of the table in the order they were made.
    pub fn take_where(&mut self, pick: impl Fn(&Job) -> bool) -> Vec<Job> {
        let (taken, kept) = std::mem::take(&mut self.jobs).into_iter().partition(pick);
        self.jobs = kept;
        taken
    }
}
