//! The images a workspace holds, as the image picker of the Fill section lists them (E2.5, ADR-0015 follow-up).
//! An asset has no name yet: it is told by its id (the content hash), the size in bytes and the type, in the order
//! of the ids (the server keeps no upload time). The state is the page or pages the app has read; the transitions
//! are the plain functions here, so a front end and a test see the same ones. The answer of the server comes from the
//! network, so the app checks it too (`AssetPage::is_valid`): a front end may check it as well, and none has to.

use crate::action::Action;
use crate::wire::MAX_SAFE_INT;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many assets one request asks for. Each tile of the picker fetches its image, so a page stays well inside the server's
/// per-client burst (60 calls, then 30 a second) with the list request and the canvas's own fetches (the route's own default is 50).
pub const ASSET_PAGE: u32 = 24;

/// The most assets one page may hold: the most `GET /v1/assets` answers (it reads a larger `limit` as 200). A page with
/// more is not the server's.
pub const ASSET_PAGE_MAX: usize = 200;

/// The most assets the picker lists in all. A grid of thousands of tiles is no way to choose one image, and every tile
/// fetches its image, so the list stops here: `AssetsState::capped` says so and there is no more to load.
pub const ASSET_LIST_MAX: usize = 500;

/// The types the server serves (SVG is refused at upload).
const MIMES: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];

/// Whether `id` has the form of an asset id: 64 lowercase hex characters (the SHA-256 of the bytes).
pub fn is_asset_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// One asset of the workspace, as `GET /v1/assets` lists it. No content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetInfo {
    /// The lowercase hex SHA-256 of the bytes: the id `image.assetId` takes.
    pub id: String,
    #[ts(type = "number")]
    pub bytes: u64,
    /// The media type told by the first bytes (`image/png`, `image/jpeg`, `image/webp`, `image/gif`).
    pub mime: String,
}

impl AssetInfo {
    /// Whether it is what the server lists: an id of the right form, a size a front end can hold and one of the four types.
    pub fn is_valid(&self) -> bool {
        is_asset_id(&self.id) && self.bytes <= MAX_SAFE_INT && MIMES.contains(&self.mime.as_str())
    }
}

/// A page of the list, as the server answers it (the answer of `Effect::FetchAssets`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetPage {
    pub assets: Vec<AssetInfo>,
    /// The cursor of the page after this one (the last id of this page); none on the last page.
    #[serde(default)]
    pub next: Option<String>,
}

impl AssetPage {
    /// Whether it is a page the server could have sent: at most [`ASSET_PAGE_MAX`] valid assets, and a cursor that is the
    /// id of the last of them (there is none on a page that holds nothing, and a last page has none).
    pub fn is_valid(&self) -> bool {
        self.assets.len() <= ASSET_PAGE_MAX
            && self.assets.iter().all(AssetInfo::is_valid)
            && self
                .next
                .as_ref()
                .is_none_or(|n| self.assets.last().is_some_and(|l| &l.id == n))
    }
}

/// The image picker: the node it picks for while it is open, and the assets read so far. The list is held only while the
/// picker is open: closing it forgets the list, so a snapshot of the app does not carry it for the rest of the session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetsState {
    /// The node the open picker chooses an image for; none while it is closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target: Option<String>,
    /// The assets read, in the order of their ids, without repeats.
    pub items: Vec<AssetInfo>,
    /// The cursor to ask the next page with; none when the workspace has no more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next: Option<String>,
    /// A request is out.
    pub loading: bool,
    /// A page has arrived since the picker was opened (so an empty list is told from one not read yet).
    pub loaded: bool,
    /// The list reached [`ASSET_LIST_MAX`] and the workspace holds more: `next` is none and there is no Load more.
    #[serde(default)]
    pub capped: bool,
    /// The code the last request failed with (`unauthenticated`, `unavailable`, ...); none after a page arrives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
    /// The number of the last request made (`Effect::FetchAssets::request`). An answer carries the number of the request it
    /// answers and is taken only when it is this one, so a late answer of an older request, or of one made before the
    /// picker was closed or opened again, changes nothing.
    #[serde(default)]
    pub request: u32,
}

