//! Turns `cargo metadata --no-deps` output into rule input.

use serde_json::Value;

use crate::rules::{Package, is_ui_toolkit};

/// Parse the JSON printed by `cargo metadata --format-version 1 --no-deps`.
pub fn parse(json: &str) -> Result<Vec<Package>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("invalid metadata: {e}"))?;
    let packages = root["packages"]
        .as_array()
        .ok_or("metadata has no `packages` array")?;
    Ok(packages.iter().map(package).collect())
}

fn package(p: &Value) -> Package {
    let meta = &p["metadata"]["meldframe"];
    let mut dependencies = Vec::new();
    let mut uses_ui_toolkit = false;
    for d in p["dependencies"].as_array().into_iter().flatten() {
        // Dev-dependencies do not ship, so they do not count for layering.
        if d["kind"].as_str() == Some("dev") {
            continue;
        }
        let name = d["name"].as_str().unwrap_or_default();
        uses_ui_toolkit |= is_ui_toolkit(name);
        dependencies.push(name.to_string());
    }
    Package {
        name: p["name"].as_str().unwrap_or_default().to_string(),
        kind: string_field(&meta["kind"]),
        group: string_field(&meta["group"]),
        composite: meta["composite"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        dependencies,
        uses_ui_toolkit,
    }
}

/// A present value of the wrong type is kept as text so it fails as an unknown kind.
fn string_field(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"packages":[
      {"name":"a","metadata":{"meldframe":{"kind":"feature","group":"g","composite":["h"]}},
       "dependencies":[{"name":"b","kind":null},{"name":"egui","kind":null},{"name":"serde","kind":"dev"}]},
      {"name":"b","metadata":null,"dependencies":[]},
      {"name":"c","metadata":{"meldframe":{"kind":7}},"dependencies":[]}
    ]}"#;

    #[test]
    fn reads_kind_group_composite_and_dependencies() {
        let pkgs = parse(SAMPLE).unwrap();
        assert_eq!(pkgs.len(), 3);
        let a = &pkgs[0];
        assert_eq!(a.kind.as_deref(), Some("feature"));
        assert_eq!(a.group.as_deref(), Some("g"));
        assert_eq!(a.composite, vec!["h".to_string()]);
        assert_eq!(a.dependencies, vec!["b".to_string(), "egui".to_string()]);
        assert!(a.uses_ui_toolkit);
    }

    #[test]
    fn missing_metadata_is_a_missing_kind() {
        assert_eq!(parse(SAMPLE).unwrap()[1].kind, None);
    }

    #[test]
    fn non_string_kind_is_kept_as_text() {
        assert_eq!(parse(SAMPLE).unwrap()[2].kind.as_deref(), Some("7"));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("not json").is_err());
        assert!(parse("{}").is_err());
    }
}
