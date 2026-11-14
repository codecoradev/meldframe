//! File-system side of the source checks: walk crate directories and read source files.

use std::fs;
use std::path::{Path, PathBuf};

/// The workspace root (the parent of this crate's directory). Cargo sets `CARGO_MANIFEST_DIR` when it
/// runs `xtask`, so the tree under test is the one the command is run from. The value compiled in is
/// only a fallback: with several worktrees sharing one target directory a cached binary may have been
/// built in another tree.
pub fn workspace_root() -> PathBuf {
    let run = std::env::var("CARGO_MANIFEST_DIR").ok();
    root_of(&pick_manifest_dir(run, env!("CARGO_MANIFEST_DIR")))
}

/// The run-time value counts only if it names an existing directory called `xtask`: a stale value left
/// by another package (a `cargo test -p other` that starts xtask) must not move the root.
fn pick_manifest_dir(run_time: Option<String>, compiled_in: &str) -> String {
    run_time
        .filter(|d| Path::new(d).file_name().is_some_and(|n| n == "xtask") && Path::new(d).is_dir())
        .unwrap_or_else(|| compiled_in.to_string())
}

fn root_of(manifest_dir: &str) -> PathBuf {
    Path::new(manifest_dir)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_is_the_parent_of_the_xtask_directory() {
        assert_eq!(root_of("/a/b/xtask"), PathBuf::from("/a/b"));
    }

    #[test]
    fn the_directory_cargo_gives_at_run_time_wins_over_the_one_compiled_in() {
        // A cached binary built in one worktree must measure the worktree it is run from.
        let dir = std::env::temp_dir().join(format!("xtask-scan-{}", std::process::id()));
        let other = dir.join("xtask");
        fs::create_dir_all(&other).unwrap();
        let run = other.to_string_lossy().into_owned();
        assert_eq!(
            pick_manifest_dir(Some(run.clone()), "/built/here/xtask"),
            run
        );
        assert_eq!(
            pick_manifest_dir(None, "/built/here/xtask"),
            "/built/here/xtask"
        );
        assert_eq!(
            pick_manifest_dir(Some(String::new()), "/built/here/xtask"),
            "/built/here/xtask"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_directory_of_another_package_is_not_taken_for_the_xtask_directory() {
        // `cargo test -p other` sets CARGO_MANIFEST_DIR to that package, and may start xtask.
        let foreign = Some("/x/crates/core/meldframe-check".to_string());
        assert_eq!(
            pick_manifest_dir(foreign, "/built/here/xtask"),
            "/built/here/xtask"
        );
        let nowhere = Some("/does/not/exist".to_string());
        assert_eq!(
            pick_manifest_dir(nowhere, "/built/here/xtask"),
            "/built/here/xtask"
        );
    }
}
