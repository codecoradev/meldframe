//! What the shell needs to reach a server: where it is, which file to open, who the edits are made as, and the token.
//!
//! The token comes from the environment (`MELDFRAME_TOKEN`) or from the git-ignored `.dev-token` that `just dev` writes,
//! never from an argument, and it is kept out of every `Debug` output. This slice speaks plain HTTP and WebSocket and has no
//! TLS, so the server must be on a loopback address: a token is never sent over a network in clear text.

use std::fmt;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

/// Where a development server listens when nothing says otherwise (`just dev`).
pub const DEFAULT_SERVER: &str = "http://127.0.0.1:8080";
/// The file a development server is seeded with.
pub const DEFAULT_FILE: &str = "file_dev";
/// The actor of the development token (`MELDFRAME_DEV_TOKENS` of `just dev`): the app must know it to tell its own events.
pub const DEFAULT_ACTOR: &str = "agent:dev";
/// The name of the file `just dev` keeps the development token in.
pub const TOKEN_FILE: &str = ".dev-token";

/// Why a configuration could not be made. No variant holds the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// No token in `MELDFRAME_TOKEN` and no `.dev-token` in the working directory or above it.
    NoToken,
    /// A token that is empty or holds a character a header cannot carry.
    BadToken,
    /// An address that is not `http://host[:port]`.
    BadServer(String),
    /// `https://`: this build has no TLS.
    NoTls,
    /// A host that is not loopback: the token would cross a network in clear text.
    NotLoopback(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NoToken => write!(
                f,
                "no token: set MELDFRAME_TOKEN or run `just dev` once to make .dev-token"
            ),
            ConfigError::BadToken => write!(
                f,
                "the token is empty or holds a character that is not printable ASCII"
            ),
            ConfigError::BadServer(s) => {
                write!(f, "{s:?} is not an address like http://127.0.0.1:8080")
            }
            ConfigError::NoTls => write!(
                f,
                "this build has no TLS: use an http:// address on this machine"
            ),
            ConfigError::NotLoopback(h) => {
                write!(
                    f,
                    "{h} is not a loopback address: the token would cross the network unencrypted"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// A bearer token. Its `Debug` output is fixed text, and only this crate can read it back.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// A token as typed or read from a file: surrounding whitespace is dropped, the rest must be printable ASCII.
    pub fn new(raw: &str) -> Result<Token, ConfigError> {
        let t = raw.trim();
        if t.is_empty() || !t.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(ConfigError::BadToken);
        }
        Ok(Token(t.to_string()))
    }

    /// The value of the `Authorization` header.
    pub(crate) fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

/// A server on this machine, reached over plain HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    host: String,
    port: u16,
}

impl Server {
    /// `http://host[:port][/]` with a loopback host (`localhost`, `127.x.y.z` or `[::1]`).
    pub fn parse(url: &str) -> Result<Server, ConfigError> {
        let bad = || ConfigError::BadServer(url.to_string());
        if url.starts_with("https://") || url.starts_with("wss://") {
            return Err(ConfigError::NoTls);
        }
        let rest = url.strip_prefix("http://").ok_or_else(bad)?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        let (host, port) = match rest.strip_prefix('[') {
            Some(v6) => {
                let (host, tail) = v6.split_once(']').ok_or_else(bad)?;
                let port = match tail.strip_prefix(':') {
                    Some(p) => p.parse().map_err(|_| bad())?,
                    None if tail.is_empty() => 80,
                    None => return Err(bad()),
                };
                (host, port)
            }
            None => match rest.split_once(':') {
                Some((host, p)) => (host, p.parse().map_err(|_| bad())?),
                None => (rest, 80),
            },
        };
        let loopback =
            host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if host.is_empty() || host.contains(['/', '@', '?', '#']) {
            return Err(bad());
        }
        if !loopback {
            return Err(ConfigError::NotLoopback(host.to_string()));
        }
        Ok(Server {
            host: host.to_string(),
            port,
        })
    }

    /// `host:port`, with brackets around an IPv6 address.
    pub(crate) fn authority(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    pub(crate) fn http(&self, path: &str) -> String {
        format!("http://{}{path}", self.authority())
    }

    pub(crate) fn ws(&self, path: &str) -> String {
        format!("ws://{}{path}", self.authority())
    }

    /// Every address the host resolves to that is on this machine, in the order the resolver gave them. A name such as
    /// `localhost` may resolve to `::1` and to `127.0.0.1` while the server listens on one of them, so a caller tries each;
    /// an address that is not loopback (a name that was made to point elsewhere) is never tried, so the token stays here.
    pub(crate) fn addresses(&self) -> Vec<SocketAddr> {
        let resolved = (self.host.as_str(), self.port).to_socket_addrs();
        loopback_only(resolved.map(Iterator::collect).unwrap_or_default())
    }
}

/// Everything the shell holds about the server it talks to.
#[derive(Debug, Clone)]
pub struct Config {
    pub server: Server,
    /// The file to open.
    pub file: String,
    /// The actor the server stamps on this client's ops (the token's): the app compares it with the actor of a frame.
    pub actor: String,
    pub token: Token,
}

impl Config {
    pub fn new(server: &str, file: &str, actor: &str, token: &str) -> Result<Config, ConfigError> {
        Ok(Config {
            server: Server::parse(server)?,
            file: file.to_string(),
            actor: actor.to_string(),
            token: Token::new(token)?,
        })
    }

    /// From `MELDFRAME_SERVER`, `MELDFRAME_FILE`, `MELDFRAME_ACTOR` and `MELDFRAME_TOKEN`, each with the default of the
    /// development recipes. `get` reads one variable; `token_file` gives the text of `.dev-token` and is asked only when
    /// `MELDFRAME_TOKEN` is not set.
    pub fn from_env(
        get: &dyn Fn(&str) -> Option<String>,
        token_file: &dyn Fn() -> Option<String>,
    ) -> Result<Config, ConfigError> {
        let var = |name: &str, default: &str| {
            get(name)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default.into())
        };
        let token = get("MELDFRAME_TOKEN")
            .filter(|t| !t.trim().is_empty())
            .or_else(token_file)
            .ok_or(ConfigError::NoToken)?;
        Config::new(
            &var("MELDFRAME_SERVER", DEFAULT_SERVER),
            &var("MELDFRAME_FILE", DEFAULT_FILE),
            &var("MELDFRAME_ACTOR", DEFAULT_ACTOR),
            &token,
        )
    }

    /// `from_env` over the process environment and the `.dev-token` of the working directory or the nearest directory above it.
    pub fn from_process() -> Result<Config, ConfigError> {
        let read = || read_token_file(&std::env::current_dir().ok()?);
        Config::from_env(&|name| std::env::var(name).ok(), &read)
    }
}

/// The addresses of `all` that are loopback.
fn loopback_only(all: Vec<SocketAddr>) -> Vec<SocketAddr> {
    all.into_iter().filter(|a| a.ip().is_loopback()).collect()
}

/// The most bytes of a token file that are read: a token is a few dozen.
pub const TOKEN_FILE_MAX: usize = 1024;

/// The nearest `.dev-token` in `start` or a directory above it, looking no higher than the repository root: the first
/// directory that holds a `.git` entry (a directory, or the file of a worktree) is the last one looked in.
fn find_token_file(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let file = dir.join(TOKEN_FILE);
        if file.is_file() {
            return Some(file);
        }
        if dir.join(".git").exists() {
            return None;
        }
    }
    None
}

