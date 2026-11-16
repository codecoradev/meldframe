//! Who is calling: the [`Authenticator`] seam, and the development-token adapter behind it.

use meldframe_host::Principal;
use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;

/// Turns a bearer token into a principal. Gatehouse implements it in M2 (ADR-0006); until then
/// [`DevTokens`] does. The workspace of a call comes only from here, never from the request.
pub trait Authenticator: Send + Sync {
    /// The principal behind `token`, or `None` for an unknown or revoked one.
    fn authenticate(&self, token: &str) -> Option<Principal>;
}

/// The shortest development token accepted, in bytes: a guessable token is no token.
pub const MIN_TOKEN_LEN: usize = 16;

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
            if token.len() < MIN_TOKEN_LEN {
                return Err(SpecError("a token is shorter than 16 bytes"));
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

/// Why start-up is refused over development tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevTokensRefused(&'static str);

impl fmt::Display for DevTokensRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
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
        Err(DevTokensRefused(
            "development tokens are off in release builds; set MELDFRAME_ALLOW_DEV_TOKENS=1 to allow them",
        ))
    }
}

/// Plain HTTP carries the bearer token, so development tokens listen on loopback only, unless `allow_public`
/// (`MELDFRAME_ALLOW_DEV_PUBLIC`) is `1`. This is separate from the release override.
pub fn check_dev_listen(
    addr: SocketAddr,
    allow_public: Option<&str>,
) -> Result<(), DevTokensRefused> {
    if addr.ip().is_loopback() || allow_public == Some("1") {
        Ok(())
    } else {
        Err(DevTokensRefused(
            "development tokens send secrets over plain HTTP: listen on a loopback address, or set MELDFRAME_ALLOW_DEV_PUBLIC=1",
        ))
    }
}

/// The line to print when the override starts a release build with development tokens.
pub fn dev_tokens_warning(release: bool, allow: Option<&str>) -> Option<&'static str> {
    (release && allow == Some("1")).then_some(
        "WARNING: development tokens are enabled in a release build (MELDFRAME_ALLOW_DEV_TOKENS=1); do not use this in production",
    )
}
