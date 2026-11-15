use meldframe_http::{Config, DevTokens, check_dev_tokens_allowed, router};
use meldframe_server::{Server, SharedServer};
use std::net::SocketAddr;
use std::sync::Arc;

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
    check_dev_tokens_allowed(
        !cfg!(debug_assertions),
        std::env::var("MELDFRAME_ALLOW_DEV_TOKENS").ok().as_deref(),
    )?;
    let spec = std::env::var("MELDFRAME_DEV_TOKENS").map_err(
        |_| "set MELDFRAME_DEV_TOKENS to token:workspace:actor entries, comma separated",
    )?;
    let app = router(
        Arc::new(DevTokens::parse(&spec)?),
        Arc::new(SharedServer::new(server)),
        Config::default(),
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("listening on {addr}");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    })?;
    Ok(())
}
