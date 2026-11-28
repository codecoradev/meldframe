//! `toolkit-swap [--graph-only] [--wasm-tests]` (rule U10 of `docs/standards/modular-thin-ui.md`, E8.1.9): the toolkit of the
//! front end can be replaced without touching the app layer or the core.
//!
//! The graph part reads the workspace (text only, a second): no portable crate (core, engine, integration, os, feature, app,
//! ffi) reaches a front end crate through its dependencies or its dev-dependencies, so the app layer and the core build with no
//! front end present; every `tool` crate that tests the app layer (a `*-tests`, `*-props` or `*-scenarios` crate, the harness) is
//! held to the same rule with its dev-dependencies, and the set is derived, so a new test crate is covered at once; and the
//! headless harness (`meldframe-headless`, kind tool, a second front end written for this proof) depends on the app layer and
//! the core only. A front end crate is a `ui` or `feature-ui` crate, or a `bin` crate that reaches the app layer or a ui crate (the
//! server is a host, not a front end). The build part then does it: the app layer and the harness for `wasm32`, and the tests
//! of the app layer and of the test crates that need no host natively (`--wasm-tests` runs the tests under WASM too, which needs
//! `wasm-bindgen-test-runner`; `just wasm-test` does that in the nightly). `--graph-only` skips it, for the pull request run.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::process::ExitCode;

use crate::groups::kind_of;
use crate::plan::Step;
use crate::portable::{TARGETS, is_portable};
use crate::rules::{Kind, Package};

/// The app layer and the core crates it is made of: they must build and pass with no front end crate in the graph.
pub const INNER: &[&str] = &[
    "meldframe-app",
    "meldframe-app-model",
    "meldframe-app-rules",
    "meldframe-app-parts",
    "meldframe-app-pointer",
    "meldframe-camera",
    "meldframe-render",
    "meldframe-layout",
];
/// The app layer itself: a `bin` crate that reaches it is the shell of a front end.
const APP: &str = "meldframe-app";
/// The made-up second front end.
pub const HARNESS: &str = "meldframe-headless";

/// One broken rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// A crate the check stands on is not in the workspace (renamed or removed): the check would pass on nothing.
    Missing { krate: String },
    /// A crate that must build with no front end reaches one.
    FrontEnd {
        root: String,
        dep: String,
        kind: String,
        path: Vec<String>,
    },
    /// The harness is not a tool.
    HarnessKind { kind: String },
    /// The harness depends on something other than the app layer and the core.
    HarnessDep {
        dep: String,
        kind: String,
        path: Vec<String>,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::Missing { krate } => write!(
                f,
                "{krate}: not in the workspace; toolkit-swap checks it by name (update xtask/src/toolkit_swap.rs with the change)"
            ),
            Violation::FrontEnd {
                root,
                dep,
                kind,
                path,
            } => write!(
                f,
                "{root}: reaches {dep} ({kind}) through {}; the app layer and the core build with no front end crate (U10)",
                path.join(" -> ")
            ),
            Violation::HarnessKind { kind } => {
                write!(f, "{HARNESS}: kind is `{kind}`, it must be `tool`")
            }
            Violation::HarnessDep { dep, kind, path } => write!(
                f,
                "{HARNESS}: reaches {dep} ({kind}) through {}; the harness uses the app layer and the core only (U8)",
                path.join(" -> ")
            ),
        }
    }
}

/// Every workspace package `root` reaches by its dependencies (and, for the root alone, its dev-dependencies when
/// `with_dev`), with the shortest path to it. A package outside the workspace is not followed.
fn reach<'a>(
    root: &'a Package,
    index: &Index<'a>,
    with_dev: bool,
) -> Vec<(&'a Package, Vec<String>)> {
    let mut seen: HashSet<&str> = HashSet::from([root.name.as_str()]);
    let mut queue = VecDeque::from([(root, vec![root.name.clone()], with_dev)]);
    let mut out = Vec::new();
    while let Some((p, path, dev)) = queue.pop_front() {
        let dev_deps = if dev { &p.dev_dependencies[..] } else { &[] };
        for name in p.dependencies.iter().chain(dev_deps) {
            let Some(dep) = index.get(name.as_str()) else {
                continue;
            };
            if seen.insert(dep.name.as_str()) {
                let mut via = path.clone();
                via.push(dep.name.clone());
                out.push((*dep, via.clone()));
                queue.push_back((dep, via, false));
            }
        }
    }
    out
}

type Index<'a> = HashMap<&'a str, &'a Package>;

/// The front end crates: a toolkit swap replaces these and nothing else. A `ui` or `feature-ui` crate, or a `bin` crate that
/// reaches the app layer or a ui crate (the shell of a front end); the server is a `bin` that does neither.
fn front_ends<'a>(packages: &'a [Package], index: &Index<'a>) -> HashSet<&'a str> {
    let is_ui = |p: &Package| matches!(kind_of(p), Some(Kind::Ui | Kind::FeatureUi));
    packages
        .iter()
        .filter(|p| {
            is_ui(p)
                || (kind_of(p) == Some(Kind::Bin)
                    && reach(p, index, false)
                        .iter()
                        .any(|(d, _)| is_ui(d) || d.name == APP))
        })
        .map(|p| p.name.as_str())
        .collect()
}

