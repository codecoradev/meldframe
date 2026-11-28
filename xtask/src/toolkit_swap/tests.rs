//! Tests of `toolkit-swap` on made-up workspaces.

use super::*;

fn pkg(name: &str, kind: &str, deps: &[&str]) -> Package {
    Package {
        name: name.into(),
        kind: Some(kind.into()),
        dependencies: deps.iter().map(|d| d.to_string()).collect(),
        ..Package::default()
    }
}

/// A workspace in the shape of the real one: the app layer on the core, test crates, a harness, a front end and a server.
fn clean() -> Vec<Package> {
    let mut v = vec![
        pkg("meldframe-protocol", "core", &[]),
        pkg("meldframe-camera", "core", &[]),
        pkg("meldframe-render", "core", &["meldframe-protocol"]),
        pkg("meldframe-layout", "core", &["meldframe-protocol"]),
        pkg("meldframe-app-model", "core", &["meldframe-camera"]),
        pkg("meldframe-app-rules", "core", &["meldframe-app-model"]),
        pkg("meldframe-app-parts", "core", &["meldframe-app-model"]),
        pkg("dialog", "feature", &["meldframe-protocol"]),
        pkg(
            "meldframe-app",
            "app",
            &[
                "meldframe-app-rules",
                "meldframe-app-parts",
                "meldframe-render",
                "meldframe-layout",
                "dialog",
            ],
        ),
        pkg("meldframe-ui", "ui", &["meldframe-app"]),
        pkg("web", "bin", &["meldframe-ui"]),
        pkg("server", "bin", &["meldframe-render"]),
        pkg(HARNESS, "tool", &["meldframe-app", "meldframe-protocol"]),
    ];
    // Test crates of the app layer, one of which runs under WASM, one that needs the host, and one that has nothing to do with it.
    for (name, wasm) in [
        ("meldframe-app-unit-tests", true),
        ("meldframe-app-layout-tests", false),
    ] {
        let mut p = pkg(name, "tool", &[]);
        p.dev_dependencies = vec!["meldframe-app".into()];
        if wasm {
            p.dev_dependencies.push("wasm-bindgen-test".into());
        }
        v.push(p);
    }
    let mut host = pkg("meldframe-app-host-tests", "tool", &["server"]);
    host.dev_dependencies = vec!["meldframe-app".into()];
    v.push(host);
    let mut other = pkg("unrelated-tool", "tool", &[]);
    other.dev_dependencies = vec!["web".into()];
    v.push(other);
    v
}

fn edit(v: &mut [Package], name: &str, f: impl FnOnce(&mut Package)) {
    f(v.iter_mut().find(|p| p.name == name).unwrap());
}

fn messages(v: &[Package]) -> Vec<String> {
    check(v).iter().map(|x| x.to_string()).collect()
}

#[test]
fn the_clean_workspace_passes_and_a_front_end_may_depend_on_the_app_layer() {
    assert_eq!(messages(&clean()), Vec::<String>::new());
}

#[test]
fn a_ui_crate_that_the_app_layer_depends_on_fails() {
    let mut v = clean();
    edit(&mut v, "meldframe-app", |p| {
        p.dependencies.push("meldframe-ui".into())
    });
    let m = messages(&v);
    assert!(
        m.iter().any(|x| x.contains(
            "meldframe-app: reaches meldframe-ui (ui) through meldframe-app -> meldframe-ui"
        )),
        "{m:?}"
    );
    // The harness depends on the app layer, so it reaches the ui crate too, and says so.
    assert!(
        m.iter()
            .any(|x| x.starts_with("meldframe-headless: reaches meldframe-ui")),
        "{m:?}"
    );
}

#[test]
fn a_front_end_reached_through_a_core_crate_names_the_path() {
    let mut v = clean();
    edit(&mut v, "meldframe-camera", |p| {
        p.dependencies.push("web".into())
    });
    let m = messages(&v);
    let hit = m
        .iter()
        .find(|x| x.starts_with("meldframe-app: reaches web"))
        .unwrap();
    assert!(hit.contains("web (bin) through meldframe-app -> meldframe-app-rules -> meldframe-app-model -> meldframe-camera -> web"), "{hit}");
}

