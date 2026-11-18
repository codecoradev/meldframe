//! A cap on open connections: [`CappedListener`] stops accepting at the cap and resumes when one closes; [`serve`]
//! runs the router on it with a header-read timeout.

use axum::serve::Listener;
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// A TCP listener that serves at most `max` connections at once; the rest wait in the OS backlog.
#[derive(Debug)]
pub struct CappedListener {
    inner: TcpListener,
    slots: Arc<Semaphore>,
    per_client: usize,
    clients: Arc<Mutex<HashMap<String, usize>>>,
}

impl CappedListener {
    /// Serve at most `max` connections at once and at most `per_client` from one client key (see
    /// [`crate::client_key`]); a connection over the per-client cap is closed at once.
    pub fn new(inner: TcpListener, max: usize, per_client: usize) -> Self {
        CappedListener {
            inner,
            slots: Arc::new(Semaphore::new(max.max(1))),
            per_client: per_client.max(1),
            clients: Arc::default(),
        }
    }
}

/// A connection that holds its slot until it is dropped.
#[derive(Debug)]
pub struct CappedIo {
    io: TcpStream,
    _slot: OwnedSemaphorePermit,
    _client: ClientSlot,
}

/// One connection of a client key; released on drop.
#[derive(Debug)]
struct ClientSlot {
    clients: Arc<Mutex<HashMap<String, usize>>>,
    key: String,
}

impl Drop for ClientSlot {
    fn drop(&mut self) {
        let mut m = self.clients.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(n) = m.get_mut(&self.key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                m.remove(&self.key);
            }
        }
    }
}

impl Listener for CappedListener {
    type Io = CappedIo;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let slot = self
                .slots
                .clone()
                .acquire_owned()
                .await
                .expect("the semaphore is never closed");
            let (io, addr) = Listener::accept(&mut self.inner).await;
            let key = crate::client_key(addr.ip());
            {
                let mut m = self.clients.lock().unwrap_or_else(|p| p.into_inner());
                let n = m.entry(key.clone()).or_insert(0);
                if *n >= self.per_client {
                    continue; // drops `io` and `slot`: this client is over its share
                }
                *n += 1;
            }
            let client = ClientSlot {
                clients: self.clients.clone(),
                key,
            };
            return (
                CappedIo {
                    io,
                    _slot: slot,
                    _client: client,
                },
                addr,
            );
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

impl AsyncRead for CappedIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for CappedIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

/// Serve `app` on `listener`, HTTP/1.1 with upgrades (WebSocket), each request carrying the client address as
/// `ConnectInfo`. Unlike `axum::serve` it closes a connection whose request headers are not complete within
/// `header_read_timeout`, also one that sends nothing or stays idle between requests (issue 170): a slow client
/// cannot keep one of the capped connections for ever. An upgraded connection is past it. Behind a reverse proxy
/// listed in `trusted_proxies`, the client address of a request is the one the proxy put in `X-Forwarded-For`
/// ([`real_client`]), so the per-address buckets count clients, not the proxy; the connection caps of
/// [`CappedListener`] still count the proxy's connections.
pub async fn serve(
    mut listener: CappedListener,
    app: axum::Router,
    header_read_timeout: std::time::Duration,
    trusted_proxies: Vec<std::net::IpAddr>,
) {
    let trusted: Arc<[std::net::IpAddr]> = trusted_proxies.into();
    loop {
        let (io, addr) = Listener::accept(&mut listener).await;
        let (app, trusted) = (app.clone(), trusted.clone());
        tokio::spawn(async move {
            // The client address goes into each request, as `axum::serve` does, without rebuilding the router.
            let svc = hyper::service::service_fn(
                move |mut req: hyper::Request<hyper::body::Incoming>| {
                    let client = real_client(addr, req.headers(), &trusted);
                    req.extensions_mut()
                        .insert(axum::extract::ConnectInfo(client));
                    tower::Service::call(&mut app.clone(), req)
                },
            );
            let mut http = hyper::server::conn::http1::Builder::new();
            http.timer(hyper_util::rt::TokioTimer::new())
                .header_read_timeout(header_read_timeout);
            let conn = http.serve_connection(hyper_util::rt::TokioIo::new(io), svc);
            // A connection that ends with an error (a reset, the timeout) has nothing more to do.
            let _ = conn.with_upgrades().await;
        });
    }
}

/// The client of a request: `peer` itself, unless `peer` is one of `trusted` proxies; then the rightmost address of
/// the `X-Forwarded-For` headers (read in order) that is not a trusted proxy, since each proxy appends the address it
/// saw and anything left of that is what the client wrote. Addresses are compared as IPv4 when they are IPv4-mapped
/// IPv6 (an IPv4 proxy seen through an IPv6 socket); an entry may carry a port (`1.2.3.4:80`, `[::1]:80`); empty
/// entries are skipped. No such header, or an entry that is not an address, leaves `peer`. Only `X-Forwarded-For` is
/// read, not `Forwarded`. The port of a forwarded client is 0.
pub fn real_client(
    peer: std::net::SocketAddr,
    headers: &axum::http::HeaderMap,
    trusted: &[std::net::IpAddr],
) -> std::net::SocketAddr {
    let trusted_ip = |ip: std::net::IpAddr| {
        trusted
            .iter()
            .any(|t| t.to_canonical() == ip.to_canonical())
    };
    if !trusted_ip(peer.ip()) {
        return peer;
    }
    let entries: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect();
    for e in entries.iter().rev() {
        let ip = e
            .parse::<std::net::IpAddr>()
            .or_else(|_| e.parse::<std::net::SocketAddr>().map(|a| a.ip()));
        match ip {
            Ok(ip) if trusted_ip(ip) => continue,
            Ok(ip) => return std::net::SocketAddr::new(ip.to_canonical(), 0),
            Err(_) => return peer,
        }
    }
    peer
}
