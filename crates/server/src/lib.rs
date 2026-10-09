use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sonion_protocol::date::{format_http_date, parse_http_date};
use sonion_protocol::head::find_header_end;
use sonion_protocol::{ALPN, ProtocolError, Request, Response, Status, limits, system_time_to_unix};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Instant, timeout_at};
use tokio_rustls::TlsAcceptor;
use tracing::{info, warn};

const HEAD_TIMEOUT: Duration = Duration::from_secs(15);
const BODY_TIMEOUT: Duration = Duration::from_secs(60);
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const CHUNK_THRESHOLD: usize = 256 * 1024;
const CHUNK_SIZE: usize = 16 * 1024;
const MAX_KEEPALIVE_REQUESTS: usize = 1000;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to load TLS identity: {0}")]
    TlsIdentity(String),
}

pub struct Server {
    root: PathBuf,
    acceptor: TlsAcceptor,
    listener: TcpListener,
}

impl Server {
    pub async fn bind(
        addr: SocketAddr,
        root: impl AsRef<Path>,
        certs: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Result<Self, ServerError> {
        let root = root.as_ref().canonicalize().map_err(ServerError::Io)?;
        let mut config =
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| ServerError::TlsIdentity(e.to_string()))?;
        config.alpn_protocols = vec![ALPN.as_bytes().to_vec()];
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            root,
            acceptor: TlsAcceptor::from(Arc::new(config)),
            listener,
        })
    }
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }
    pub async fn run(self) -> Result<(), ServerError> {
        let root = Arc::new(self.root);
        loop {
            let (tcp, peer) = self.listener.accept().await?;
            let acceptor = self.acceptor.clone();
            let root = root.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_conn(acceptor, tcp, &root).await {
                    warn!(%peer, error = %e, "connection error");
                }
            });
        }
    }
}

async fn handle_conn(
    acceptor: TlsAcceptor,
    tcp: tokio::net::TcpStream,
    root: &Path,
) -> anyhow::Result<()> {
    let mut tls = acceptor.accept(tcp).await?;
    match tls.get_ref().1.alpn_protocol() {
        Some(p) if p == ALPN.as_bytes() => {}
        other => {
            anyhow::bail!(
                "ALPN mismatch: got {:?}",
                other.map(|p| String::from_utf8_lossy(p).into_owned())
            );
        }
    }
    let mut buf = Vec::with_capacity(16 * 1024);
    let max_head = limits::MAX_REQUEST_LINE + limits::MAX_HEADERS + 4;
    let mut served = 0usize;
    loop {
        if served >= MAX_KEEPALIVE_REQUESTS {
            return Ok(());
        }
        let deadline = if served == 0 {
            Instant::now() + HEAD_TIMEOUT
        } else {
            Instant::now() + IDLE_TIMEOUT
        };
        let request = match read_request(&mut tls, &mut buf, deadline, max_head).await? {
            Some(r) => r, 
            None => return Ok(())
        };
        served += 1;
        let started = Instant::now();
        let client_close = request.get_header("Connection") 
            .map(|v| v.eq_ignore_ascii_case("close"))
            .unwrap_or(false);
        if request.method != "GET" && request.method != "HEAD" {
            let mut resp = error_page(root, Status::MethodNotAllowed, b"method not allowed").await;
            resp.set_header("Allow", "GET, HEAD");
            write_response(&mut tls, &request, resp, client_close).await?;
            info!(method = %request.method, path = %request.path, status = 405, elapsed_us = &started.elapsed().as_micros(), "response");
            return Ok(());
        }
        let response = build_response(root, &request).await;
        let status = response.status.code();
        let bytes = response.body.len();
        write_response(&mut tls, &request, response, client_close).await?;
        info!(method = %request.method, path = %request.path, %status, %bytes, elapsed_us = %started.elapsed().as_micros(), "response");
        if client_close {
            return Ok(());
        }
    }
}

async fn read_request<S>(
    tls: &mut S,
    buf: &mut Vec<u8>,
    deadline: Instant,
    max_head: usize,
) -> anyhow::Result<Option<Request>> 
where 
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    loop {
        if find_header_end(buf).is_some() {
            break;
        }
        if buf.len() > max_head {
            write_simple(tls, Status::BadRequest, b"request head too large").await?;
            anyhow::bail!("request head too large");
        }
        let mut tmp = [0u8; 8 * 1024];
        let n = match timeout_at(deadline, tls.read(&mut tmp)).await {
            Ok(Ok(0)) => {
                if buf.is_empty() {
                    return Ok(None);
                }
                write_simple(tls, Status::BadRequest, b"incomplete request").await?;
                return Ok(None);
            }
            Ok(Ok(n)) => n, 
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                if buf.is_empty() {
                    return Ok(None);
                }
                write_simple(tls, Status::BadRequest, b"request timeout").await?;
                return Ok(None);
            }
        };
        buf.extend_from_slice(&tmp[..n]);
    }
    let body_deadline = Instant::now() + BODY_TIMEOUT;
    loop {
        match Request::parse(buf) {
            Ok(r) => {
                let consumed = request_wire_len(buf, &r);
                buf.drain(..consumed);
                return Ok(Some(r));
            }
            Err(ProtocolError::UnexpectedEof) => {
                if buf.len() > max_head + limits::MAX_BODY {
                    write_simple(tls, Status::PayloadTooLarge, b"payload too large").await?;
                    anyhow::bail!("request too large");
                }
                let mut tmp = [0u8; 16 * 1024];
                let n = match timeout_at(body_deadline, tls.read(&mut tmp)).await {
                    Ok(Ok(0)) => {
                        write_simple(tls, Status::BadRequest, b"incomplete requeest").await?;
                        return Ok(None);
                    }
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => return Err(e.into()),
                    Err(_) => {
                        write_simple(tls, Status::BadRequest, b"request timeout").await?;
                        return Ok(None);
                    }
                };
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(e) => {
                warn!(error = %e, "malformed request");
                write_simple(tls, Status::BadRequest, b"bad request").await?;
                return Ok(None);
            }
        }
    }
}

