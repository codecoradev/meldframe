//! Assets on local disk: `<root>/<SHA-256 of the workspace>/<first two characters of the id>/<id>`.
//!
//! The workspace directory is the hex SHA-256 of its name, so every name (a slash, `..`, a NUL, 128 bytes that
//! would be too long in hex) gets a directory of its own, of a fixed length, inside the root; the id is checked to
//! be 64 lowercase hex characters before a path is built. A new file is written next to its final name, flushed
//! to disk and then renamed, so a reader never sees half of it. It is read back with its id checked, so a file
//! changed on disk or cut short is `asset_corrupt`, never served under its old name; storing the same bytes again
//! checks the file there and writes it anew when it does not match. A crash between the write and the rename
//! leaves a `.tmp` file that nothing removes yet.

use super::{AssetError, BlobStore, ImageType, Stored, asset_id, check_id, check_upload, sniff};
use std::io::Write;
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
    fn put(&mut self, workspace: &str, bytes: &[u8]) -> Result<Stored, AssetError> {
        let (kind, id) = check_upload(bytes)?;
        let path = self.path_of(workspace, &id)?;
        let stored = Stored {
            id: id.clone(),
            kind,
            bytes: bytes.len(),
        };
        // The same bytes are there already, unless the file was changed or cut short: then write it anew. The
        // bytes in hand are compared with the file, so a repeat upload costs a read, not a second hash.
        if std::fs::read(&path).is_ok_and(|on_disk| on_disk == bytes) {
            return Ok(stored);
        }
        let dir = path.parent().expect("the path has a directory");
        std::fs::create_dir_all(dir).map_err(unavailable)?;
        let tmp = dir.join(format!(
            ".{id}.{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let written = std::fs::File::create(&tmp)
            .and_then(|mut f| f.write_all(bytes).and_then(|()| f.sync_all()))
            .and_then(|()| std::fs::rename(&tmp, &path));
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
