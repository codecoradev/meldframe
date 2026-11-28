//! Reading the packages of the workspaces for the commands (E8.0.4). The main workspace is at the
//! repository root and the Rune workspace under `frontends/rune/`, with a toolchain of its own. A
//! command that reads source (`deps-check`, `size-check`, `mm-check`, `ui-literals`) gets the
//! packages of both, every `dir` relative to the root; a command that runs cargo for each package
//! (`build-matrix`, `portable-check`) gets the main workspace only.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::metadata;
use crate::rules::Package;
use crate::workspaces::RUNE_DIR;

/// Which workspaces a command reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Main,
    Both,
}

/// The workspaces a command reads.
pub fn scope(command: &str) -> Scope {
    match command {
        "deps-check" | "size-check" | "mm-check" | "ui-literals" => Scope::Both,
        _ => Scope::Main,
    }
}

/// The manifest of the Rune workspace, when the checkout has one.
pub fn rune_manifest(root: &Path) -> Option<PathBuf> {
    let manifest = root.join(RUNE_DIR).join("Cargo.toml");
    manifest.is_file().then_some(manifest)
}

/// `cargo metadata` of the workspace at `manifest`, run from `root`: the main toolchain reads the
/// manifests, so the Rune toolchain (1.99.1) need not be installed, and with `--no-deps` nothing is
/// fetched.
pub fn cargo_metadata(root: &Path, manifest: &Path, extra: &[&str]) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(manifest)
        .args(extra)
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

/// The packages of the workspaces in `scope`, under the workspace root `root`.
pub fn read(root: &Path, scope: Scope) -> Result<Vec<Package>, String> {
    let main = cargo_metadata(root, &root.join("Cargo.toml"), &["--no-deps"])?;
    let mut packages = metadata::parse(&main)?;
    if scope == Scope::Both
        && let Some(manifest) = rune_manifest(root)
    {
        let rune = cargo_metadata(root, &manifest, &["--no-deps"])?;
        let under = metadata::workspace_root(&main)?;
        packages.extend(metadata::parse_under(&rune, Some(&under))?);
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commands_that_read_source_read_both_workspaces_and_the_others_one() {
        for command in ["deps-check", "size-check", "mm-check", "ui-literals"] {
            assert_eq!(scope(command), Scope::Both, "{command}");
        }
        for command in ["build-matrix", "portable-check", "licences", "other"] {
            assert_eq!(scope(command), Scope::Main, "{command}");
        }
    }
}