fn request_wire_len(buf: &[u8], req: &Request) -> usize {
    let head_end = find_header_end(buf).map(|p| p + 4).unwrap_or(buf.len());
    let body_len = req.get_header("Content-Length")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    (head_end + body_len).min(buf.len())
}

async fn write_response<S: AsyncWriteExt + Unpin>(
    tls: &mut S,
    request: &Request,
    mut response: Response,
    closing: bool,
) -> anyhow::Result<()> {
    if closing {
        response.set_header("Connection", "close");
    }
    let head_only = request.method == "HEAD" || response.status == Status::NotModified;
    let wire = if head_only {
        response.serialize_head()
    } else if response.body.len() > CHUNK_THRESHOLD {
        response.serialize_chunked(CHUNK_SIZE)
    } else {
        response.serialize()
    };
    tls.write_all(&wire).await?;
    tls.flush().await?;
    Ok(())
}

async fn build_response(root: &Path, request: &Request) -> Response {
    let raw_path = request.path.split('?').next().unwrap_or("/");
    let Some(path) = resolve_path(root, raw_path) else {
        return error_page(root, Status::BadRequest, b"invalid path").await;
    };
    if path.is_dir() && !raw_path.ends_with('/') {
        let mut resp = Response::new(Status::MovedPermanently, Vec::new());
        resp.set_header("Location", format!("{raw_path}/"));
        return resp;
    }
    let path = if path.is_dir() {
        path.join("index.html")
    } else {
        path
    };
    let md = match tokio::fs::metadata(&path).await {
        Ok(md) => md,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return not_found_response(root, raw_path).await;
        }
        Err(_) => return error_page(root, Status::InternalServerError, b"internal server error").await
    };
    if !md.is_file() {
        return not_found_response(root, raw_path).await;
    }
    if md.len() > limits::MAX_BODY as u64 {
        return error_page(root, Status::PayloadTooLarge, b"payload too large").await;
    }
    let mtime = md.modified().map(system_time_to_unix).unwrap_or(0);
    if let Some(ims) = request.get_header("If-Modified-Since") {
        if let Some(since) = parse_http_date(ims) {
            if mtime <= since {
                let mut resp = Response::new(Status::NotModified, Vec::new());
                resp.set_header("Last-Modified", format_http_date(mtime));
                return resp;
            }
        }
    }
    match tokio::fs::read(&path).await {
        Ok(body) => {
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            let mut resp = Response::new(Status::Ok, body);
            resp.set_header("Content-Type", &mime);
            resp.set_header("Last-Modified", format_http_date(mtime));
            resp
        }
        Err(_) => error_page(root, Status::InternalServerError, b"internal server error").await
    }
}

async fn not_found_response(root: &Path, raw_path: &str) -> Response {
    let looks_like_file = raw_path.rsplit('/').next().is_some_and(|s| s.contains('.'));
    if looks_like_file {
        error_page(root, Status::NotFound, b"not found").await
    } else {
        let index = root.join("index.html");
        match tokio::fs::read(&index).await {
            Ok(body) => {
                let md = tokio::fs::metadata(&index).await.ok();
                let mut resp = Response::new(Status::Ok, body);
                resp.set_header("Content-Type", "text/html");
                if let Some(mtime) = md.and_then(|m| m.modified().ok()).map(system_time_to_unix) {
                    resp.set_header("Last-Modified", format_http_date(mtime));
                }
                resp
            }
            Err(_) => error_page(root, Status::NotFound, b"not found").await
        }
    }
}

async fn error_page(root: &Path, status: Status, fallback: &[u8]) -> Response {
    let custom = root.join(format!("{}.html", status.code()));
    if let Ok(body) = tokio::fs::read(&custom).await {
        if body.len() <= limits::MAX_BODY {
            let mut resp = Response::new(status, body);
            resp.set_header("Content-Type", "text/html");
            return resp;
        }
    }
    simple(status,fallback)
}

fn resolve_path(root: &Path, request_path: &str) -> Option<PathBuf> {
    let decoded = sonion_protocol::decode_path(request_path).ok()?;
    let mut out = root.to_path_buf();
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            ".." => return None,
            s if s.contains(':') || s.contains('\0') => return None,
            s => out.push(s),
        }
    }
    if !out.starts_with(root) {
        return None;
    }
    Some(out)
}

fn simple(status: Status, body: &[u8]) -> Response {
    let mut resp = Response::new(status, body.to_vec());
    resp.set_header("Content-Type", "text/plain");
    resp
}

async fn write_simple<S: AsyncWriteExt + Unpin>(
    stream: &mut S,
    status: Status,
    body: &[u8],
) -> anyhow::Result<()> {
    stream.write_all(&simple(status, body).serialize()).await?;
    stream.flush().await?;
    Ok(())
}