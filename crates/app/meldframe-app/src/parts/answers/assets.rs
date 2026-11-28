//! The asset list route: the page of images the picker lists.

use super::error_code;
use meldframe_app_model::{Action, AssetPage};

fn failed(request: u32, code: &str) -> Action {
    Action::AssetsRequestFailed {
        request,
        code: code.into(),
    }
}

/// The answer of `GET /v1/assets` for request number `request`, which asked after `after`: the page, or the failure it
/// stands for. A page holds at most `ASSET_PAGE_MAX` assets, each with an id of the right form, a size a front end can hold
/// and one of the four types the server serves (`AssetPage::is_valid` says all of it); the cursor is none or an id. A
/// field of the page or of an asset that the app does not know is ignored.
pub(super) fn page(request: u32, after: Option<String>, status: u16, body: &str) -> Action {
    if status != 200 {
        return failed(request, &error_code(body));
    }
    match serde_json::from_str::<AssetPage>(body) {
        Ok(page) if page.is_valid() => Action::AssetsLoaded {
            request,
            after,
            page,
        },
        _ => failed(request, "unavailable"),
    }
}
