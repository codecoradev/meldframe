//! Assets feature module (E2.5): the images that image fills point at.
//!
//! - An upload is png, jpeg, webp or gif, told by its first bytes (never by a name or a content type the client
//!   sends). SVG and anything else is refused with `asset_type_refused`: SVG can carry script and external
//!   references, so it is not served back to a browser.
//! - An asset is named by the lowercase hex SHA-256 of its bytes ([`asset_id`]), the form `image.assetId` already
//!   takes (`meldframe-check`). The same bytes are the same asset; storing them again writes nothing unless the stored file no longer
//!   matches (then it is written anew).
//! - Assets belong to one workspace (E2.3): the store keys them by workspace and id, and a workspace never reads
//!   another one's asset, even with its id (`asset_not_found`).
//! - [`BlobStore`] is the seam; [`MemoryBlobs`] keeps them in memory, `DiskBlobs` on local disk (not built for
//!   wasm32, which has no file system).
//!
//! The codes are in `docs/error-codes.md` (section "Asset codes").

#[cfg(not(target_arch = "wasm32"))]
mod disk;
mod usage;

pub use usage::{Reservation, Reserve, Usage};

#[cfg(not(target_arch = "wasm32"))]
pub use disk::DiskBlobs;

use meldframe_protocol::is_asset_id;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

/// The largest upload: 10 MiB. The transport must stop reading past it.
pub const MAX_ASSET_BYTES: usize = 10 * 1024 * 1024;

/// The image types an asset can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageType {
    Png,
    Jpeg,
    Webp,
    Gif,
}

impl ImageType {
    /// The media type to serve it with.
    pub fn mime(self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Webp => "image/webp",
            ImageType::Gif => "image/gif",
        }
    }
}

/// Why an asset call failed. [`AssetError::code`] is the stable code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// The upload is empty, or an id is not a lowercase hex SHA-256.
    Invalid(String),
    /// The upload is larger than the limit.
    TooLarge { limit: usize },
    /// The bytes are not png, jpeg, webp or gif (SVG included).
    Unsupported,
    /// No asset with this id in this workspace.
    NotFound,
    /// The store failed.
    Unavailable(String),
    /// A stored file no longer matches its id (changed on disk, or cut short by a crash). Uploading the same
    /// image again repairs it.
    Corrupt,
}

impl AssetError {
    pub fn code(&self) -> &'static str {
        match self {
            AssetError::Invalid(_) => "invalid",
            AssetError::TooLarge { .. } => "asset_too_large",
            AssetError::Unsupported => "asset_type_refused",
            AssetError::NotFound => "asset_not_found",
            AssetError::Unavailable(_) => "unavailable",
            AssetError::Corrupt => "asset_corrupt",
        }
    }

    /// Whether the same call can succeed later unchanged: only when the store failed.
    pub fn retryable(&self) -> bool {
        matches!(self, AssetError::Unavailable(_))
    }
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::Invalid(why) => write!(f, "{why}"),
            AssetError::TooLarge { limit } => write!(f, "an asset is at most {limit} bytes"),
            AssetError::Unsupported => write!(
                f,
                "an asset is a png, jpeg, webp or gif image (SVG is refused)"
            ),
            AssetError::NotFound => write!(f, "no asset with this id in this workspace"),
            AssetError::Unavailable(_) => write!(f, "the asset store is unavailable"),
            AssetError::Corrupt => write!(
                f,
                "the stored image no longer matches its id: upload it again"
            ),
        }
    }
}

/// The type of an image from its first bytes, or `None`.
pub fn sniff(bytes: &[u8]) -> Option<ImageType> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageType::Png)
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(ImageType::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageType::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageType::Webp)
    } else {
        None
    }
}

/// The id of an asset: the lowercase hex SHA-256 of its bytes.
pub fn asset_id(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The checks of an upload, in order: not empty, not over [`MAX_ASSET_BYTES`] (before it is hashed), a known
/// image type. Returns the type and the id.
pub fn check_upload(bytes: &[u8]) -> Result<(ImageType, String), AssetError> {
    if bytes.is_empty() {
        return Err(AssetError::Invalid("the upload is empty".into()));
    }
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(AssetError::TooLarge {
            limit: MAX_ASSET_BYTES,
        });
    }
    let kind = sniff(bytes).ok_or(AssetError::Unsupported)?;
    Ok((kind, asset_id(bytes)))
}

fn check_id(id: &str) -> Result<(), AssetError> {
    if is_asset_id(id) {
        Ok(())
    } else {
        Err(AssetError::Invalid(
            "an asset id is a lowercase hex SHA-256 (64 characters)".into(),
        ))
    }
}

/// What a stored asset is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub id: String,
    pub kind: ImageType,
    pub bytes: usize,
    /// This call wrote the file; `false` when the workspace held the image already (or another call wrote it
    /// first), so the storage quota charges it once.
    pub created: bool,
}

/// The most assets one [`BlobStore::list`] page holds.
pub const MAX_LIST: usize = 200;

/// One asset in a listing: what is known without reading the whole file. There is no name and no upload time (the
/// store keeps neither), so a listing shows the id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub kind: ImageType,
    pub bytes: usize,
}

/// A page of a listing. `next` is the cursor for the page after it (the last id of this page; pass it as `after`)
/// and is `None` on the last page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    pub items: Vec<Listed>,
    pub next: Option<String>,
}

