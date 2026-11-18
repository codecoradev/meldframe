use meldframe_http::{
    CappedListener, Config, DevTokens, check_dev_listen, check_dev_tokens_allowed,
    dev_tokens_warning,
};
use meldframe_http_live::LiveConfig;
use meldframe_server::{Backing, Server, SharedServer, TOUCH_WINDOW_MS};
use std::net::SocketAddr;
use std::sync::Arc;

/// Open connections served at once; more wait in the backlog.
const MAX_CONNECTIONS: usize = 512;
/// Open connections one client (an IPv4 address or an IPv6 /64) may hold.
const MAX_PER_CLIENT: usize = 32;
/// A connection whose request headers are not complete within this time is closed (issue 170).
const HEADER_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The capture relay's limits: 4 MiB images (about 5.5 MiB as base64) of up to 16 million pixels, 20 seconds
/// for the editor to answer.
#[cfg(feature = "capture")]
const CAPTURE: capture::Config = capture::Config {
    max_bytes: 4 << 20,
    max_pixels: 1 << 24,
    timeout_ms: 20_000,
    max_pending: 64,
    max_pending_per_client: 4,
    max_clients: 256,
    max_pending_per_workspace: 16,
    max_pending_per_principal: 4,
    max_clients_per_workspace: 32,
};

fn main() {
    // Serve only when asked, and read secrets from the environment, never from argv. Postgres (and its
    // migrations) is touched only when serving: DATABASE_URL set, else memory.
    let listen = std::env::var("MELDFRAME_LISTEN").ok();
    let backing = if listen.is_some() {
        Backing::from_env().unwrap_or_else(|e| fail(&e))
    } else {
        Backing::memory()
    };
    let server = Server::assemble_in(Vec::new(), backing).with_touch_hints(TOUCH_WINDOW_MS);
    for module in server.registry().module_names() {
        println!("module {module}");
    }
    for tool in server.registry().agent_tools() {
        println!("tool {} ({})", tool.name, tool.tier.as_str());
    }
    if let Some(listen) = listen
        && let Err(e) = serve(&listen, server)
    {
        fail(&e);
    }
}

fn fail(e: &dyn std::fmt::Display) -> ! {
    eprintln!("meldframe-server: {e}");
    std::process::exit(1);
}

fn serve(listen: &str, server: Server) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = listen.parse()?;
    // Gatehouse (M2) replaces this adapter; until then only development tokens exist.
    let release = !cfg!(debug_assertions);
    let allow = std::env::var("MELDFRAME_ALLOW_DEV_TOKENS").ok();
    check_dev_tokens_allowed(release, allow.as_deref())?;
    check_dev_listen(
        addr,
        std::env::var("MELDFRAME_ALLOW_DEV_PUBLIC").ok().as_deref(),
    )?;
    if let Some(warning) = dev_tokens_warning(release, allow.as_deref()) {
        eprintln!("{warning}");
    }
    let spec = std::env::var("MELDFRAME_DEV_TOKENS").map_err(
        |_| "set MELDFRAME_DEV_TOKENS to token:workspace:actor entries, comma separated",
    )?;
    // The assets count against the storage limit of their workspace, in the gate and in the upload route.
    #[cfg(feature = "assets")]
    let (blobs, usage) = open_assets(release)?;
    #[cfg(feature = "assets")]
    let server = {
        let held = usage.clone();
        server.with_extra_storage(move |ws| held.used(ws).unwrap_or(0))
    };
    #[cfg(feature = "capture")]
    let capture_policy = server.registry().policy_for("capture");
    #[cfg(feature = "assets")]
    let upload_policy = server.registry().policy_for(assets::UPLOAD_TOOL);
    // `shared` outlives `block_on`: the last reference to a PgStore runtime must not drop inside one.
    let shared = Arc::new(SharedServer::new(server));
    let auth = Arc::new(DevTokens::parse(&spec)?);
    let config = Config::default();
    let clock = config.clock.clone();
    // One state for every transport: one authenticator, one set of limits, one path to the submitter.
    let state = meldframe_http::shared(auth, shared.clone(), config);
    #[cfg(feature = "capture")]
    let cap = meldframe_http_capture::Capture::new(
        state.clone(),
        shared.clone(),
        shared.clone(),
        clock.clone(),
        CAPTURE,
        meldframe_http_capture::Options {
            policy: capture_policy,
            ..Default::default()
        },
    );
    #[allow(unused_mut, reason = "only the capture feature sets the hook")]
    let mut live_config = LiveConfig::default();
    #[cfg(feature = "capture")]
    {
        live_config.hook = Some(cap.hook());
    }
    let (live, timer) =
        meldframe_http_live::routes(state.clone(), shared.clone(), live_config, clock);
    // Liveness for scripts: no token, no data (tools/dev-smoke.sh waits on it).
    let health = axum::Router::new().route(
        "/v1/health",
        axum::routing::get(|| async { r#"{"status":"ok"}"# }),
    );
    #[allow(unused_mut, reason = "only the capture feature adds a route")]
    let mut app = meldframe_http::routes(state.clone())
        .merge(live)
        .merge(health)
        .merge(meldframe_http_snapshot::routes(
            state.clone(),
            shared.clone(),
        ));
    #[cfg(feature = "capture")]
    {
        app = app.merge(cap.routes());
    }
    // The tools of other crates, served by MCP besides its own.
    #[cfg(feature = "mcp")]
    #[allow(unused_mut, reason = "only the features that add a tool push to it")]
    let mut extra_tools: Vec<Arc<dyn meldframe_http::mcp::Extra>> = Vec::new();
    #[cfg(feature = "capture")]
    extra_tools.push(cap.tool());
    #[cfg(feature = "assets")]
    {
        let ops = shared.clone();
        let assets = meldframe_http_assets::Assets::new(
            state.clone(),
            blobs,
            usage,
            Arc::new(move |ws| ops.ops_storage(ws)),
            meldframe_http_assets::Options::default(),
        );
        app = app.merge(assets.routes());
        extra_tools.push(assets.tool(upload_policy));
    }
    #[cfg(feature = "mcp")]
    let app = {
        // MCP answers to loopback host names unless MELDFRAME_MCP_HOSTS lists others (comma separated).
        let hosts = std::env::var("MELDFRAME_MCP_HOSTS").unwrap_or_default();
        let mcp = meldframe_http::mcp::McpConfig {
            allowed_hosts: hosts
                .split(',')
                .map(str::trim)
                .filter(|h| !h.is_empty())
                .map(str::to_owned)
                .collect(),
            extra: extra_tools,
        };
        app.merge(meldframe_http::mcp::routes(state, shared.clone(), mcp))
    };
    // Reverse proxies whose X-Forwarded-For is believed (comma-separated addresses); none by default.
    let trusted = trusted_proxies(&std::env::var("MELDFRAME_TRUSTED_PROXIES").unwrap_or_default())?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let served = rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("listening on {addr}");
        // Heartbeats and the reaping of silent live clients (ADR-0009) run on this timer.
        tokio::spawn(timer.run());
        let listener = CappedListener::new(listener, MAX_CONNECTIONS, MAX_PER_CLIENT);
        meldframe_http::serve(listener, app, HEADER_READ_TIMEOUT, trusted).await;
        Ok::<(), std::io::Error>(())
    });
    drop(shared); // after block_on: see above
    Ok(served?)
}

