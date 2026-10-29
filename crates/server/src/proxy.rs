use sonion_protocol::head::{CRLF, find_header_end, parse_headers};
use sonion_protocol::{Request, Response, Status, limits};
use std::net::IpAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Instant, timeout, timeout_at};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
const HOP_BY_HOP: &[&str] = &["connection", "keep-alive", "proxy-connection", "transfer-encoding", "content-length", "te", "trailer", "upgrade"];

pub async fn forward(upstream: &str, req: &Request, peer: IpAddr) -> Response {
    match exchange(upstream, req, peer).await {
        Ok(resp) => resp,
        Err(status) => {
            let mut resp = Response::new(status, status.reason().as_bytes().to_vec());
            resp.set_header("Content-Type", "text/plain");
            resp
        }
    }
}

async fn exchange(upstream: &str, req: &Request, peer: IpAddr) -> Result<Response, Status> {
    let mut tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect(upstream)).await
        .map_err(|_| Status::GatewayTimeout)?
        .map_err(|_| Status::BadGateway)?;
    tcp.write_all(&http_request(req, peer)).await.map_err(|_| Status::BadGateway)?;
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    let cap = limits::MAX_HEADERS + limits::MAX_BODY;
    let mut buf = Vec::with_capacity(16 * 1024);
    let mut tmp = [0u8; 16 * 1024];
    loop {
        let n = timeout_at(deadline, tcp.read(&mut tmp)).await
            .map_err(|_| Status::GatewayTimeout)?
            .map_err(|_| Status::BadGateway)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > cap {
            return Err(Status::BadGateway);
        }
    }
    parse_http_response(&buf, req.method == "HEAD", upstream).ok_or(Status::BadGateway)
}

fn http_request(req: &Request, peer: IpAddr) -> Vec<u8> {
    let mut out = format!("{} {} HTTP/1.1{CRLF}", req.method, req.path);
    let mut forwarded_for = peer.to_string();
    for (name, value) in &req.headers {
        let lower = name.to_ascii_lowercase();
        if HOP_BY_HOP.contains(&lower.as_str()) {
            continue;
        }
        if lower == "x-forwarded-for" {
            forwarded_for = format!("{value}, {peer}");
            continue;
        }
        out.push_str(&format!("{name}: {value}{CRLF}"));
    }
    if req.get_header("Host").is_none() {
        out.push_str(&format!("Host: localhost{CRLF}"));
    }
    out.push_str(&format!("X-Forwarded-For: {forwarded_for}{CRLF}"));
    out.push_str(&format!("X-Forwarded-Proto: sonion{CRLF}"));
    out.push_str(&format!("Content-Length: {}{CRLF}", req.body.len()));
    out.push_str(&format!("Connection: close{CRLF}{CRLF}"));
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&req.body);
    bytes
}

fn parse_http_response(buf: &[u8], head_only: bool, upstream: &str) -> Option<Response> {
    let header_end = find_header_end(buf)?;
    let head = std::str::from_utf8(&buf[..header_end]).ok()?;
    let mut lines = head.split(CRLF);
    let mut parts = lines.next()?.splitn(3, ' ');
    if !parts.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let code: u16 = parts.next()?.parse().ok()?;
    let headers = parse_headers(lines).ok()?;
    let header = |name: &str| headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
    let raw = &buf[header_end + 4..];
    let body = if head_only || matches!(code, 100..=199 | 204 | 304) {
        Vec::new()
    } else if header("Transfer-Encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        decode_chunked(raw)?
    } else if let Some(len) = header("Content-Length") {
        raw.get(..len.trim().parse::<usize>().ok()?)?.to_vec()
    } else {
        raw.to_vec()
    };
    let upstream_len = header("Content-Length").map(str::to_string);
    let mut resp = Response::new(Status::nearest(code), body);
    for (name, value) in &headers {
        let lower = name.to_ascii_lowercase();
        if HOP_BY_HOP.contains(&lower.as_str()) {
            continue;
        }
        let value = if lower == "location" { same_origin_path(value, upstream) } else { value.clone() };
        resp.headers.push((name.clone(), value));
    }
    if head_only {
        if let Some(len) = upstream_len {
            resp.set_header("Content-Length", len);
        }
    }
    Some(resp)
}

fn same_origin_path(location: &str, upstream: &str) -> String {
    location.strip_prefix("http://")
        .and_then(|rest| rest.strip_prefix(upstream))
        .filter(|path| path.is_empty() || path.starts_with('/'))
        .map(|path| if path.is_empty() { "/".to_string() } else { path.to_string() })
        .unwrap_or_else(|| location.to_string())
}

fn decode_chunked(mut rest: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = rest.windows(2).position(|w| w == b"\r\n")?;
        let size_line = std::str::from_utf8(&rest[..line_end]).ok()?;
        let size = usize::from_str_radix(size_line.split(';').next()?.trim(), 16).ok()?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            return Some(body);
        }
        if body.len() + size > limits::MAX_BODY {
            return None;
        }
        body.extend_from_slice(rest.get(..size)?);
        rest = rest.get(size + 2..)?;
    }
}