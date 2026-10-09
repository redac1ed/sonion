use crate::head::{find_header_end, parse_headers, CRLF};
use crate::tls::client_config;
use crate::{limits, ProtocolError, Request, Response, SonionUrl, Status, ALPN};
use rustls::pki_types::ServerName;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::{timeout, timeout_at, Instant};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HEAD_TIMEOUT: Duration = Duration::from_secs(15);
const BODY_TIMEOUT: Duration = Duration::from_secs(60);
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_REDIRECTS: usize = 5;

type TlsConn = TlsStream<TcpStream>;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls error: {0}")]
    Tls(String),
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("server did not negotiate ALPN {ALPN:?} (got {0:?})")]
    AlpnMismatch(Option<String>),
    #[error("invalid server name: {0}")]
    BadServerName(String),
    #[error("{0} timed out")]
    Timeout(&'static str),
    #[error("too many redirects (limits {0})")]
    TooManyRedirects(usize),
    #[error("redirect without Location header")]
    RedirectMissingLocation
}

async fn connect(url: &SonionUrl, insecure_dev: bool) -> Result<TlsConn, ClientError> {
    let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect((url.host.as_str(), url.port))).await
        .map_err(|_| ClientError::Timeout("connect"))??;
    let connector = TlsConnector::from(Arc::new(client_config(insecure_dev)));
    let server_name = ServerName::try_from(url.host.clone())
        .map_err(|_| ClientError::BadServerName(url.host.clone()))?;
    let tls = timeout(CONNECT_TIMEOUT, connector.connect(server_name, tcp)).await
        .map_err(|_| ClientError::Timeout("tls handshake"))?
        .map_err(|e| ClientError::Tls(e.to_string()))?;
    let negotiated = tls.get_ref().1.alpn_protocol().map(|p| p.to_vec());
    match negotiated.as_deref() {
        Some(p) if p == ALPN.as_bytes() => Ok(tls),
        other => Err(ClientError::AlpnMismatch(
            other.map(|p| String::from_utf8_lossy(p).into_owned())
        )),
    }
}

async fn send_on(
    tls: &mut TlsConn,
    buf: &mut Vec<u8>,
    request: &Request
) -> Result<Response, ClientError> {
    let write_err = match tls.write_all(&request.serialize()).await {
        Ok(()) => tls.flush().await.err(),
        Err(e) => Some(e)
    };
    buf.clear();
    let head_deadline = Instant::now() + HEAD_TIMEOUT;
    if let Some(e) = write_err {
        let mut tmp = [0u8; 8192];
        match timeout_at(head_deadline, tls.read(&mut tmp)).await {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return Err(ClientError::Io(e)),
            Ok(Ok(n)) => buf.extend_from_slice(&tmp[..n])
        }
    }
    let max_head = limits::MAX_STATUS_LINE + limits::MAX_HEADERS;
    let (body_start, framing) = loop {
        if find_header_end(buf).is_some() {
            let mut parsed = head_framing(buf).ok_or_else(|| ProtocolError::MalformedLine("bad response head".into()))?;
            if request.method == "HEAD" {
                parsed.1 = Framing::Empty;
            }
            break parsed;
        }
        if buf.len() > max_head {
            return Err(ProtocolError::HeadersTooLarge { max: limits::MAX_HEADERS }.into());
        }
        read_more(tls, buf, head_deadline, "response head").await?;
    };
    let body_deadline = Instant::now() + BODY_TIMEOUT;
    loop {
        if body_complete(buf, body_start, &framing)? {
            let consumed = body_consumed(buf, body_start, &framing);
            let resp = if request.method == "HEAD" {
                Response::parse_head(buf)?
            } else {
                Response::parse(&buf[..consumed])?
            };
            buf.drain(..consumed);
            return Ok(resp);
        }
        if buf.len() > max_head + limits::MAX_BODY {
            return Err(ProtocolError::BodyTooLarge { max: limits::MAX_BODY }.into());
        }
        read_more(tls, buf, body_deadline, "response body").await?;
    }
}

fn wants_close(resp: &Response) -> bool {
    resp.get_header("Connection")
        .map(|v| v.eq_ignore_ascii_case("close"))
        .unwrap_or(false)
}

