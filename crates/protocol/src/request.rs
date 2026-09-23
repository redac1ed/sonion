use crate::{
    head::{find_header_end, parse_headers, CRLF, VERSION},
    limits, ProtocolError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
}

impl Request {
    pub fn get(path: &str) -> Self {
        Self {
            method: "GET".into(),
            path: path.into(),
            headers: Vec::new(),
        }
    }
    pub fn head(path: &str) -> Self {
        Self {
            method: "HEAD".into(),
            path: path.into(),
            headers: Vec::new(),
        }
    }
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = format!("{} {} {VERSION}{CRLF}", self.method, self.path);
        for (name, value) in &self.headers {
            out.push_str(&format!("{name}: {value}{CRLF}"));
        }
        out.push_str(CRLF);
        out.into_bytes()
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let header_end = find_header_end(bytes).ok_or(ProtocolError::UnexpectedEof)?;
        if header_end > limits::MAX_REQUEST_LINE + limits::MAX_HEADERS {
            return Err(ProtocolError::HeadersTooLarge {
                max: limits::MAX_HEADERS,
            });
        }
        let head = std::str::from_utf8(&bytes[..header_end])
            .map_err(|_| ProtocolError::MalformedLine("non-utf8 head".into()))?;
        let mut lines = head.split(CRLF);
        let request_line = lines.next().ok_or(ProtocolError::UnexpectedEof)?;
        if request_line.len() > limits::MAX_REQUEST_LINE {
            return Err(ProtocolError::RequestLineTooLarge {
                max: limits::MAX_REQUEST_LINE,
            });
        }
        let mut parts = request_line.split_whitespace();
        let method = parts
            .next()
            .ok_or_else(|| ProtocolError::MalformedLine(request_line.into()))?;
        let path = parts
            .next()
            .ok_or_else(|| ProtocolError::MalformedLine(request_line.into()))?;
        let version = parts
            .next()
            .ok_or_else(|| ProtocolError::MalformedLine(request_line.into()))?;
        if version != VERSION {
            return Err(ProtocolError::MalformedLine(format!(
                "bad version: {version}"
            )));
        }
        match method {
            "GET" | "HEAD" => {}
            other => return Err(ProtocolError::MalformedLine(format!("bad method: {other}"))),
        }
        if !path.starts_with('/') {
            return Err(ProtocolError::MalformedLine(format!(
                "path must start with '/': {path}"
            )));
        }
        let headers = parse_headers(lines)?;
        Ok(Self {
            method: method.into(),
            path: path.into(),
            headers,
        })
    }
}
