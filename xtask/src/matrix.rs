//! `build-matrix` (M3): the product builds with no features and with each feature alone.

use crate::plan::Step;
use crate::rules::Package;

/// Steps for the matrix: one workspace build with default features off, then one build per
/// declared feature of each package, with default features off so the feature stands alone. A package
/// with default features also runs its tests with them off, so a test can prove that a feature left out
/// removes what it adds.
pub fn plan(packages: &[Package]) -> Vec<Step> {
    let mut steps = vec![Step::new(
        "no features",
        &["build", "--workspace", "--no-default-features"],
    )];
    let mut sorted: Vec<&Package> = packages.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for p in sorted {
        for f in p.features.iter().filter(|f| *f != "default") {
            steps.push(Step::new(
                format!("{} with feature {f}", p.name),
                &[
                    "build",
                    "-p",
                    &p.name,
                    "--no-default-features",
                    "--features",
                    f,
                ],
            ));
        }
        if p.features.iter().any(|f| f == "default") {
            steps.push(Step::new(
                format!("{} tests with default features off", p.name),
                &["test", "-p", &p.name, "--no-default-features"],
            ));
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, features: &[&str]) -> Package {
        Package {
            name: name.into(),
            features: features.iter().map(|s| s.to_string()).collect(),
            ..Package::default()
        }
    }

    #[test]
    fn no_features_still_builds_the_workspace_bare() {
        let steps = plan(&[pkg("a", &[])]);
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0].args,
            ["build", "--workspace", "--no-default-features"]
        );
    }

    #[test]
    fn each_feature_is_built_alone() {
        let steps = plan(&[pkg("a", &["x", "y"])]);
        assert_eq!(steps.len(), 3);
        assert_eq!(
            steps[1].args,
            [
                "build",
                "-p",
                "a",
                "--no-default-features",
                "--features",
                "x"
            ]
        );
        assert_eq!(steps[2].args.last().map(String::as_str), Some("y"));
    }

    #[test]
    fn a_package_with_default_features_is_tested_with_them_off() {
        let steps = plan(&[pkg("a", &["default", "x"])]);
        let last = steps.last().unwrap();
        assert_eq!(last.args, ["test", "-p", "a", "--no-default-features"]);
        assert!(last.label.contains("a"));
    }

    #[test]
    fn a_package_without_default_features_has_no_test_step() {
        let steps = plan(&[pkg("a", &["x"])]);
        assert!(steps.iter().all(|s| s.args[0] == "build"));
    }

    #[test]
    fn the_default_feature_is_not_a_member_of_the_matrix() {
        let steps = plan(&[pkg("a", &["default", "x"])]);
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn packages_are_ordered_by_name_for_stable_logs() {
        let steps = plan(&[pkg("b", &["f"]), pkg("a", &["f"])]);
        assert!(steps[1].label.contains("a"));
        assert!(steps[2].label.contains("b"));
    }
}
