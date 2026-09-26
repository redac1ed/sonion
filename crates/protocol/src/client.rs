use crate::head::{find_header_end, parse_headers, CRLF};
use crate::tls::client_config;
use crate::{limits, ProtocolError, Request, Response, SonionUrl, ALPN};
use rustls::pki_types::ServerName;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, timeout_at, Instant};
use tokio_rustls::TlsConnector;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HEAD_TIMEOUT: Duration = Duration::from_secs(15);
const BODY_TIMEOUT: Duration = Duration::from_secs(60);

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
}

pub async fn fetch(url: &SonionUrl, insecure_dev: bool) -> Result<Response, ClientError> {
    let req = Request::get(&url.encoded_path()).header("Host", &url.host);
    send(url, &req, insecure_dev).await //get url.path
}

pub async fn send(
    url: &SonionUrl,
    request: &Request,
    insecure_dev: bool,
) -> Result<Response, ClientError> {
    let tcp = timeout(
        CONNECT_TIMEOUT,
        TcpStream::connect((url.host.as_str(), url.port)),
    )
    .await
    .map_err(|_| ClientError::Timeout("connect"))??;
    let connector = TlsConnector::from(Arc::new(client_config(insecure_dev)));
    let server_name = ServerName::try_from(url.host.clone())
        .map_err(|_| ClientError::BadServerName(url.host.clone()))?;
    let mut tls = timeout(CONNECT_TIMEOUT, connector.connect(server_name, tcp))
        .await
        .map_err(|_| ClientError::Timeout("tls handshake"))?
        .map_err(|e| ClientError::Tls(e.to_string()))?;
    let negotiated = tls.get_ref().1.alpn_protocol().map(|p| p.to_vec());
    match negotiated.as_deref() {
        Some(p) if p == ALPN.as_bytes() => {}
        other => {
            return Err(ClientError::AlpnMismatch(
                other.map(|p| String::from_utf8_lossy(p).into_owned()),
            ));
        }
    }
    tls.write_all(&request.serialize()).await?;
    tls.flush().await?;
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let max_head = limits::MAX_STATUS_LINE + limits::MAX_HEADERS;
    let head_deadline = Instant::now() + HEAD_TIMEOUT;
    let (body_start, framing) = loop {
        if find_header_end(&buf).is_some() {
            let mut parsed = head_framing(&buf)
                .ok_or_else(|| ProtocolError::MalformedLine("bad response head".into()))?;
            if request.method == "HEAD" {
                parsed.1 = Framing::Empty;
            }
            break parsed;
        }
        if buf.len() > max_head {
            return Err(ProtocolError::HeadersTooLarge {
                max: limits::MAX_HEADERS,
            }
            .into());
        }
        read_more(&mut tls, &mut buf, head_deadline, "response head").await?;
    };
    let body_deadline = Instant::now() + BODY_TIMEOUT;
    loop {
        if body_complete(&buf, body_start, &framing)? {
            return Ok(if request.method == "HEAD" {
                Response::parse_head(&buf)?
            } else {
                Response::parse(&buf)?
            });
        }
        if buf.len() > max_head + limits::MAX_BODY {
            return Err(ProtocolError::BodyTooLarge {
                max: limits::MAX_BODY,
            }
            .into());
        }
        read_more(&mut tls, &mut buf, body_deadline, "response body").await?;
    }
}

enum Framing {
    Empty,
    ContentLength(usize),
    Chunked,
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
// check to not reparse on every read
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
        total += match total.checked_add(size) {
            Some(t)if t <= limits::MAX_BODY => t,
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