/// The `tool` crates that test the app layer: they depend on it or on a crate it is made of, also as a dev-dependency.
fn derived_tools(packages: &[Package]) -> Vec<&Package> {
    let needs = |p: &Package| {
        let mut all = p.dependencies.iter().chain(&p.dev_dependencies);
        all.any(|d| INNER.contains(&d.as_str()))
    };
    packages
        .iter()
        .filter(|p| kind_of(p) == Some(Kind::Tool) && needs(p))
        .collect()
}

/// Every broken rule, sorted.
pub fn check(packages: &[Package]) -> Vec<Violation> {
    let index: Index = packages.iter().map(|p| (p.name.as_str(), p)).collect();
    let mut v = Vec::new();
    for name in INNER.iter().chain(&[HARNESS]) {
        if !index.contains_key(name) {
            v.push(Violation::Missing {
                krate: (*name).into(),
            });
        }
    }
    let front = front_ends(packages, &index);
    // Every portable crate and every test crate of the app layer, with its own dev-dependencies.
    let roots = packages
        .iter()
        .filter(|p| kind_of(p).is_some_and(is_portable))
        .chain(derived_tools(packages));
    for root in roots {
        for (dep, path) in reach(root, &index, true) {
            if front.contains(dep.name.as_str()) {
                v.push(Violation::FrontEnd {
                    root: root.name.clone(),
                    dep: dep.name.clone(),
                    kind: dep.kind.clone().unwrap_or_default(),
                    path,
                });
            }
        }
    }
    if let Some(h) = index.get(HARNESS) {
        if h.kind.as_deref() != Some("tool") {
            v.push(Violation::HarnessKind {
                kind: h.kind.clone().unwrap_or_default(),
            });
        }
        let direct = |name: &str| h.dependencies.iter().any(|d| d == name);
        for (dep, path) in reach(h, &index, false) {
            let kind = kind_of(dep);
            let allowed = match direct(&dep.name) {
                true => matches!(kind, Some(Kind::Core | Kind::App)),
                false => kind.is_some_and(is_portable),
            };
            if !allowed {
                v.push(Violation::HarnessDep {
                    dep: dep.name.clone(),
                    kind: dep.kind.clone().unwrap_or_default(),
                    path,
                });
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

/// The builds and tests that go with the graph: the app layer and the harness for each target, and their tests natively.
pub fn plan(packages: &[Package], wasm_tests: bool) -> Vec<Step> {
    let inner: Vec<&str> = INNER
        .iter()
        .copied()
        .filter(|n| packages.iter().any(|p| p.name == *n))
        .collect();
    let with = |first: &[&str], names: &[&str]| -> Vec<String> {
        let mut args: Vec<String> = first.iter().map(|s| s.to_string()).collect();
        for n in names {
            args.push("-p".into());
            args.push((*n).into());
        }
        args
    };
    let step = |label: &str, args: Vec<String>| Step {
        label: label.into(),
        args,
    };
    // The test crates that need no host (a bin crate such as the server) are built and run here; the others are the
    // workspace's `cargo test`.
    let index: Index = packages.iter().map(|p| (p.name.as_str(), p)).collect();
    let tools: Vec<&Package> = derived_tools(packages)
        .into_iter()
        .filter(|t| {
            !reach(t, &index, true)
                .iter()
                .any(|(d, _)| kind_of(d) == Some(Kind::Bin))
        })
        .collect();
    let names = |only_wasm: bool| -> Vec<&str> {
        let wasm = |t: &&Package| t.dev_dependencies.iter().any(|d| d == "wasm-bindgen-test");
        tools
            .iter()
            .filter(|t| !only_wasm || wasm(t))
            .map(|t| t.name.as_str())
            .collect()
    };
    let all: Vec<&str> = inner.iter().copied().chain(names(false)).collect();
    let mut steps = vec![step(
        "the app layer, its core and their test crates: tests, natively",
        with(&["test"], &all),
    )];
    let built: Vec<&str> = inner.iter().chain(&[HARNESS]).copied().collect();
    for target in TARGETS {
        steps.push(step(
            &format!("the app layer, its core and the harness: build for {target}"),
            with(&["build", "--target", target], &built),
        ));
    }
    if wasm_tests {
        for target in TARGETS {
            steps.push(step(
                &format!("the test crates of the app layer: tests for {target}"),
                with(&["test", "--target", target, "--tests"], &names(true)),
            ));
        }
    }
    steps
}

/// `toolkit-swap [--graph-only] [--wasm-tests]`.
pub fn run(packages: &[Package], args: &[String]) -> Result<ExitCode, String> {
    crate::args::check_flags("toolkit-swap", args, &["--graph-only", "--wasm-tests"])?;
    let flag = |f: &str| args.iter().any(|a| a == f);
    let graph = crate::report(
        "toolkit-swap",
        "no front end crate under the app layer and the core, the harness on them only",
        &check(packages),
    );
    if graph != ExitCode::SUCCESS || flag("--graph-only") {
        return Ok(graph);
    }
    let steps = plan(packages, flag("--wasm-tests"));
    Ok(crate::run_steps("toolkit-swap", &steps, None))
}

#[cfg(test)]
mod tests;
