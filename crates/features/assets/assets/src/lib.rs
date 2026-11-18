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

/// Whether `id` has the form of an asset id (64 lowercase hex characters), so a path built from it is safe.
pub fn is_asset_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
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
    /// Whether `workspace` has the asset.
    fn has(&self, workspace: &str, id: &str) -> Result<bool, AssetError> {
        match self.get(workspace, id) {
            Ok(_) => Ok(true),
            Err(AssetError::NotFound) => Ok(false),
            Err(e) => Err(e),
        }
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
