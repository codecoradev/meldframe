use axum::serve::ListenerExt;
use meldframe_http::{
    CappedListener, Config, DevTokens, check_dev_listen, check_dev_tokens_allowed,
    dev_tokens_warning,
};
use meldframe_server::{Server, SharedServer};
use std::net::SocketAddr;
use std::sync::Arc;

/// Open connections served at once; more wait in the backlog.
const MAX_CONNECTIONS: usize = 512;
/// Open connections one client (an IPv4 address or an IPv6 /64) may hold.
const MAX_PER_CLIENT: usize = 32;

fn main() {
    let server = Server::assemble();
    for module in server.registry().module_names() {
        println!("module {module}");
    }
    for tool in server.registry().agent_tools() {
        println!("tool {} ({})", tool.name, tool.tier.as_str());
    }
    // Serve only when asked, and read secrets from the environment, never from argv.
    if let Ok(listen) = std::env::var("MELDFRAME_LISTEN")
        && let Err(e) = serve(&listen, server)
    {
        eprintln!("meldframe-server: {e}");
        std::process::exit(1);
    }
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
    let shared = Arc::new(SharedServer::new(server));
    let auth = Arc::new(DevTokens::parse(&spec)?);
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
        };
        meldframe_http::router_with_mcp_config(auth, shared.clone(), shared, Config::default(), mcp)
    };
    #[cfg(not(feature = "mcp"))]
    let app = meldframe_http::router(auth, shared, Config::default());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("listening on {addr}");
        let listener =
            CappedListener::new(listener, MAX_CONNECTIONS, MAX_PER_CLIENT).tap_io(|_| {});
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    })?;
    Ok(())
}
