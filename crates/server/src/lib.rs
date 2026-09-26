use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sonion_protocol::head::find_header_end;
use sonion_protocol::{ALPN, Request, Response, Status, limits};
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
const CHUNK_THRESHOLD: usize = 256 * 1024;
const CHUNK_SIZE: usize = 16 * 1024;

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
    // uses tls 1.3
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
        // enforce alpn
        Some(p) if p == ALPN.as_bytes() => {}
        other => {
            anyhow::bail!(
                "ALPN mismatch: got {:?}",
                other.map(|p| String::from_utf8_lossy(p).into_owned())
            );
        }
    }
    let deadline = Instant::now() + HEAD_TIMEOUT; // read req head with limits/deadline
    let mut buf = Vec::with_capacity(16 * 1024);
    let max_head = limits::MAX_REQUEST_LINE + limits::MAX_HEADERS + 4;
    loop {
        if find_header_end(&buf).is_some() {
            break;
        }
        if buf.len() > max_head {
            write_simple(&mut tls, Status::BadRequest, b"request head too large").await?;
            anyhow::bail!("request head too large");
        }
        let mut tmp = [0u8; 8 * 1024];
        let n = match timeout_at(deadline, tls.read(&mut tmp)).await {
            Ok(Ok(0)) => anyhow::bail!("client closed before request"),
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => anyhow::bail!("request head timeout"),
        };
        buf.extend_from_slice(&tmp[..n]);
    }
    let request = match Request::parse(&buf) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "malformed request");
            write_simple(&mut tls, Status::BadRequest, b"bad request").await?;
            return Ok(());
        }
    };
    info!(method = %request.method, path = %request.path, "request");
    let response = build_response(root, &request).await;
    let wire = if request.method == "HEAD" {
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
    let Some(path) = resolve_path(root, &request.path) else {
        return simple(Status::BadRequest, b"invalid path");
    };
    let path = if path.is_dir() {
        path.join("index.html")
    } else {
        path
    };
    match tokio::fs::metadata(&path).await {
        Ok(md) if md.is_file() && md.len() > limits::MAX_BODY as u64 => {
            return simple(Status::PayloadTooLarge, b"payload too large");
        }
        _ => {}
    }
    match tokio::fs::read(&path).await {
        Ok(body) => {
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            let mut resp = Response::new(Status::Ok, body);
            resp.set_header("Content-Type", &mime);
            resp
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let looks_like_file = request
                .path
                .rsplit('/')
                .next()
                .is_some_and(|s| s.contains('.'));
            if looks_like_file {
                simple(Status::NotFound, b"not found")
            } else {
                match tokio::fs::read(root.join("index.html")).await {
                    Ok(body) => {
                        let mut resp = Response::new(Status::Ok, body);
                        resp.set_header("Content-Type", "text/html");
                        resp
                    }
                    Err(_) => simple(Status::NotFound, b"not found"),
                }
            }
        }
        Err(_) => simple(Status::InternalServerError, b"internal server error"),
    }
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