/// The text of the nearest `.dev-token`, or `None` when there is none or it is larger than [`TOKEN_FILE_MAX`].
fn read_token_file(start: &Path) -> Option<String> {
    let mut text = String::new();
    let limit = u64::try_from(TOKEN_FILE_MAX + 1).ok()?;
    std::fs::File::open(find_token_file(start)?)
        .ok()?
        .take(limit)
        .read_to_string(&mut text)
        .ok()?;
    (text.len() <= TOKEN_FILE_MAX).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| pairs.iter().find(|p| p.0 == name).map(|p| p.1.to_string())
    }

    #[test]
    fn the_token_comes_from_the_environment_before_the_file_and_defaults_fill_the_rest() {
        let file = || Some("from-file-0123456789\n".to_string());
        let c = Config::from_env(&env(&[("MELDFRAME_TOKEN", "from-env-0123456789")]), &file)
            .expect("a config");
        assert_eq!(c.token, Token::new("from-env-0123456789").unwrap());
        assert_eq!(
            (c.file.as_str(), c.actor.as_str()),
            (DEFAULT_FILE, DEFAULT_ACTOR)
        );
        assert_eq!(c.server, Server::parse(DEFAULT_SERVER).unwrap());
        let c = Config::from_env(
            &env(&[("MELDFRAME_FILE", "f2"), ("MELDFRAME_ACTOR", "user:x")]),
            &file,
        )
        .unwrap();
        assert_eq!(c.token, Token::new("from-file-0123456789").unwrap());
        assert_eq!((c.file.as_str(), c.actor.as_str()), ("f2", "user:x"));
        assert_eq!(
            Config::from_env(&env(&[]), &|| None).unwrap_err(),
            ConfigError::NoToken
        );
    }

    #[test]
    fn a_token_is_never_shown_and_never_holds_what_a_header_cannot_carry() {
        let c = Config::new("http://127.0.0.1:1", "f", "a", "secret-token-0123456789").unwrap();
        assert!(!format!("{c:?}").contains("secret-token"));
        assert!(!ConfigError::BadToken.to_string().contains("secret"));
        for bad in ["", "  ", "a b", "a\r\nx: y", "tökén"] {
            assert_eq!(
                Token::new(bad).unwrap_err(),
                ConfigError::BadToken,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_name_that_resolves_to_both_loopback_families_gives_both_and_nothing_else() {
        let all: Vec<SocketAddr> = ["[::1]:80", "127.0.0.1:80", "10.0.0.1:80", "[fe80::1]:80"]
            .iter()
            .map(|a| a.parse().unwrap())
            .collect();
        let kept = loopback_only(all);
        assert_eq!(
            kept.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["[::1]:80", "127.0.0.1:80"]
        );
        assert!(
            !Server::parse("http://localhost:9")
                .unwrap()
                .addresses()
                .is_empty()
        );
    }

    #[test]
    fn only_a_server_on_this_machine_over_plain_http_is_taken() {
        for (url, authority) in [
            ("http://127.0.0.1:8080", "127.0.0.1:8080"),
            ("http://localhost/", "localhost:80"),
            ("http://[::1]:9", "[::1]:9"),
        ] {
            assert_eq!(Server::parse(url).unwrap().authority(), authority, "{url}");
        }
        assert_eq!(
            Server::parse("https://127.0.0.1").unwrap_err(),
            ConfigError::NoTls
        );
        assert!(matches!(
            Server::parse("http://example.com").unwrap_err(),
            ConfigError::NotLoopback(_)
        ));
        assert!(matches!(
            Server::parse("http://10.0.0.5:80").unwrap_err(),
            ConfigError::NotLoopback(_)
        ));
        for bad in [
            "127.0.0.1:80",
            "http://",
            "http://127.0.0.1:x",
            "http://user@127.0.0.1",
            "http://[::1",
        ] {
            assert!(
                matches!(Server::parse(bad).unwrap_err(), ConfigError::BadServer(_)),
                "{bad}"
            );
        }
    }

    /// A directory of its own under the temporary one, removed at the end of the test.
    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Dir {
            use std::hash::{BuildHasher, Hasher};
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u32(std::process::id());
            let dir = std::env::temp_dir().join(format!("meldframe-net-config-{:x}", h.finish()));
            std::fs::create_dir_all(&dir).unwrap();
            Dir(dir)
        }

        fn at(&self, path: &str) -> PathBuf {
            let p = self.0.join(path);
            std::fs::create_dir_all(&p).unwrap();
            p
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_token_file_is_looked_for_up_to_the_repository_root_and_no_higher() {
        let d = Dir::new();
        let (root, inner) = (d.at("repo"), d.at("repo/frontends/rune"));
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(d.0.join(TOKEN_FILE), "outside-the-repository-0123456789").unwrap();
        assert_eq!(
            read_token_file(&inner),
            None,
            "a token above the repository is not taken"
        );
        std::fs::write(root.join(TOKEN_FILE), "in-the-repository-0123456789\n").unwrap();
        assert_eq!(
            read_token_file(&inner).as_deref(),
            Some("in-the-repository-0123456789\n")
        );
        // The file of a worktree stands for the directory too.
        let tree = d.at("tree/sub");
        std::fs::write(d.0.join("tree/.git"), "gitdir: elsewhere").unwrap();
        assert_eq!(read_token_file(&tree), None);
    }

    #[test]
    fn a_token_file_over_a_kilobyte_is_not_read() {
        let d = Dir::new();
        let dir = d.at("repo");
        std::fs::create_dir(dir.join(".git")).unwrap();
        std::fs::write(dir.join(TOKEN_FILE), "a".repeat(TOKEN_FILE_MAX)).unwrap();
        assert_eq!(read_token_file(&dir).map(|t| t.len()), Some(TOKEN_FILE_MAX));
        std::fs::write(dir.join(TOKEN_FILE), "a".repeat(TOKEN_FILE_MAX + 1)).unwrap();
        assert_eq!(read_token_file(&dir), None);
    }
}