/// The seam: keep the assets of each workspace. Methods take `&self` and a store is `Sync`: calls run at once, so a
/// slow write of one image never makes a read of another wait. Every method checks its input first ([`check_upload`], the form of
/// the id), so no implementation sees an unchecked id or an unchecked upload.
pub trait BlobStore: Send + Sync {
    /// Store `bytes` for `workspace`; the same bytes again are the same asset and write nothing, unless the stored
    /// copy no longer matches them.
    fn put(&self, workspace: &str, bytes: &[u8]) -> Result<Stored, AssetError>;
    /// The type and the bytes of an asset of `workspace`.
    fn get(&self, workspace: &str, id: &str) -> Result<(ImageType, Vec<u8>), AssetError>;
    /// The bytes of every asset `workspace` holds (what its storage quota counts).
    fn used(&self, workspace: &str) -> Result<u64, AssetError>;
    /// The assets of `workspace`, at most `limit` (kept between 1 and [`MAX_LIST`]) with an id after `after`, in the
    /// order of their ids. That order is stable (an asset added later shows up where its id sorts and nothing is
    /// repeated or lost between pages) but it is not the order of upload: the store keeps no upload time, and a
    /// newest-first list needs a metadata index (ADR-0015). `after` is checked like an id. A file that is not an
    /// image is not listed; the bytes are checked against the id only by [`BlobStore::get`].
    fn list(&self, workspace: &str, after: Option<&str>, limit: usize) -> Result<Page, AssetError>;
    /// Whether `workspace` has the asset.
    fn has(&self, workspace: &str, id: &str) -> Result<bool, AssetError> {
        match self.get(workspace, id) {
            Ok(_) => Ok(true),
            Err(AssetError::NotFound) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

/// The first `limit` of `sorted` (one more than a page is enough) as a page: a cursor only when more follow.
fn page_of(mut sorted: Vec<Listed>, limit: usize) -> Page {
    let limit = limit.clamp(1, MAX_LIST);
    let more = sorted.len() > limit;
    sorted.truncate(limit);
    let next = if more {
        sorted.last().map(|l| l.id.clone())
    } else {
        None
    };
    Page {
        items: sorted,
        next,
    }
}

/// Assets in memory, for tests and a server without a disk.
#[derive(Debug, Default)]
pub struct MemoryBlobs {
    blobs: Mutex<Blobs>,
}

/// The assets by workspace and id.
type Blobs = HashMap<(String, String), (ImageType, Vec<u8>)>;

/// Insert only what is not there; whether it was.
trait InsertNew {
    fn insert_new(&mut self, key: (String, String), value: (ImageType, Vec<u8>)) -> bool;
}

impl InsertNew for Blobs {
    fn insert_new(&mut self, key: (String, String), value: (ImageType, Vec<u8>)) -> bool {
        match self.entry(key) {
            std::collections::hash_map::Entry::Occupied(_) => false,
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(value);
                true
            }
        }
    }
}

impl BlobStore for MemoryBlobs {
    fn put(&self, workspace: &str, bytes: &[u8]) -> Result<Stored, AssetError> {
        let (kind, id) = check_upload(bytes)?;
        let created = self
            .blobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert_new((workspace.to_owned(), id.clone()), (kind, bytes.to_vec()));
        Ok(Stored {
            id,
            kind,
            bytes: bytes.len(),
            created,
        })
    }

    fn list(&self, workspace: &str, after: Option<&str>, limit: usize) -> Result<Page, AssetError> {
        if let Some(after) = after {
            check_id(after)?;
        }
        let blobs = self
            .blobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut held: Vec<Listed> = blobs
            .iter()
            .filter(|((w, id), _)| w == workspace && after.is_none_or(|a| id.as_str() > a))
            .map(|((_, id), (kind, bytes))| Listed {
                id: id.clone(),
                kind: *kind,
                bytes: bytes.len(),
            })
            .collect();
        held.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(page_of(held, limit))
    }

    fn used(&self, workspace: &str) -> Result<u64, AssetError> {
        let blobs = self
            .blobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(blobs
            .iter()
            .filter(|((w, _), _)| w == workspace)
            .map(|(_, (_, bytes))| bytes.len() as u64)
            .sum())
    }

    fn get(&self, workspace: &str, id: &str) -> Result<(ImageType, Vec<u8>), AssetError> {
        check_id(id)?;
        self.blobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(workspace.to_owned(), id.to_owned()))
            .cloned()
            .ok_or(AssetError::NotFound)
    }
}

/// The name of the agent tool that uploads an image.
pub const UPLOAD_TOOL: &str = "upload_asset";

/// What the `upload_asset` tool says about itself: it is the text an agent reads.
pub const UPLOAD_DESCRIPTION: &str = "Upload an image to the workspace so that a node can use it as an image fill. Pass data, the bytes of one png, jpeg, webp or gif image as base64 (SVG and anything else is refused with asset_type_refused; at most 10 MiB, but a call is bounded by the size of an MCP request, which is about 1 MiB of base64 by default, so an image of up to roughly 700 KiB fits; a larger image goes to POST /v1/assets as the raw body, with the same Bearer token). The result carries id, the SHA-256 of the bytes: the same bytes are the same asset, so uploading an image again is safe and costs nothing. Use the id in an updateNode or addNode command as image {assetId: id} (also fit: cover, contain or fill). An asset belongs to the workspace of your token: another workspace cannot read it. Assets count against the same storage limit as the file's changes: a new image that does not fit is refused with quota_storage.";

/// The assets module. It registers one Change-tier agent tool: an upload writes a file to the workspace's store.
#[derive(Debug, Clone, Copy)]
pub struct AssetsModule;

impl meldframe_module::Module for AssetsModule {
    fn name(&self) -> &'static str {
        "assets"
    }

    fn register(&self, registrar: &mut meldframe_module::Registrar) {
        registrar.agent_tool(
            UPLOAD_TOOL,
            UPLOAD_DESCRIPTION,
            meldframe_module::RiskTier::Change,
        );
    }
}
