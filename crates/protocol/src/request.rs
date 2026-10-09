use crate::{
    head::{find_header_end, parse_headers, CRLF, VERSION},
    limits, ProtocolError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>
}

impl Request {
    pub fn get(path: &str) -> Self {
        Self {
            method: "GET".into(),
            path: path.into(),
            headers: Vec::new(),
            body: Vec::new()
        }
    }
    pub fn head(path: &str) -> Self {
        Self {
            method: "HEAD".into(),
            path: path.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
    pub fn post(path: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            method: "POST".into(),
            path:path.into(), 
            headers: Vec::new(),
            body: body.into()
        }
    }
    pub fn put(path: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            method: "PUT".into(),
            path: path.into(),
            headers: Vec::new(),
            body: body.into()
        }
    }
    pub fn delete(path: &str) -> Self {
        Self {
            method: "DELETE".into(),
            path: path.into(),
            headers: Vec::new(),
            body: Vec::new()
        }
    }
    pub fn header(mut self, name: &str, value: &str) -> Self {
        assert!(
            !name.bytes().any(|b| b == b'\r' || b == b'\n' || b == b':'),
            "header name must not contain CR, LF or ':'"
        );
        assert!(
            !value.bytes().any(|b| b == b'\r' || b == b'\n'),
            "header value must not contain CR or LF"
        );
        self.headers.push((name.into(), value.into()));
        self
    }
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = format!("{} {} {VERSION}{CRLF}", self.method, self.path);
        let has_cl = self.headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("Content-Length"));
        for (name, value) in &self.headers {
            out.push_str(&format!("{name}: {value}{CRLF}"));
        }
        if !self.body.is_empty() && !has_cl {
            out.push_str(&format!("Content-Length: {}{CRLF}", self.body.len()));
        }
        out.push_str(CRLF);
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let header_end = find_header_end(bytes).ok_or(ProtocolError::UnexpectedEof)?;
        if header_end > limits::MAX_REQUEST_LINE + limits::MAX_HEADERS {
            return Err(ProtocolError::HeadersTooLarge {
                max: limits::MAX_HEADERS,
            });
        }
        let head = std::str::from_utf8(&bytes[..header_end]).map_err(|_| ProtocolError::MalformedLine("non-utf8 head".into()))?;
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
            "GET" | "HEAD" | "POST" | "PUT" | "DELETE" => {}
            other => return Err(ProtocolError::MalformedLine(format!("bad method: {other}"))),
        }
        if !path.starts_with('/') {
            return Err(ProtocolError::MalformedLine(format!(
                "path must start with '/': {path}"
            )));
        }
        let headers = parse_headers(lines)?;
        let mut req = Self {
            method: method.into(),
            path: path.into(), 
            headers,
            body: Vec::new()
        };
        if let Some(len) = req.get_header("Content-Length") {
            let len: usize = len.trim().parse().map_err(|_| ProtocolError::InvalidHeader("Content-Length".into()))?;
            if len > limits::MAX_BODY {
                return Err(ProtocolError::BodyTooLarge {
                    max: limits::MAX_BODY,
                });
            }
            let body_bytes = &bytes[header_end + 4..];
            if body_bytes.len() < len {
                return Err(ProtocolError::UnexpectedEof);
            }
            req.body = body_bytes[..len].to_vec();
        }
        Ok(req)
    }
}
