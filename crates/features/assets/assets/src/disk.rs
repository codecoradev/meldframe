//! Assets on local disk: `<root>/<SHA-256 of the workspace>/<first two characters of the id>/<id>`.
//!
//! The workspace directory is the hex SHA-256 of its name, so every name (a slash, `..`, a NUL, 128 bytes that
//! would be too long in hex) gets a directory of its own, of a fixed length, inside the root; the id is checked to
//! be 64 lowercase hex characters before a path is built. A new file is written next to its final name, flushed
//! to disk and then renamed, so a reader never sees half of it. It is read back with its id checked, so a file
//! changed on disk or cut short is `asset_corrupt`, never served under its old name; storing the same bytes again
//! checks the file there and writes it anew when it does not match. A crash between the write and the rename
//! leaves a `.tmp` file that nothing removes yet.

use super::{
    AssetError, BlobStore, ImageType, Listed, MAX_LIST, Page, Stored, asset_id, check_id,
    check_upload, page_of, sniff,
};
use meldframe_protocol::{is_asset_id, is_lower_hex};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Assets in a directory of the local disk.
#[derive(Debug, Clone)]
pub struct DiskBlobs {
    root: PathBuf,
}

fn unavailable(e: std::io::Error) -> AssetError {
    AssetError::Unavailable(e.to_string())
}

impl DiskBlobs {
    /// A store in `root`; directories are made when the first asset of a workspace is written.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Where the asset `id` of `workspace` is (or would be) kept.
    pub fn path_of(&self, workspace: &str, id: &str) -> Result<PathBuf, AssetError> {
        check_id(id)?;
        Ok(self
            .root
            .join(asset_id(workspace.as_bytes()))
            .join(&id[..2])
            .join(id))
    }
}

impl BlobStore for DiskBlobs {
    fn put(&self, workspace: &str, bytes: &[u8]) -> Result<Stored, AssetError> {
        let (kind, id) = check_upload(bytes)?;
        let path = self.path_of(workspace, &id)?;
        let mut stored = Stored {
            id: id.clone(),
            kind,
            bytes: bytes.len(),
            created: false,
        };
        // The same bytes are there already, unless the file was changed or cut short: then write it anew. The
        // bytes in hand are compared with the file, so a repeat upload costs a read, not a second hash.
        let dir = path.parent().expect("the path has a directory");
        if std::fs::read(&path).is_ok_and(|on_disk| on_disk == bytes) {
            // The first upload may have answered before its name was flushed: flush it before this one answers.
            std::fs::File::open(dir)
                .and_then(|d| d.sync_all())
                .map_err(unavailable)?;
            return Ok(stored);
        }
        std::fs::create_dir_all(dir).map_err(unavailable)?;
        let tmp = dir.join(format!(
            ".{id}.{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let existed = path.exists();
        let written = std::fs::File::create(&tmp)
            .and_then(|mut f| f.write_all(bytes).and_then(|()| f.sync_all()))
            .and_then(|()| {
                if existed {
                    // A file that no longer matches is written anew, in place of the old one.
                    return std::fs::rename(&tmp, &path);
                }
                // A new name is made by a hard link, which fails when another upload of the same image got there
                // first: exactly one call creates the file, so the quota charges it once.
                match std::fs::hard_link(&tmp, &path) {
                    Ok(()) => {
                        stored.created = true;
                        // The name is made; a temporary file that stays is not a failure of the upload.
                        let _ = std::fs::remove_file(&tmp);
                        Ok(())
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        let _ = std::fs::remove_file(&tmp);
                        Ok(())
                    }
                    // A file system without hard links: rename, and count the file as created unless another
                    // call has made it in the meantime (a small window, on such a file system only).
                    Err(_) => {
                        let taken = path.exists();
                        std::fs::rename(&tmp, &path).inspect(|()| stored.created = !taken)
                    }
                }
            });
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(unavailable(e));
        }
        // Flush the directory too, so the new name survives a power loss once put has answered.
        std::fs::File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(unavailable)?;
        Ok(stored)
    }

    fn list(&self, workspace: &str, after: Option<&str>, limit: usize) -> Result<Page, AssetError> {
        if let Some(after) = after {
            check_id(after)?;
        }
        let limit = limit.clamp(1, MAX_LIST);
        let dir = self.root.join(asset_id(workspace.as_bytes()));
        let top = match std::fs::read_dir(&dir) {
            Ok(top) => top,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Page::default()),
            Err(e) => return Err(unavailable(e)),
        };
        // The two-character directories in order, from the one the cursor is in. At most 256 names.
        let mut shards = Vec::new();
        for sub in top {
            let sub = sub.map_err(unavailable)?;
            let name = sub.file_name().to_string_lossy().into_owned();
            let hex = name.len() == 2 && is_lower_hex(&name);
            if hex
                && sub.file_type().map_err(unavailable)?.is_dir()
                && after.is_none_or(|a| &a[..2] <= name.as_str())
            {
                shards.push(name);
            }
        }
        shards.sort();
        // One more than a page, to know whether a cursor is needed; only those files are opened.
        let mut items = Vec::new();
        'walk: for shard in shards {
            let mut names = Vec::new();
            for file in std::fs::read_dir(dir.join(&shard)).map_err(unavailable)? {
                let name = file
                    .map_err(unavailable)?
                    .file_name()
                    .to_string_lossy()
                    .into_owned();
                if is_asset_id(&name)
                    && name.starts_with(&shard)
                    && after.is_none_or(|a| name.as_str() > a)
                {
                    names.push(name);
                }
            }
            names.sort();
            for id in names {
                let path = dir.join(&shard).join(&id);
                let Ok(mut file) = std::fs::File::open(&path) else {
                    continue;
                };
                // The first bytes tell the type; the length is the size. The whole file is checked only by `get`.
                let mut head = Vec::with_capacity(12);
                let size = file.metadata().map_err(unavailable)?.len();
                Read::by_ref(&mut file)
                    .take(12)
                    .read_to_end(&mut head)
                    .map_err(unavailable)?;
                let Some(kind) = sniff(&head) else { continue };
                items.push(Listed {
                    id,
                    kind,
                    bytes: usize::try_from(size).unwrap_or(usize::MAX),
                });
                if items.len() > limit {
                    break 'walk;
                }
            }
        }
        Ok(page_of(items, limit))
    }

    fn used(&self, workspace: &str) -> Result<u64, AssetError> {
        let dir = self.root.join(asset_id(workspace.as_bytes()));
        let top = match std::fs::read_dir(&dir) {
            Ok(top) => top,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(unavailable(e)),
        };
        let mut total = 0u64;
        for sub in top {
            let sub = sub.map_err(unavailable)?;
            // Only the two-character directories hold assets; anything else in the workspace directory is not one.
            if !sub.file_type().map_err(unavailable)?.is_dir() {
                continue;
            }
            for file in std::fs::read_dir(sub.path()).map_err(unavailable)? {
                let file = file.map_err(unavailable)?;
                // A file a crash left half written (a name that starts with a dot) is not an asset.
                if file.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                total = total.saturating_add(file.metadata().map_err(unavailable)?.len());
            }
        }
        Ok(total)
    }

    fn get(&self, workspace: &str, id: &str) -> Result<(ImageType, Vec<u8>), AssetError> {
        let path = self.path_of(workspace, id)?;
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(AssetError::NotFound),
            Err(e) => return Err(unavailable(e)),
        };
        match sniff(&bytes) {
            Some(kind) if asset_id(&bytes) == id => Ok((kind, bytes)),
            _ => Err(AssetError::Corrupt),
        }
    }
}
