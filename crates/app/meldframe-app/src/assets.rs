//! The image picker of the Fill section and the choice of an image (E2.5, ADR-0015 follow-up): which node the picker is
//! open for, the page of the workspace's assets it reads (`Effect::FetchAssets`, answered by `Action::AssetsLoaded`), and
//! `Action::SetImageFill`; the picker is view state, so nothing is written to the file until an image is chosen, and that is
//! one undo step, with the rules of are those of `parts::image` and `AssetsState`.

use crate::action::Action;
use crate::app::App;
use crate::document::refuse;
use crate::effect::Effect;
use crate::event::Event;
use crate::parts::document::update;
use crate::parts::image::{image_fill_patch, is_open_valid, open_refusal};
use meldframe_app_model::{ASSET_PAGE, AssetsState};
use meldframe_protocol::ImageFit;
use meldframe_sync::SubmitError;

impl App {
    pub(crate) fn assets_action(&mut self, action: Action, fx: &mut Vec<Effect>) {
        // The front end redraws from events: one is raised for each change of `AppState::assets`, and none for a dispatch
        // that leaves it as it was.
        let changed = match action {
            Action::OpenAssets { id } => {
                if let Some(code) = open_refusal(self.nodes(), &self.state.selection, &id) {
                    drop(refuse(&mut self.events, code));
                    self.state.assets.close()
                } else {
                    let request = self.state.assets.open(id);
                    fx.push(Effect::FetchAssets {
                        after: None,
                        limit: ASSET_PAGE,
                        request,
                    });
                    true
                }
            }
            Action::CloseAssets {} => self.state.assets.close(),
            Action::LoadMoreAssets {} => match self.state.assets.more() {
                Some((after, request)) => {
                    fx.push(Effect::FetchAssets {
                        after: Some(after),
                        limit: ASSET_PAGE,
                        request,
                    });
                    true
                }
                None => false,
            },
            Action::RetryAssets {} => match self.state.assets.retry() {
                Some((after, request)) => {
                    fx.push(Effect::FetchAssets {
                        after,
                        limit: ASSET_PAGE,
                        request,
                    });
                    true
                }
                None => false,
            },
            Action::AssetsLoaded {
                request,
                after,
                page,
            } => self.state.assets.arrived(request, after.as_deref(), page),
            Action::AssetsRequestFailed { request, code } => {
                self.state.assets.failed(request, crate::text::code(&code))
            }
            _ => false,
        };
        if changed {
            self.events.push(Event::AssetsChanged {});
        }
    }

    /// The open picker closes when its node is no longer the one selected (or is gone). Nothing while it is closed.
    pub(crate) fn refresh_assets(&mut self) {
        let AssetsState {
            target: Some(id), ..
        } = &self.state.assets
        else {
            return;
        };
        if !is_open_valid(self.nodes(), &self.state.selection, id) {
            self.state.assets.close();
            self.events.push(Event::AssetsChanged {});
        }
    }

    pub(crate) fn set_image_fill(
        &mut self,
        id: &str,
        asset_id: String,
        fit: Option<ImageFit>,
    ) -> Result<(), SubmitError> {
        let Some(node) = self.nodes().iter().find(|n| n.id == id) else {
            return refuse(&mut self.events, "not_found");
        };
        let Some(patch) = image_fill_patch(node, asset_id, fit) else {
            // Already this image: nothing to change, and the picker has done its work.
            self.close_assets_for(id);
            return Ok(());
        };
        self.replica.apply(&update(id, patch))?;
        self.close_assets_for(id);
        Ok(())
    }

    /// The image of node `id` is settled: the picker open for it closes. One that was refused leaves it open.
    fn close_assets_for(&mut self, id: &str) {
        if self.state.assets.target.as_deref() == Some(id) && self.state.assets.close() {
            self.events.push(Event::AssetsChanged {});
        }
    }
}
