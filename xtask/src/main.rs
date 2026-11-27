//! Repository checks: layering rules, build matrix, portability and other gates.

mod args;
mod gitsrc;
mod groups;
mod licence_scopes;
mod matrix;
mod metadata;
mod monolith;
mod plan;
mod portable;
mod rules;
mod scan;
mod size;
mod ui_literals;
mod units;
mod workspaces;

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("deps-check") => with_all_packages(|p| {
            report("deps-check", &format!("{} crates", p.len()), &deps_check(p))
        }),
        Some("build-matrix") => {
            with_packages(|p| run_steps("build-matrix", &matrix::plan(p), None))
        }
        Some("portable-check") => with_packages(|p| {
            let hint = "if a target is missing: rustup target add wasm32-unknown-unknown \
                        (rust-toolchain.toml installs it on a fresh checkout)";
            run_steps("portable-check", &portable::plan(p), Some(hint))
        }),
        Some("size-check") => with_all_packages(|p| finish(size_check(p))),
        Some("mm-check") => {
            with_all_packages(|p| finish(monolith::run(p, &std::env::args().collect::<Vec<_>>())))
        }
        Some("licences") => finish(licences_check()),
        Some("ui-literals") => with_all_packages(|p| {
            let found = ui_literals::collect(&scan::workspace_root(), p);
            report("ui-literals", "UI code", &found)
        }),
        other => {
            eprintln!(
                "usage: cargo xtask <deps-check|build-matrix|portable-check|size-check|mm-check|ui-literals|licences> (got {other:?})"
            );
            ExitCode::from(2)
        }
    }
}

/// Print every violation; success only when there are none.
fn report(name: &str, scope: &str, violations: &[impl std::fmt::Display]) -> ExitCode {
    violations.iter().for_each(|v| eprintln!("{v}"));
    if violations.is_empty() {
        println!("{name}: {scope}, no violations");
        ExitCode::SUCCESS
    } else {
        eprintln!("{name}: {} violation(s)", violations.len());
        ExitCode::FAILURE
    }
}

/// A command that could not even run (bad input) exits 2; a finished check keeps its code.
fn finish(result: Result<ExitCode, String>) -> ExitCode {
    result.unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::from(2)
    })
}

/// Run planned steps; fail when any step fails.
fn run_steps(name: &str, steps: &[plan::Step], hint: Option<&str>) -> ExitCode {
    let failed = plan::run(steps);
    if failed.is_empty() {
        println!("{name}: {} step(s) passed", steps.len());
        return ExitCode::SUCCESS;
    }
    for f in &failed {
        eprintln!("{name}: FAILED {f}");
    }
    if let Some(h) = hint {
        eprintln!("{name}: {h}");
    }
    ExitCode::FAILURE
}

/// Run `cargo metadata` with extra arguments and return its JSON, or an exit code on failure.
fn metadata_json(extra: &[&str]) -> Result<String, ExitCode> {
    run_metadata(None, extra)
}

/// Where the Rune workspace manifest is, when the checkout has the Rune workspace.
fn rune_manifest() -> Option<std::path::PathBuf> {
    let manifest = scan::workspace_root()
        .join(workspaces::RUNE_DIR)
        .join("Cargo.toml");
    manifest.is_file().then_some(manifest)
}

/// `cargo metadata` of the Rune workspace. It runs from the repository root, so the main toolchain
/// reads the manifests and the Rune toolchain (1.99.1) need not be installed; with `--no-deps` it
/// fetches nothing. `Ok(None)` when the checkout has no `frontends/rune`.
fn rune_metadata_json(extra: &[&str]) -> Result<Option<String>, ExitCode> {
    let Some(manifest) = rune_manifest() else {
        return Ok(None);
    };
    let manifest = manifest.to_string_lossy().into_owned();
    let args: Vec<&str> = ["--manifest-path", manifest.as_str()]
        .into_iter()
        .chain(extra.iter().copied())
        .collect();
    run_metadata(Some(&scan::workspace_root()), &args).map(Some)
}

fn run_metadata(dir: Option<&std::path::Path>, extra: &[&str]) -> Result<String, ExitCode> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.args(["metadata", "--format-version", "1"]).args(extra);
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    match cmd.output() {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(o) => {
            eprintln!(
                "cargo metadata failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            );
            Err(ExitCode::from(2))
        }
        Err(e) => {
            eprintln!("cannot run cargo metadata: {e}");
            Err(ExitCode::from(2))
        }
    }
}

