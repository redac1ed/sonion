use crate::{limits, ProtocolError};

const CRLF: &str = "\r\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>
}

impl Request {
    pub fn get(path: &str) -> Self {
        Self { method: "GET".into(), path: path.into(), headers: Vec::new() }
    }
    pub fn head(path: &str) -> Self {
        Self { method: "HEAD".into(), path: path.into(), headers: Vec::new() }
    }
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = format!("{} {} SONION/1.0{}", self.method, self.path, CRLF);
        for (name, value) in &self.headers {
            out.push_str(&format!("{name}: {value}{CRLF}"));
        }
        out.push_str(CRLF);
        out.into_bytes()
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let header_end = find_header_end(bytes).ok_or(ProtocolError::UnexpectedEof)?;
        if header_end > limits::MAX_REQUEST_LINE + limits::MAX_HEADERS {
            return Err(ProtocolError::HeadersTooLarge { max: limits::MAX_HEADERS });
        }
        let head = std::str::from_utf8(&bytes[..header_end])
            .map_err(|_| ProtocolError::MalformedLine("non-utf8 head".into()))?;
        let mut lines = head.split(CRLF);
        let request_line = lines.next().ok_or(ProtocolError::UnexpectedEof)?;
        if request_line.len() > limits::MAX_REQUEST_LINE {
            return Err(ProtocolError::RequestLineTooLarge { max: limits::MAX_REQUEST_LINE });
        }
        let mut parts = request_line.split_whitespace();
        let method = parts.next().ok_or_else(|| ProtocolError::MalformedLine(request_line.into()))?;
        let path = parts.next().ok_or_else(|| ProtocolError::MalformedLine(request_line.into()))?;
        let version = parts.next().ok_or_else(|| ProtocolError::MalformedLine(request_line.into()))?;
        if version != "SONION/1.0" {
            return Err(ProtocolError::MalformedLine(format!("bad version: {version}")));
        }
        match method {
            "GET" | "HEAD" => {}
            other => return Err(ProtocolError::MalformedLine(format!("bad method: {other}"))),
        }
        let headers = parse_headers(lines)?;
        Ok(Self { method: method.into(), path: path.into(), headers })
    }
}

pub(crate) fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
}

pub(crate) fn parse_headers<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> Result<Vec<(String, String)>, ProtocolError> {
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ProtocolError::InvalidHeader(line.into()))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(ProtocolError::InvalidHeader(line.into()));
        }
        headers.push((name.to_string(), value.trim().to_string()));
    }
    Ok(headers)
}