//! `licences --check` (D2): every third-party crate's licence expression must be allowed by the
//! `[licenses] allow` list in `deny.toml`, the same list `cargo deny check` reads.

use std::fmt;

use serde_json::Value;

/// A dependency reduced to what the licence check needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub version: String,
    pub licence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// (name, version)
    Unlicensed(String, String),
    /// (name, version, licence expression)
    NotAllowed(String, String, String),
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlicensed(n, v) => write!(f, "{n} {v}: no licence declared"),
            Self::NotAllowed(n, v, l) => {
                write!(f, "{n} {v}: licence `{l}` is not in the allow-list")
            }
        }
    }
}

/// The `[licenses] allow` list of a `deny.toml`.
pub fn parse_allow(deny_toml: &str) -> Result<Vec<String>, String> {
    let root: toml::Table = deny_toml.parse().map_err(|e| format!("deny.toml: {e}"))?;
    let list = root
        .get("licenses")
        .and_then(|l| l.get("allow"))
        .and_then(toml::Value::as_array)
        .ok_or("deny.toml: missing [licenses] allow list")?;
    list.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or("deny.toml: allow entries must be strings".to_string())
        })
        .collect()
}

/// Third-party packages (those with a source) from full `cargo metadata` output.
pub fn parse_deps(json: &str) -> Result<Vec<Dep>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("invalid metadata: {e}"))?;
    let packages = root["packages"]
        .as_array()
        .ok_or("metadata has no `packages` array")?;
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    Ok(packages
        .iter()
        .filter(|p| !p["source"].is_null())
        .map(|p| Dep {
            name: text(&p["name"]),
            version: text(&p["version"]),
            licence: p["license"].as_str().map(str::to_string),
        })
        .collect())
}

/// Evaluates an SPDX expression: `OR` of `AND` of terms, parentheses, `X WITH Y`, and the
/// legacy `/` as `OR`.
struct Eval<'a> {
    tokens: Vec<&'a str>,
    pos: usize,
    allowed: &'a [String],
}

impl<'a> Eval<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.tokens.get(self.pos).copied()
    }

    /// Every operand is parsed (no short-circuit) so a malformed tail is still an error.
    fn or(&mut self) -> Option<bool> {
        let mut ok = self.and()?;
        while self.peek() == Some("OR") {
            self.pos += 1;
            ok |= self.and()?;
        }
        Some(ok)
    }

    fn and(&mut self) -> Option<bool> {
        let mut ok = self.term()?;
        while self.peek() == Some("AND") {
            self.pos += 1;
            ok &= self.term()?;
        }
        Some(ok)
    }

    fn term(&mut self) -> Option<bool> {
        let t = self.peek()?;
        self.pos += 1;
        match t {
            "(" => {
                let ok = self.or()?;
                (self.peek() == Some(")")).then(|| self.pos += 1)?;
                Some(ok)
            }
            ")" | "OR" | "AND" | "WITH" => None,
            id => {
                let mut full = id.to_string();
                if self.peek() == Some("WITH") {
                    self.pos += 1;
                    let e = self.peek()?;
                    self.pos += 1;
                    full = format!("{id} WITH {e}");
                }
                Some(self.allowed.iter().any(|a| *a == full || a == id))
            }
        }
    }
}

/// True when the SPDX expression can be satisfied with allowed licences only.
pub fn permitted(expr: &str, allowed: &[String]) -> bool {
    let spaced = expr
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('/', " OR ");
    let mut eval = Eval {
        tokens: spaced.split_whitespace().collect(),
        pos: 0,
        allowed,
    };
    eval.or()
        .is_some_and(|ok| ok && eval.pos == eval.tokens.len())
}

