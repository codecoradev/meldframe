//! File-system side of the source checks: walk crate directories and read source files.

use std::fs;
use std::path::{Path, PathBuf};

/// The workspace root (the parent of this crate's directory).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// Every file under `dir` whose extension is in `exts`, as `/`-separated paths relative to
/// `root`, sorted. Skips `target`, `node_modules`, `dist` and hidden directories.
pub fn files(root: &Path, dir: &str, exts: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, &root.join(dir), exts, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, exts: &[&str], out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(name.as_str(), "target" | "node_modules" | "dist")
                && !name.starts_with('.')
            {
                walk(root, &path, exts, out);
            }
        } else if path
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| exts.contains(&x))
        {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Read a file relative to `root`; unreadable files read as empty (a skipped file is not a
/// finding for the checks that call this).
pub fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_default()
}