/// The store of the assets in its directory (checked now, so a bad one is an error at start and not a 503 on the
/// first upload) and the count of their bytes.
#[cfg(feature = "assets")]
fn open_assets(
    release: bool,
) -> Result<(Arc<dyn assets::BlobStore>, assets::Usage), Box<dyn std::error::Error>> {
    let dir = assets_dir(
        release,
        std::env::var("MELDFRAME_ASSETS_DIR").ok().as_deref(),
    )?;
    std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::metadata(&dir))
        .and_then(|m| match m.permissions().readonly() {
            true => Err(std::io::Error::other("the directory is read-only")),
            false => Ok(()),
        })
        .map_err(|e| format!("MELDFRAME_ASSETS_DIR {dir}: {e}"))?;
    println!("assets in {dir}");
    let blobs: Arc<dyn assets::BlobStore> = Arc::new(assets::DiskBlobs::new(dir));
    let usage = assets::Usage::new(blobs.clone());
    Ok((blobs, usage))
}

/// The directory of the assets (`MELDFRAME_ASSETS_DIR`, ADR-0015): required in a release build, so images are
/// never written to a place nobody chose; a development build keeps them in `.dev-assets`.
#[cfg(feature = "assets")]
fn assets_dir(release: bool, set: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    match (set.map(str::trim).filter(|d| !d.is_empty()), release) {
        (Some(dir), _) => Ok(dir.to_owned()),
        (None, false) => Ok(".dev-assets".to_owned()),
        (None, true) => {
            Err("set MELDFRAME_ASSETS_DIR to the directory that keeps the assets".into())
        }
    }
}

/// The addresses of `MELDFRAME_TRUSTED_PROXIES`: comma-separated, blanks ignored; one that is not an address is an
/// error, so a typo does not silently trust nothing.
fn trusted_proxies(spec: &str) -> Result<Vec<std::net::IpAddr>, Box<dyn std::error::Error>> {
    spec.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse().map_err(|_| {
                format!("MELDFRAME_TRUSTED_PROXIES: {s:?} is not an IP address").into()
            })
        })
        .collect()
}

#[cfg(all(test, feature = "assets"))]
mod tests {
    use super::assets_dir;

    #[test]
    fn a_release_build_needs_the_assets_directory_and_a_development_build_has_a_default() {
        assert_eq!(assets_dir(false, None).unwrap(), ".dev-assets");
        assert_eq!(assets_dir(false, Some("  ")).unwrap(), ".dev-assets");
        assert!(assets_dir(true, None).is_err());
        assert!(assets_dir(true, Some("")).is_err());
        assert_eq!(
            assets_dir(true, Some("/srv/assets")).unwrap(),
            "/srv/assets"
        );
    }
}
