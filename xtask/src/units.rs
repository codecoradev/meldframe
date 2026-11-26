//! Source that no Cargo package owns: `size-check` counts it too. Under `tools/` and `packages/`
//! every directory that is not inside a package is a unit of its own (`tools/fidelity-oracle`,
//! `packages/protocol`), and the files that sit directly in `tools/` are the unit `tools`.

use std::collections::BTreeMap;
use std::path::Path;

use crate::rules::Package;
use crate::scan;
use crate::size::{EXTENSIONS, Sized};

/// Directories searched for source that no package owns.
const ROOTS: &[&str] = &["tools", "packages"];

/// Group `files` (workspace-relative) that are not inside a package directory into units.
pub fn group(files: &[String], package_dirs: &[&str]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in files {
        let owned = package_dirs.iter().any(|d| {
            file.strip_prefix(d)
                .is_some_and(|rest| rest.starts_with('/'))
        });
        if owned {
            continue;
        }
        let parts: Vec<&str> = file.split('/').collect();
        let unit = if parts.len() > 2 {
            format!("{}/{}", parts[0], parts[1])
        } else {
            parts[0].to_string()
        };
        out.entry(unit).or_default().push(file.clone());
    }
    out
}

/// The units of source that no package owns, measured like a package.
pub fn unowned(root: &Path, packages: &[Package]) -> Vec<Sized> {
    let dirs: Vec<&str> = packages.iter().map(|p| p.dir.as_str()).collect();
    let files: Vec<String> = ROOTS
        .iter()
        .flat_map(|dir| scan::files(root, dir, EXTENSIONS))
        .collect();
    group(&files, &dirs)
        .into_iter()
        .map(|(unit, files)| {
            let counted = files
                .into_iter()
                .map(|path| {
                    let lines = scan::read(root, &path).lines().count();
                    (path, lines)
                })
                .collect();
            (unit, counted)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_directory_outside_every_package_is_a_unit() {
        let got = group(
            &files(&[
                "tools/oracle/a.mjs",
                "tools/oracle/test/b.mjs",
                "packages/protocol/src/c.ts",
            ]),
            &[],
        );
        assert_eq!(
            got.keys().map(String::as_str).collect::<Vec<_>>(),
            ["packages/protocol", "tools/oracle"]
        );
        assert_eq!(got["tools/oracle"].len(), 2);
    }

    #[test]
    fn files_directly_in_tools_are_one_unit_named_tools() {
        let got = group(&files(&["tools/smoke.mjs", "tools/other.js"]), &[]);
        assert_eq!(got.len(), 1);
        assert_eq!(got["tools"].len(), 2);
    }

    #[test]
    fn a_file_inside_a_package_is_left_to_that_package() {
        let got = group(
            &files(&[
                "packages/web-x/src/a.ts",
                "packages/web-xy/src/b.ts",
                "packages/p/c.ts",
            ]),
            &["packages/web-x"],
        );
        let units: Vec<&str> = got.keys().map(String::as_str).collect();
        assert_eq!(units, ["packages/p", "packages/web-xy"]);
    }

    #[test]
    fn the_measure_reads_the_files_and_skips_what_a_package_owns() {
        let dir = std::env::temp_dir().join(format!("xtask-units-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in [
            ("tools/t/a.mjs", "a\nb\n"),
            ("packages/p/x.ts", "x\n"),
            ("packages/owned/y.ts", "y\n"),
        ] {
            let full = dir.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, text).unwrap();
        }
        let owned = Package {
            name: "owned".into(),
            dir: "packages/owned".into(),
            ..Package::default()
        };
        let got = unowned(&dir, &[owned]);
        std::fs::remove_dir_all(&dir).unwrap();
        let sizes: Vec<(String, usize)> = got
            .into_iter()
            .map(|(unit, f)| (unit, f.iter().map(|x| x.1).sum()))
            .collect();
        assert_eq!(
            sizes,
            [("packages/p".to_string(), 1), ("tools/t".to_string(), 2)]
        );
    }
}
