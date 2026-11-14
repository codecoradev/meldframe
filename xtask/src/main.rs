//! Repository checks: layering rules, build matrix, portability and other gates.

mod matrix;
mod metadata;
mod plan;
mod portable;
mod rules;

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
        other => {
            eprintln!(
                "usage: cargo xtask <deps-check|build-matrix|portable-check> (got {other:?})"
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
