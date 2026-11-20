//! Who may set a review state (ADR-0021, "Who may set it: deny by default"): the actors named in
//! `MELDFRAME_REVIEWERS`, by exact, case-sensitive match of the whole actor string. Nothing else is a reviewer, not
//! even a `person:` prefix: a prefix is a convention of a token's configuration, which the rule must not trust.
//!
//! The value is split on commas; each entry is trimmed of white space and an empty entry is ignored. An entry that is
//! not a valid actor (1 to 128 bytes, no control or hidden character) is an error, and so are more than
//! [`MAX_REVIEWERS`] entries. Unset or empty means nobody may review. This is the one place M2 changes when a
//! principal has a kind: the setter asks [`Reviewers::contains`] and nothing else.

use activity_store::check_actor;

/// The name of the environment variable.
pub const VAR: &str = "MELDFRAME_REVIEWERS";

/// The most entries a list may hold.
pub const MAX_REVIEWERS: usize = 32;

/// The listed reviewers; the default is nobody.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reviewers(Vec<String>);

impl Reviewers {
    /// Parse a comma-separated list. The error says what is wrong and never echoes an entry in full.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let entries: Vec<&str> = (spec.split(',').map(str::trim))
            .filter(|e| !e.is_empty())
            .collect();
        if entries.len() > MAX_REVIEWERS {
            return Err(format!("at most {MAX_REVIEWERS} reviewers may be listed"));
        }
        for (i, e) in entries.iter().enumerate() {
            check_actor(e).map_err(|why| format!("entry {} is not an actor: {why}", i + 1))?;
        }
        Ok(Reviewers(entries.into_iter().map(str::to_owned).collect()))
    }

    /// The list of [`VAR`]: unset means nobody; a value that is not valid text or does not parse is an error that
    /// names the variable.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var(VAR) {
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(std::env::VarError::NotUnicode(_)) => Err(format!("{VAR} is not valid text")),
            Ok(spec) => Self::parse(&spec).map_err(|e| format!("{VAR}: {e}")),
        }
    }

    /// Whether `actor` is listed: an exact match of the whole string.
    pub fn contains(&self, actor: &str) -> bool {
        self.0.iter().any(|r| r == actor)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