/// Read `cargo metadata` and hand the packages of the main workspace to `f` (the steps that run
/// cargo for each package, `build-matrix` and `portable-check`, use these only: the Rune workspace
/// has its own toolchain and its own CI job).
fn with_packages(f: impl FnOnce(&[rules::Package]) -> ExitCode) -> ExitCode {
    let json = match metadata_json(&["--no-deps"]) {
        Ok(j) => j,
        Err(code) => return code,
    };
    match metadata::parse(&json) {
        Ok(p) => f(&p),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

/// Like `with_packages`, with the packages of the Rune workspace added (E8.0.4): each `dir` is
/// relative to the repository root, so every source check reads both workspaces.
fn with_all_packages(f: impl FnOnce(&[rules::Package]) -> ExitCode) -> ExitCode {
    let read = || -> Result<Vec<rules::Package>, ExitCode> {
        let main = metadata_json(&["--no-deps"])?;
        let root = metadata::workspace_root(&main).map_err(|e| fail(&e))?;
        let mut packages = metadata::parse(&main).map_err(|e| fail(&e))?;
        if let Some(rune) = rune_metadata_json(&["--no-deps"])? {
            packages.extend(metadata::parse_under(&rune, Some(&root)).map_err(|e| fail(&e))?);
        }
        Ok(packages)
    };
    match read() {
        Ok(p) => f(&p),
        Err(code) => code,
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(2)
}

/// The rules of `deps-check`: kinds and the matrix, the line between the two workspaces, and the
/// manifests that name a git source.
fn deps_check(packages: &[rules::Package]) -> Vec<String> {
    let manifests = gitsrc::collect(&scan::workspace_root());
    fn shown(v: &[impl std::fmt::Display]) -> Vec<String> {
        v.iter().map(ToString::to_string).collect()
    }
    [
        shown(&rules::check(packages)),
        shown(&workspaces::check(packages)),
        shown(&gitsrc::check(&manifests)),
    ]
    .concat()
}

/// `licences [--check] [--list] [--main-only]`: third-party licences of each workspace against the
/// allow list of its own `deny.toml` (D2; the Rune workspace has its own, E8.0.4). The Rune graph
/// is read with `--locked` and needs the registry and the Rune repository, so `--main-only` leaves
/// it out for a job that has no reason to fetch them. `--list` prints every crate with its licence.
fn licences_check() -> Result<ExitCode, String> {
    let args: Vec<String> = std::env::args().collect();
    args::check_flags("licences", &args, &["--check", "--list", "--main-only"])?;
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let root = scan::workspace_root();
    let main = metadata_json(&[]).map_err(|_| "cargo metadata failed".to_string())?;
    let main_deny = scan::read(&root, "deny.toml");
    let rune_deny = scan::read(&root, &format!("{}/deny.toml", workspaces::RUNE_DIR));
    let rune = if has("--main-only") {
        None
    } else {
        rune_metadata_json(&["--locked"]).map_err(|_| "cargo metadata failed".to_string())?
    };
    let mut scopes = vec![licence_scopes::Scope {
        name: "main",
        metadata: &main,
        deny_toml: &main_deny,
    }];
    if let Some(rune) = &rune {
        scopes.push(licence_scopes::Scope {
            name: workspaces::RUNE_DIR,
            metadata: rune,
            deny_toml: &rune_deny,
        });
    }
    let outcome = licence_scopes::check(&scopes)?;
    if has("--list") {
        outcome.listing.iter().for_each(|l| println!("{l}"));
    }
    let counts: Vec<String> = scopes
        .iter()
        .zip(&outcome.counts)
        .map(|(s, n)| format!("{n} ({})", s.name))
        .collect();
    let scope = format!("{} third-party crates", counts.join(" + "));
    Ok(report("licences", &scope, &outcome.violations))
}

/// `size-check [--base <git-ref>]`: ceilings, and with `--base` that the allow-list did not grow.
/// After the result line it notes the crates above the soft value (2,500); that never fails.
fn size_check(packages: &[rules::Package]) -> Result<ExitCode, String> {
    let root = scan::workspace_root();
    let allow = size::parse_allow(&scan::read(&root, size::ALLOW_FILE))?;
    let mut sizes = size::measure(&root, packages);
    // Source that no package owns (`tools/*`, a `packages/*` without a manifest) counts too.
    sizes.extend(units::unowned(&root, packages));
    let mut violations = size::check(&sizes, &allow);
    let args: Vec<String> = std::env::args().collect();
    args::check_flags("size-check", &args, &["--base"])?;
    if let Some(base) = size::base_arg(&args)? {
        // The ref must resolve: a bad ref is an error, never an empty base.
        let resolves = |spec: &str| {
            Command::new("git")
                .args(["rev-parse", "--verify", "--quiet", spec])
                .current_dir(&root)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        if !resolves(&format!("{base}^{{commit}}")) {
            return Err(format!(
                "size-check: --base `{base}` is not a git ref that resolves"
            ));
        }
        // A file missing on a resolved base means the list is being introduced: start from empty.
        let out = Command::new("git")
            .args(["show", &format!("{base}:{}", size::ALLOW_FILE)])
            .current_dir(&root)
            .output();
        let text = out
            .ok()
            .filter(|o| o.status.success())
            .map(|o| o.stdout)
            .unwrap_or_default();
        let base_allow = size::parse_allow(&String::from_utf8_lossy(&text))?;
        violations.extend(size::check_not_raised(&base_allow, &allow));
    }
    let code = report(
        "size-check",
        &format!("{} crates and units", sizes.len()),
        &violations,
    );
    if let Some(note) = size::split_note(&size::split_candidates(&sizes)) {
        println!("{note}");
    }
    Ok(code)
}

mod both_workspaces;
