//! [`Activity`], the one constructor of the activity panel's server side, and the publisher of its live frame.

use crate::reviewers::Reviewers;
use crate::sink::Sink;
use crate::{review, route};
use activity_store::BatchStore;
use meldframe_activity::BatchSink;
use meldframe_host::Principal;
use meldframe_http::Shared;
use meldframe_http::mcp::Reader;
use meldframe_module::Policy;
use meldframe_protocol::ServerMsg;
use std::sync::Arc;

/// Queues a frame on the live streams of a file of the principal's workspace, as ephemeral and with headroom; answers
/// how many streams took it. The server's `SharedServer::publish_with_headroom` is the one.
pub type Publish = Arc<dyn Fn(&Principal, &str, ServerMsg) -> usize + Send + Sync>;

/// The activity panel's server side: the store of batch headers and the reader that counts a failed call's keys.
#[derive(Debug)]
pub struct Activity {
    sink: Arc<Sink>,
    setter: Arc<review::Setter>,
}

impl Activity {
    /// Headers are kept and listed, and no frame is sent.
    pub fn new(store: Arc<dyn BatchStore>, reader: Arc<dyn Reader>) -> Self {
        Self::with_live(store, reader, None)
    }

    /// As [`Activity::new`], and a header that changed is also sent through `live`.
    pub fn with_live(
        store: Arc<dyn BatchStore>,
        reader: Arc<dyn Reader>,
        live: Option<Publish>,
    ) -> Self {
        let setter = review::Setter::new(reader.clone(), live.clone());
        Activity {
            sink: Arc::new(Sink {
                store,
                reader,
                live,
            }),
            setter: Arc::new(setter),
        }
    }

    /// Names who may set a review state and what the registry's policy for [`REVIEW_TOOL`] is. Without it nobody
    /// may: the setter answers `forbidden` to every caller and `canReview` is false.
    #[must_use]
    pub fn with_review(mut self, reviewers: Reviewers, policy: Option<Policy>) -> Self {
        self.setter = Arc::new(self.setter.with(reviewers, policy));
        self
    }

    /// `GET /v1/files/{file}/batches` and `PUT /v1/files/{file}/batches/{n}/review`, on the limits and the tokens of
    /// `shared`.
    pub fn routes(&self, shared: Arc<Shared>) -> axum::Router {
        route::routes(shared, self.sink.store.clone(), self.setter.clone())
    }

    /// The sink to set on `meldframe_http::Shared::batches`.
    pub fn sink(&self) -> Arc<dyn BatchSink> {
        self.sink.clone()
    }
}