#[test]
fn every_front_end_kind_counts_each_on_its_own() {
    // A bin crate is a front end when it reaches the app layer; a ui and a feature-ui crate are one by their kind.
    for (kind, deps) in [
        ("ui", &[][..]),
        ("feature-ui", &[]),
        ("bin", &["meldframe-app"]),
    ] {
        let mut v = clean();
        v.push(pkg("shell", kind, deps));
        edit(&mut v, "meldframe-render", |p| {
            p.dependencies.push("shell".into())
        });
        let m = messages(&v);
        let hit = m
            .iter()
            .find(|x| x.starts_with("meldframe-render: reaches shell"));
        assert!(
            hit.is_some_and(|x| x.contains(&format!("shell ({kind})")) && x.contains("(U10)")),
            "{kind}: {m:?}"
        );
    }
}

#[test]
fn the_server_is_a_host_and_not_a_front_end() {
    let mut v = clean();
    edit(&mut v, "meldframe-camera", |p| {
        p.dependencies.push("server".into())
    });
    assert!(!messages(&v).iter().any(|x| x.contains("(U10)")));
}

#[test]
fn a_tool_or_a_feature_crate_under_the_app_layer_is_not_a_front_end() {
    let mut v = clean();
    v.push(pkg("support", "tool", &[]));
    edit(&mut v, "meldframe-render", |p| {
        p.dependencies.push("support".into())
    });
    // The harness would reach it too, and says so; the app layer and the core have no front end in their graph.
    assert!(!messages(&v).iter().any(|x| x.contains("(U10)")));
}

#[test]
fn a_test_crate_with_a_front_end_as_a_dev_dependency_fails() {
    let mut v = clean();
    edit(&mut v, "meldframe-app-unit-tests", |p| {
        p.dev_dependencies.push("meldframe-ui".into())
    });
    let m = messages(&v);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].starts_with("meldframe-app-unit-tests: reaches meldframe-ui (ui)"),
        "{m:?}"
    );
}

#[test]
fn a_dev_dependency_on_a_front_end_fails_for_the_app_layer_the_core_and_the_harness() {
    for root in [
        "meldframe-app",
        "meldframe-render",
        "meldframe-app-parts",
        HARNESS,
    ] {
        let mut v = clean();
        edit(&mut v, root, |p| {
            p.dev_dependencies.push("meldframe-ui".into())
        });
        let m = messages(&v);
        assert!(
            m.iter().any(|x| x.starts_with(&format!(
                "{root}: reaches meldframe-ui (ui) through {root} -> meldframe-ui"
            ))),
            "{root}: {m:?}"
        );
    }
}

#[test]
fn a_test_crate_is_found_by_what_it_depends_on_and_not_by_a_list() {
    // A crate nobody listed, which tests a core crate of the app layer: its dev-dependencies count.
    let mut v = clean();
    let mut p = pkg("meldframe-new-props", "tool", &[]);
    p.dev_dependencies = vec!["meldframe-app-rules".into(), "meldframe-ui".into()];
    v.push(p);
    let m = messages(&v);
    assert!(
        m.iter()
            .any(|x| x.starts_with("meldframe-new-props: reaches meldframe-ui (ui)")),
        "{m:?}"
    );
    // A tool that has nothing to do with the app layer is left alone (`unrelated-tool` has a front end as a dev-dependency).
    assert!(
        !messages(&clean())
            .iter()
            .any(|x| x.contains("unrelated-tool"))
    );
}

#[test]
fn a_cycle_between_crates_ends() {
    let mut v = clean();
    edit(&mut v, "meldframe-camera", |p| {
        p.dependencies.push("meldframe-app-model".into())
    });
    assert_eq!(messages(&v), Vec::<String>::new());
}