/// Check every dependency; the result is sorted by crate name.
pub fn check(deps: &[Dep], allowed: &[String]) -> Vec<Violation> {
    let mut out: Vec<Violation> = deps
        .iter()
        .filter_map(|d| {
            let (n, v) = (d.name.clone(), d.version.clone());
            match &d.licence {
                None => Some(Violation::Unlicensed(n, v)),
                Some(l) if !permitted(l, allowed) => Some(Violation::NotAllowed(n, v, l.clone())),
                Some(_) => None,
            }
        })
        .collect();
    out.sort_by_key(|v| match v {
        Violation::Unlicensed(n, _) | Violation::NotAllowed(n, ..) => n.clone(),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        [
            "MIT",
            "Apache-2.0",
            "Unicode-3.0",
            "Apache-2.0 WITH LLVM-exception",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn dep(name: &str, licence: Option<&str>) -> Dep {
        Dep {
            name: name.into(),
            version: "1.0.0".into(),
            licence: licence.map(Into::into),
        }
    }

    #[test]
    fn single_and_or_expressions() {
        let a = allowed();
        assert!(permitted("MIT", &a));
        assert!(!permitted("GPL-3.0-only", &a));
        assert!(permitted("MIT OR GPL-3.0-only", &a));
        assert!(permitted("GPL-3.0-only OR Apache-2.0", &a));
        assert!(!permitted("GPL-3.0-only OR AGPL-3.0-only", &a));
        assert!(permitted("MIT AND Unicode-3.0", &a));
        assert!(!permitted("MIT AND GPL-3.0-only", &a));
    }

    #[test]
    fn parentheses_legacy_slash_and_exceptions() {
        let a = allowed();
        assert!(permitted("(MIT OR GPL-3.0-only) AND Apache-2.0", &a));
        assert!(!permitted("(GPL-3.0-only OR AGPL-3.0-only) AND MIT", &a));
        assert!(permitted("MIT/Apache-2.0", &a));
        assert!(permitted("Apache-2.0 WITH LLVM-exception", &a));
        assert!(permitted("Apache-2.0 WITH Other-exception", &a));
        assert!(!permitted("GPL-2.0-only WITH Classpath-exception-2.0", &a));
    }

    #[test]
    fn malformed_expressions_are_not_permitted() {
        let a = allowed();
        for bad in ["", "MIT OR", "(MIT", "MIT AND )", "OR MIT"] {
            assert!(!permitted(bad, &a), "{bad:?}");
        }
    }

    #[test]
    fn check_reports_missing_and_disallowed_licences_sorted() {
        let deps = [
            dep("ok", Some("MIT OR Apache-2.0")),
            dep("zed", None),
            dep("bad", Some("GPL-3.0-only")),
        ];
        let (v1, bad, zed) = ("1.0.0".to_string(), "bad".to_string(), "zed".to_string());
        assert_eq!(
            check(&deps, &allowed()),
            vec![
                Violation::NotAllowed(bad, v1.clone(), "GPL-3.0-only".into()),
                Violation::Unlicensed(zed, v1)
            ]
        );
    }

    #[test]
    fn deny_toml_allow_list_is_read() {
        let t = "[licenses]\nallow = [\"MIT\", \"Zlib\"]\nconfidence-threshold = 0.9\n";
        assert_eq!(parse_allow(t).unwrap(), ["MIT", "Zlib"]);
        assert!(parse_allow("").is_err());
        assert!(parse_allow("[licenses]\nallow = [1]\n").is_err());
    }

    #[test]
    fn only_packages_with_a_source_are_third_party() {
        let json = r#"{"packages":[
          {"name":"mine","version":"0.0.0","license":null,"source":null},
          {"name":"serde","version":"1.0.1","license":"MIT OR Apache-2.0","source":"registry+x"},
          {"name":"odd","version":"2.0.0","license":null,"source":"registry+x"}]}"#;
        let serde = Dep {
            version: "1.0.1".into(),
            ..dep("serde", Some("MIT OR Apache-2.0"))
        };
        let odd = Dep {
            version: "2.0.0".into(),
            ..dep("odd", None)
        };
        assert_eq!(parse_deps(json).unwrap(), vec![serde, odd]);
        assert!(parse_deps("nope").is_err());
    }
}
