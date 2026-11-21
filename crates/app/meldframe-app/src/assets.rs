//! The image picker of the Fill section (E2.5, ADR-0015 follow-up): which node the picker is open for, and the page of
//! the workspace's assets it reads (`Effect::FetchAssets`, answered by `Action::AssetsLoaded`).
//! The picker is view state: nothing is written to the file. The rules are `AssetsState`'s and the two here.

use crate::action::Action;
use crate::app::App;
use crate::document::refuse;
use crate::effect::Effect;
use crate::event::Event;
use meldframe_app_model::{ASSET_PAGE, AssetsState};
use meldframe_protocol::{Node, NodeKind};

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
                self.state.assets.failed(request, code)
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
}

/// The code a request to open the picker for node `id` is refused with, when it is: no such node, the node is not the one
/// selected, or it is text, which takes no image.
fn open_refusal(nodes: &[Node], selection: &[String], id: &str) -> Option<&'static str> {
    match nodes.iter().find(|n| n.id == id) {
        None => Some("not_found"),
        Some(_) if !matches!(selection, [only] if only == id) => Some("not_selected"),
        Some(n) if n.kind == NodeKind::Text => Some("image_on_text"),
        Some(_) => None,
    }
}

/// Whether the picker open for node `id` still has what it needs: the node is there and is the one selected.
fn is_open_valid(nodes: &[Node], selection: &[String], id: &str) -> bool {
    matches!(selection, [only] if only == id) && nodes.iter().any(|n| n.id == id)
}
