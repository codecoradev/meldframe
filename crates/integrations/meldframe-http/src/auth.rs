//! Who is calling: the [`Authenticator`] seam, and the development-token adapter behind it.

use meldframe_host::Principal;
use std::collections::HashMap;
use std::fmt;

/// Turns a bearer token into a principal. Gatehouse implements it in M2 (ADR-0006); until then
/// [`DevTokens`] does. The workspace of a call comes only from here, never from the request.
pub trait Authenticator: Send + Sync {
    /// The principal behind `token`, or `None` for an unknown or revoked one.
    fn authenticate(&self, token: &str) -> Option<Principal>;
}

/// A fixed table of development tokens. Never for production: see [`check_dev_tokens_allowed`].
#[derive(Debug, Clone)]
pub struct DevTokens(HashMap<String, Principal>);

/// A token spec that does not parse. The message never repeats the spec, because it holds secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError(&'static str);

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid development token list: {}", self.0)
    }
}

impl std::error::Error for SpecError {}

impl DevTokens {
    /// Parse `token:workspace:actor` entries separated by commas; the actor may itself hold `:`
    /// (`agent:alpha`). The caller reads the spec from configuration or the environment, never from
    /// argv, where any local user can read it.
    pub fn parse(spec: &str) -> Result<Self, SpecError> {
        let mut table = HashMap::new();
        for entry in spec.split(',').map(str::trim) {
            let mut parts = entry.splitn(3, ':');
            let (Some(token), Some(workspace), Some(actor)) =
                (parts.next(), parts.next(), parts.next())
            else {
                return Err(SpecError("an entry is not token:workspace:actor"));
            };
            if token.is_empty() || workspace.is_empty() || actor.is_empty() {
                return Err(SpecError("an entry has an empty part"));
            }
            if table
                .insert(token.to_owned(), Principal::new(workspace, actor))
                .is_some()
            {
                return Err(SpecError("a token is listed twice"));
            }
        }
        Ok(DevTokens(table))
    }
}

impl Authenticator for DevTokens {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        self.0.get(token).cloned()
    }
}

/// Why a release build will not start with development tokens on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevTokensRefused;

impl fmt::Display for DevTokensRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "development tokens are off in release builds; set MELDFRAME_ALLOW_DEV_TOKENS=1 to allow them"
        )
    }
}

impl std::error::Error for DevTokensRefused {}

/// The start-up guard (E2.1): a release build refuses development tokens unless `allow` is `1`.
pub fn check_dev_tokens_allowed(
    release: bool,
    allow: Option<&str>,
) -> Result<(), DevTokensRefused> {
    if !release || allow == Some("1") {
        Ok(())
    } else {
        Err(DevTokensRefused)
    }
}