pub async fn fetch(url: &SonionUrl, insecure_dev: bool) -> Result<Response, ClientError> {
    let req = Request::get(&url.encoded_path()).header("Host", &url.host);
    send(url, &req, insecure_dev).await 
}

pub async fn fetch_redirects(
    url: &SonionUrl,
    insecure_dev: bool
) -> Result<Response, ClientError> {
    let mut current = url.clone();
    let client = Client::new(insecure_dev);
    for _ in 0..MAX_REDIRECTS {
        let req = Request::get(&current.encoded_path()).header("Host", &current.host);
        let resp = client.send(&current, &req).await?;
        if !resp.status.is_redirect() {
            return Ok(resp);
        }
        let location = resp.get_header("Location").ok_or(ClientError::RedirectMissingLocation)?;
        current = resolve_redirect(&current, location)?;
    }
    Err(ClientError::TooManyRedirects(MAX_REDIRECTS))
}

fn resolve_redirect(base: &SonionUrl, location: &str) -> Result<SonionUrl, ClientError> {
    if location.starts_with("sonion://") {
        return SonionUrl::parse(location).map_err(ClientError::Protocol);
    }
    if let Some(path) = location.strip_prefix('/') {
        let mut next = base.clone();
        let raw = format!("/{path}");
        let (raw_path, raw_query) = match raw.find('?') {
            Some(i) => (&raw[..i], Some(&raw[i + 1..])),
            None => (raw.as_str(), None)
        };
        next.path = crate::canonicalize_path(&crate::decode_path(raw_path)?)?;
        next.query = raw_query.map(|q| q.to_string());
        return Ok(next);
    }
    Err(ClientError::Protocol(ProtocolError::InvalidUrl(format!(
        "unsupported redirect location: {location}"
    ))))
}

pub async fn send(
    url: &SonionUrl,
    request: &Request,
    insecure_dev: bool,
) -> Result<Response, ClientError> {
    let mut tls = connect(url, insecure_dev).await?;
    let mut buf = Vec::with_capacity(64 * 1024);
    send_on(&mut tls, &mut buf, request).await
}

#[derive(Default)]
pub struct Client {
    insecure_dev: bool,
    conns: Mutex<HashMap<(String, u16), Vec<Pooled>>>
}

struct Pooled {
    tls: TlsConn,
    buf: Vec<u8>, 
    idle_since: Instant
}

impl Client {
    pub fn new(insecure_dev: bool) -> Self {
        Self {
            insecure_dev,
            conns: Mutex::new(HashMap::new())
        }
    }
    pub async fn send(&self, url: &SonionUrl, request: &Request) -> Result<Response, ClientError> {
        let key = (url.host.clone(), url.port);
        let mut pooled = self.take(&key).await;
        let fresh;
        let (tls, buf) = match pooled.take() {
            Some(p) => {
                let mut p = p;
                if !p.buf.is_empty() {
                    let mut discard = Vec::new();
                    std::mem::swap(&mut p.buf, &mut discard);
                }
                (p.tls, p.buf)
            }
            None => {
                fresh = connect(url, self.insecure_dev).await?;
                (fresh, Vec::with_capacity(64 * 1024))
            }
        };
        let mut tls = tls;
        let mut buf = buf;
        match send_on(&mut tls, &mut buf, request).await {
            Ok(resp) => {
                if !wants_close(&resp) && request_wants_keep_alive(request) {
                    self.put(key, tls, buf).await;
                }
                Ok(resp)
            }
            Err(e) => Err(e)
        } 
    }
    async fn take(&self, key: &(String, u16)) -> Option<Pooled> {
        let mut map = self.conns.lock().await;
        let pool = map.get_mut(key)?;
        while let Some(p) = pool.pop() {
            if p.idle_since.elapsed() < IDLE_TIMEOUT {
                return Some(p);
            }
        }
        None
    }
    async fn put(&self, key: (String, u16), tls: TlsConn, buf: Vec<u8>) {
        let mut map = self.conns.lock().await;
        map.entry(key).or_default().push(Pooled {
            tls, 
            buf, 
            idle_since: Instant::now()
        });
    }
    pub async fn close_all(&self) {
        let mut map = self.conns.lock().await;
        for (_, pool) in map.drain() {
            for mut p in pool {
                let _ = p.tls.shutdown().await;
            }
        }
    }
    pub async fn idle_connection_count(&self) -> usize {
        let map = self.conns.lock().await;
        map.values().map(|v| v.len()).sum()
    }
}