impl AssetsState {
    /// The picker opens for `target`: whatever was read is forgotten and the list is read from its start (an agent may have
    /// uploaded since). Returns the number of the request to make. An answer of an earlier request is no longer taken.
    pub fn open(&mut self, target: String) -> u32 {
        let request = self.request.wrapping_add(1);
        *self = AssetsState {
            target: Some(target),
            loading: true,
            request,
            ..AssetsState::default()
        };
        request
    }

    /// The cursor and the request number of the request `LoadMoreAssets` makes, and the state for it: none while the picker is
    /// closed, while a request is out or when there is no more.
    pub fn more(&mut self) -> Option<(String, u32)> {
        if self.target.is_none() || self.loading {
            return None;
        }
        let after = self.next.clone()?;
        self.loading = true;
        self.error = None;
        self.request = self.request.wrapping_add(1);
        Some((after, self.request))
    }

    /// The cursor and the request number of the request `RetryAssets` makes, and the state for it: only while the picker is
    /// open, no request is out and the last one failed. It asks for the page after what is held, or for the first page when
    /// nothing was read (the cursor is none then).
    pub fn retry(&mut self) -> Option<(Option<String>, u32)> {
        if self.target.is_none() || self.loading || self.error.is_none() {
            return None;
        }
        self.loading = true;
        self.error = None;
        self.request = self.request.wrapping_add(1);
        Some((self.next.clone(), self.request))
    }

    /// A page arrived for request number `request`, which asked `after` (none: the first page). It is taken only while the
    /// picker is open and that request is the one out. A page that is not valid (`AssetPage::is_valid`), or that does not
    /// follow what is held, ends the request as failed with the code `invalid`: what was read stays, and so does the
    /// cursor, so the person can try again. The first page replaces the list; a later one is added after the last asset
    /// held, an id already held is not listed twice, and the list stops at [`ASSET_LIST_MAX`]. Whether the state changed
    /// (false: the answer was not for this request, or the picker is closed).
    pub fn arrived(&mut self, request: u32, after: Option<&str>, page: AssetPage) -> bool {
        if self.target.is_none() || !self.loading || request != self.request {
            return false;
        }
        let follows = match after {
            None => true,
            Some(a) => self.items.last().is_some_and(|l| l.id == a),
        };
        if !follows || !page.is_valid() {
            return self.failed(request, "invalid".into());
        }
        if after.is_none() {
            self.items.clear();
        }
        for a in page.assets {
            if self.items.len() >= ASSET_LIST_MAX {
                break;
            }
            if !self.items.iter().any(|i| i.id == a.id) {
                self.items.push(a);
            }
        }
        // Past the cap with more to read: say so and stop offering more.
        self.capped = page.next.is_some() && self.items.len() >= ASSET_LIST_MAX;
        self.next = if self.capped { None } else { page.next };
        self.loading = false;
        self.loaded = true;
        self.error = None;
        true
    }

    /// Request number `request` failed with `code`: what was read stays, and the cursor with it, so the person can try
    /// again. Taken under the same conditions as a page. Whether it was.
    pub fn failed(&mut self, request: u32, code: String) -> bool {
        if self.target.is_none() || !self.loading || request != self.request {
            return false;
        }
        self.loading = false;
        self.error = Some(code);
        true
    }

    /// The picker closes and forgets the list. A request still out is no longer answered for. Whether it was open.
    pub fn close(&mut self) -> bool {
        let was = self.target.is_some();
        if was {
            *self = AssetsState {
                request: self.request.wrapping_add(1),
                ..AssetsState::default()
            };
        }
        was
    }
}

impl Action {
    /// Whether the image picker handles it (`meldframe-app`, `assets.rs`). `SetImageFill` changes the document and is not one.
    pub fn is_assets(&self) -> bool {
        matches!(
            self,
            Action::OpenAssets { .. }
                | Action::CloseAssets {}
                | Action::LoadMoreAssets {}
                | Action::RetryAssets {}
                | Action::AssetsLoaded { .. }
                | Action::AssetsRequestFailed { .. }
        )
    }
}
