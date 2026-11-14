//! Repository checks: layering rules, build matrix, portability and other gates.

mod matrix;
mod metadata;
mod plan;
mod portable;
mod rules;
mod scan;
mod size;
mod ui_literals;

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("deps-check") => with_packages(|p| {
            let violations = rules::check(p);
            for v in &violations {
                eprintln!("{v}");
            }
            if violations.is_empty() {
                println!("deps-check: {} crates, no violations", p.len());
                ExitCode::SUCCESS
            } else {
                eprintln!("deps-check: {} violation(s)", violations.len());
                ExitCode::FAILURE
            }
        }),
        Some("build-matrix") => {
            with_packages(|p| run_steps("build-matrix", &matrix::plan(p), None))
        }
        Some("portable-check") => with_packages(|p| {
            let hint = "if a target is missing: rustup target add wasm32-unknown-unknown \
                        (rust-toolchain.toml installs it on a fresh checkout)";
            run_steps("portable-check", &portable::plan(p), Some(hint))
        }),
        Some("size-check") => with_packages(size_check),
        Some("ui-literals") => with_packages(|p| {
            let found = ui_literals::collect(&scan::workspace_root(), p);
            found.iter().for_each(|f| eprintln!("{f}"));
            if found.is_empty() {
                println!("ui-literals: no literals in UI code");
                ExitCode::SUCCESS
            } else {
                eprintln!("ui-literals: {} literal(s)", found.len());
                ExitCode::FAILURE
            }
        }),
        other => {
            eprintln!(
                "usage: cargo xtask <deps-check|build-matrix|portable-check|size-check|ui-literals> (got {other:?})"
            );
            ExitCode::from(2)
        }
    }
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

/// Read `cargo metadata` and hand the workspace packages to `f`.
fn with_packages(f: impl FnOnce(&[rules::Package]) -> ExitCode) -> ExitCode {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = match Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            eprintln!(
                "cargo metadata failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            );
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("cannot run cargo metadata: {e}");
            return ExitCode::from(2);
        }
    };
    match metadata::parse(&String::from_utf8_lossy(&output.stdout)) {
        Ok(p) => f(&p),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

/// `size-check [--base <git-ref>]`: ceilings, and with `--base` that the allow-list did not grow.
fn size_check(packages: &[rules::Package]) -> ExitCode {
    let root = scan::workspace_root();
    let allow = match size::parse_allow(&scan::read(&root, size::ALLOW_FILE)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let mut violations = size::check(&size::measure(&root, packages), &allow);
    let args: Vec<String> = std::env::args().collect();
    if let Some(base) = args
        .iter()
        .position(|a| a == "--base")
        .and_then(|i| args.get(i + 1))
    {
        let spec = format!("{base}:{}", size::ALLOW_FILE);
        // A missing file on the base means the list is being introduced: start from empty.
        let text = Command::new("git")
            .args(["show", &spec])
            .current_dir(&root)
            .output();
        let text = text
            .ok()
            .filter(|o| o.status.success())
            .map(|o| o.stdout)
            .unwrap_or_default();
        match size::parse_allow(&String::from_utf8_lossy(&text)) {
            Ok(base_allow) => violations.extend(size::check_not_raised(&base_allow, &allow)),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        }
    }
    for v in &violations {
        eprintln!("{v}");
    }
    if violations.is_empty() {
        println!("size-check: {} crates within ceilings", packages.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("size-check: {} violation(s)", violations.len());
        ExitCode::FAILURE
    }
}