fn request_wants_keep_alive(req: &Request) -> bool {
    !req.get_header("Connection").map(|v| v.eq_ignore_ascii_case("close")).unwrap_or(false)
}

enum Framing {
    Empty, 
    ContentLength(usize),
    Chunked
}

fn head_framing(buf: &[u8]) -> Option<(usize, Framing)> {
    let header_end = find_header_end(buf)?;
    let head = std::str::from_utf8(&buf[..header_end]).ok()?;
    let headers = parse_headers(head.split(CRLF).skip(1)).ok()?;
    let mut framing = Framing::Empty;
    for (name, value) in &headers {
        if name.eq_ignore_ascii_case("content-length") {
            framing = Framing::ContentLength(value.trim().parse().ok()?);
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            && value.trim().eq_ignore_ascii_case("chunked")
        {
            framing = Framing::Chunked;
        }
    }
    Some((header_end + 4, framing))
}

fn body_complete(buf: &[u8], body_start: usize, framing: &Framing) -> Result<bool, ProtocolError> {
    match framing {
        Framing::Empty => Ok(true),
        Framing::ContentLength(len) => Ok(buf.len() >= body_start + len),
        Framing::Chunked => chunked_complete(&buf[body_start..]),
    }
}

fn body_consumed(buf: &[u8], body_start: usize, framing:&Framing) -> usize {
    match framing {
        Framing::Empty => body_start,
        Framing::ContentLength(len) => body_start + len,
        Framing::Chunked => chunked_len(&buf[body_start..])
            .map(|n| body_start + n) 
            .unwrap_or(buf.len())
    }
}

fn chunked_len(mut rest: &[u8]) ->  Option<usize> {
    let mut total = 0usize;
    loop {
        let line_end = rest.windows(2).position(|w| w == b"\r\n")?;
        let size_str = std::str::from_utf8(&rest[..line_end]).ok()?;
        let size = usize::from_str_radix(size_str.trim(), 16).ok()?;
        rest = &rest[line_end + 2..];
        total += line_end + 2;
        if size == 0 {
            if rest.len() < 2 {
                return None;
            }
            return Some(total + 2);
        }
        if rest.len() < size + 2 {
            return None;
        }
        rest = &rest[size + 2..];
        total += size + 2;
    }
}

fn chunked_complete(mut rest: &[u8]) -> Result<bool, ProtocolError> {
    let mut total = 0usize;
    loop {
        let Some(line_end) = rest.windows(2).position(|w| w == b"\r\n") else {
            return Ok(false);
        };
        let size_str = std::str::from_utf8(&rest[..line_end])
            .map_err(|_| ProtocolError::InvalidChunk("non-utf8 size".into()))?;
        let size = usize::from_str_radix(size_str.trim(), 16)
            .map_err(|_| ProtocolError::InvalidChunk(size_str.into()))?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            return Ok(rest.len() >= 2);
        }
        total = match total.checked_add(size) {
            Some(t) if t <= limits::MAX_BODY => t,
            _ => {
                return Err(ProtocolError::BodyTooLarge {
                    max: limits::MAX_BODY,
                });
            }
        };
        if rest.len() < size + 2 {
            return Ok(false);
        }
        rest = &rest[size + 2..];
    }
}

async fn read_more<S>(
    stream: &mut S,
    buf: &mut Vec<u8>,
    deadline: Instant,
    what: &'static str,
) -> Result<(), ClientError>
where
    S: AsyncReadExt + Unpin,
{
    let mut tmp = [0u8; 16 * 1024];
    let n = timeout_at(deadline, stream.read(&mut tmp))
        .await
        .map_err(|_| ClientError::Timeout(what))??;
    if n == 0 {
        return Err(ProtocolError::UnexpectedEof.into());
    }
    buf.extend_from_slice(&tmp[..n]);
    Ok(())
}

pub fn status_allows_body(status: Status) -> bool {
    !matches!(status, Status::NotModified)
}