#[test]
fn the_harness_may_use_the_app_layer_and_the_core_directly_and_nothing_else() {
    let mut v = clean();
    // A feature crate is below the app layer, but the harness has no business calling it itself.
    edit(&mut v, HARNESS, |p| p.dependencies.push("dialog".into()));
    let m = messages(&v);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].starts_with(
            "meldframe-headless: reaches dialog (feature) through meldframe-headless -> dialog"
        ),
        "{m:?}"
    );
    let mut v = clean();
    v.push(pkg("support", "tool", &[]));
    edit(&mut v, HARNESS, |p| p.dependencies.push("support".into()));
    assert!(
        messages(&v)
            .iter()
            .any(|x| x.contains("reaches support (tool)"))
    );
    let mut v = clean();
    edit(&mut v, HARNESS, |p| p.dependencies.push("server".into()));
    assert!(
        messages(&v)
            .iter()
            .any(|x| x.contains("reaches server (bin)"))
    );
}

#[test]
fn the_harness_is_a_tool() {
    let mut v = clean();
    edit(&mut v, HARNESS, |p| p.kind = Some("ui".into()));
    assert_eq!(
        messages(&v),
        ["meldframe-headless: kind is `ui`, it must be `tool`"]
    );
}

#[test]
fn a_crate_the_check_stands_on_that_is_gone_fails_instead_of_passing_on_nothing() {
    for gone in [
        HARNESS,
        "meldframe-app",
        "meldframe-render",
        "meldframe-app-parts",
    ] {
        let v: Vec<Package> = clean().into_iter().filter(|p| p.name != gone).collect();
        assert!(
            messages(&v)
                .iter()
                .any(|x| x.starts_with(&format!("{gone}: not in the workspace"))),
            "{gone}"
        );
    }
}

#[test]
fn the_plan_builds_the_app_layer_for_wasm_and_tests_it_natively_with_the_test_crates_that_need_no_host()
 {
    let steps = plan(&clean(), false);
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].args[0], "test");
    let has = |step: &Step, n: &str| step.args.windows(2).any(|w| w[0] == "-p" && w[1] == n);
    for n in INNER.iter().chain(&[
        HARNESS,
        "meldframe-app-unit-tests",
        "meldframe-app-layout-tests",
    ]) {
        assert!(has(&steps[0], n), "{n}");
    }
    // One that needs the server (a host) and one that is not a test of the app layer are the workspace's own tests.
    assert!(!has(&steps[0], "meldframe-app-host-tests"));
    assert!(!has(&steps[0], "unrelated-tool"));
    assert_eq!(
        &steps[1].args[..3],
        ["build", "--target", "wasm32-unknown-unknown"]
    );
    assert!(has(&steps[1], HARNESS));
    // No test crate in the wasm build: those are built by their tests.
    assert!(!has(&steps[1], "meldframe-app-unit-tests"));
}

#[test]
fn the_wasm_tests_are_a_step_of_their_own_and_only_for_crates_that_run_under_wasm() {
    let steps = plan(&clean(), true);
    assert_eq!(steps.len(), 3);
    assert_eq!(
        &steps[2].args[..4],
        ["test", "--target", "wasm32-unknown-unknown", "--tests"]
    );
    let has = |n: &str| steps[2].args.iter().any(|a| a == n);
    assert!(has("meldframe-app-unit-tests"));
    assert!(!has("meldframe-app-layout-tests"));
}

#[test]
fn a_crate_that_is_not_in_the_workspace_is_left_out_of_the_plan() {
    let v: Vec<Package> = clean()
        .into_iter()
        .filter(|p| p.name != "meldframe-app-parts")
        .collect();
    assert!(
        !plan(&v, false)[0]
            .args
            .iter()
            .any(|a| a == "meldframe-app-parts")
    );
}

#[test]
fn an_unknown_flag_is_an_error() {
    let args: Vec<String> = ["xtask", "toolkit-swap", "--graph"]
        .map(String::from)
        .to_vec();
    assert!(run(&clean(), &args).is_err());
}
