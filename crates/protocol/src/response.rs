use crate::{
    head::{find_header_end, parse_headers, CRLF, VERSION},
    limits, ProtocolError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,                  // 200
    MovedPermanently,    // 301
    Found,               // 302
    BadRequest,          // 400
    NotFound,            // 404
    PayloadTooLarge,     // 413
    InternalServerError, // 500
}

impl Status {
    pub fn code(self) -> u16 {
        match self {
            Status::Ok => 200,
            Status::MovedPermanently => 301,
            Status::Found => 302,
            Status::BadRequest => 400,
            Status::NotFound => 404,
            Status::PayloadTooLarge => 413,
            Status::InternalServerError => 500,
        }
    }
    pub fn from_code(code: u16) -> Result<Status, ProtocolError> {
        match code {
            200 => Ok(Status::Ok),
            301 => Ok(Status::MovedPermanently),
            302 => Ok(Status::Found),
            400 => Ok(Status::BadRequest),
            404 => Ok(Status::NotFound),
            413 => Ok(Status::PayloadTooLarge),
            500 => Ok(Status::InternalServerError),
            c => Err(ProtocolError::InvalidStatus(c)),
        }
    }
    pub fn reason(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::MovedPermanently => "moved permanently",
            Status::Found => "found",
            Status::BadRequest => "bad request",
            Status::NotFound => "not found",
            Status::PayloadTooLarge => "payload too large",
            Status::InternalServerError => "internal server error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: Status,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: Status, body: impl Into<Vec<u8>>) -> Self {
        let body = body.into();
        Self {
            status,
            headers: vec![("Content-Length".into(), body.len().to_string())],
            body,
        }
    }
    pub fn set_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
        self.headers.push((name, value.into()));
    }
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn is_chunked(&self) -> bool {
        self.get_header("Transfer-Encoding")
            .map(|v| v.eq_ignore_ascii_case("chunked"))
            .unwrap_or(false)
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = self.serialize_head();
        bytes.extend_from_slice(&self.body);
        bytes
    }
    pub fn serialize_head(&self) -> Vec<u8> {
        let mut out = format!(
            "SONION/1.0 {} {}{}",
            self.status.code(),
            self.status.reason(),
            CRLF
        );
        for (name, value) in &self.headers {
            out.push_str(&format!("{name}: {value}{CRLF}"));
        }
        out.push_str(CRLF);
        out.into_bytes()
    }
    pub fn serialize_chunked(&self, chunk_size: usize) -> Vec<u8> {
        let mut out = format!(
            "SONION/1.0 {} {}{}",
            self.status.code(),
            self.status.reason(),
            CRLF
        );
        for (name, value) in &self.headers {
            if name.eq_ignore_ascii_case("Content-Length")
                || name.eq_ignore_ascii_case("Transfer-Encoding")
            {
                continue;
            }
            out.push_str(&format!("{name}: {value}{CRLF}"));
        }
        out.push_str(&format!("Transfer-Encoding: chunked{CRLF}{CRLF}"));
        let mut bytes = out.into_bytes();
        let chunk_size = chunk_size.max(1);
        for chunk in self.body.chunks(chunk_size) {
            bytes.extend_from_slice(format!("{:x}{CRLF}", chunk.len()).as_bytes());
            bytes.extend_from_slice(chunk);
            bytes.extend_from_slice(CRLF.as_bytes());
        }
        bytes.extend_from_slice(format!("0{CRLF}{CRLF}").as_bytes());
        bytes
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let header_end = find_header_end(bytes).ok_or(ProtocolError::UnexpectedEof)?;
        if header_end > limits::MAX_STATUS_LINE + limits::MAX_HEADERS {
            return Err(ProtocolError::HeadersTooLarge {
                max: limits::MAX_HEADERS,
            });
        }
        let head = std::str::from_utf8(&bytes[..header_end])
            .map_err(|_| ProtocolError::MalformedLine("non-utf8 head".into()))?;
        let mut lines = head.split(CRLF);
        let status_line = lines.next().ok_or(ProtocolError::UnexpectedEof)?;
        let mut parts = status_line.splitn(3, ' ');
        let version = parts.next().unwrap_or("");
        if version != VERSION {
            return Err(ProtocolError::MalformedLine(format!(
                "bad version: {status_line}"
            )));
        }
        let code: u16 = parts
            .next()
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| ProtocolError::MalformedLine(status_line.into()))?;
        let status = Status::from_code(code)?;
        let headers = parse_headers(lines)?;
        let body_bytes = &bytes[header_end + 4..]; // skip CRLFCRLF
        let mut resp = Self {
            status,
            headers,
            body: Vec::new(),
        };
        if resp.is_chunked() {
            resp.body = parse_chunked_body(body_bytes)?;
        } else if let Some(len) = resp.get_header("Content-Length") {
            let len: usize = len
                .parse()
                .map_err(|_| ProtocolError::InvalidHeader("Content-Length".into()))?;
            if len > limits::MAX_BODY {
                return Err(ProtocolError::BodyTooLarge {
                    max: limits::MAX_BODY,
                });
            }
            if body_bytes.len() < len {
                return Err(ProtocolError::UnexpectedEof);
            }
            resp.body = body_bytes[..len].to_vec();
        }
        Ok(resp)
    }
    pub fn parse_head(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let header_end = find_header_end(bytes).ok_or(ProtocolError::UnexpectedEof)?;
        if header_end > limits::MAX_STATUS_LINE + limits::MAX_HEADERS {
            return Err(ProtocolError::HeadersTooLarge {
                max: limits::MAX_HEADERS,
            });
        }
        let head = std::str::from_utf8(&bytes[..header_end])
            .map_err(|_| ProtocolError::MalformedLine("non-utf8 head".into()))?;
        let mut lines = head.split(CRLF);
        let status_line = lines.next().ok_or(ProtocolError::UnexpectedEof)?;
        let mut parts = status_line.splitn(3, ' ');
        let version = parts.next().unwrap_or("");
        if version != VERSION {
            return Err(ProtocolError::MalformedLine(format!(
                "bad version: {status_line}"
            )));
        }
        let code: u16 = parts
            .next()
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| ProtocolError::MalformedLine(status_line.into()))?;
        let status = Status::from_code(code)?;
        let headers = parse_headers(lines)?;
        Ok(Self {
            status,
            headers,
            body: Vec::new(),
        })
    }
}

fn parse_chunked_body(bytes: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut body = Vec::new();
    let mut rest = bytes;
    loop {
        let line_end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or(ProtocolError::UnexpectedEof)?;
        let size_str = std::str::from_utf8(&rest[..line_end])
            .map_err(|_| ProtocolError::InvalidChunk("non-utf8 size".into()))?;
        let size = usize::from_str_radix(size_str.trim(), 16)
            .map_err(|_| ProtocolError::InvalidChunk(size_str.into()))?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            if rest.len() < 2 || &rest[..2] != b"\r\n" {
                return Err(ProtocolError::UnexpectedEof);
            }
            return Ok(body);
        }
        if body.len() + size > limits::MAX_BODY {
            return Err(ProtocolError::BodyTooLarge {
                max: limits::MAX_BODY,
            });
        }
        if rest.len() < size + 2 {
            return Err(ProtocolError::UnexpectedEof);
        }
        body.extend_from_slice(&rest[..size]);
        if &rest[size..size + 2] != b"\r\n" {
            return Err(ProtocolError::InvalidChunk(
                "missing CRLF after chunk".into(),
            ));
        }
        rest = &rest[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const GOLDEN_200: &[u8] = include_bytes!("../../../tests/golden/response-200.txt");
    const GOLDEN_404: &[u8] = include_bytes!("../../../tests/golden/response-404.txt");
    const GOLDEN_CHUNKED: &[u8] = include_bytes!("../../../tests/golden/response-chunked.txt");
    #[test]
    fn parse_golden_200() {
        let resp = Response::parse(GOLDEN_200).unwrap();
        assert_eq!(resp.status, Status::Ok);
        assert_eq!(resp.get_header("Content-Type"), Some("text/html"));
        assert_eq!(resp.body, b"konichiwa son");
    }
    #[test]
    fn serialize_200_matches_golden() {
        let mut resp = Response::new(Status::Ok, b"konichiwa son".to_vec());
        resp.set_header("Content-Type", "text/html"); // set_header appended to content type cus of the golden thing
        resp.headers = vec![
            ("Content-Type".into(), "text/html".into()),
            ("Content-Length".into(), "13".into()),
        ];
        assert_eq!(resp.serialize(), GOLDEN_200);
    }
    #[test]
    fn parse_golden_404() {
        let resp = Response::parse(GOLDEN_404).unwrap();
        assert_eq!(resp.status, Status::NotFound);
        assert_eq!(resp.body, b"idk whatchu talkin abt son");
    }
    #[test]
    fn serialize_404_matches_golden() {
        let mut resp = Response::new(Status::NotFound, b"idk whatchu talkin abt son".to_vec());
        resp.headers = vec![("Content-Length".into(), "26".into())];
        assert_eq!(resp.serialize(), GOLDEN_404);
    }
    #[test]
    fn roundtrip_chunked() {
        let body = b"konichiwa son".to_vec();
        let mut resp = Response::new(Status::Ok, body.clone());
        resp.set_header("Content-Type", "text/html");
        let bytes = resp.serialize_chunked(4);
        let parsed = Response::parse(&bytes).unwrap();
        assert!(parsed.is_chunked());
        assert_eq!(parsed.body, body);
    }
    #[test]
    fn parse_golden_chunked() {
        let resp = Response::parse(GOLDEN_CHUNKED).unwrap();
        assert_eq!(resp.status, Status::Ok);
        assert_eq!(resp.body, b"konichiwa son");
    }
    #[test]
    fn rejects_bad_version() {
        assert!(Response::parse(b"HTTP/1.1 200 OK\r\n\r\n").is_err());
    }
    #[test]
    fn rejects_unknown_status() {
        assert!(matches!(
            Response::parse(b"SONION/1.0 418 teapot\r\n\r\n"),
            Err(ProtocolError::InvalidStatus(418))
        ));
    }
    #[test]
    fn rejects_truncated_body() {
        assert!(matches!(
            Response::parse(b"SONION/1.0 200 ok\r\nContent-Length: 100\r\n\r\nshort"),
            Err(ProtocolError::UnexpectedEof)
        ));
    }
    #[test]
    fn chunked_encode_single_chunk() {
        let resp = Response::new(Status::Ok, b"hello".to_vec());
        let wire = resp.serialize_chunked(1024);
        let wire_str = String::from_utf8(wire).unwrap();
        assert!(wire_str.contains("Transfer-Encoding: chunked"));
        assert!(!wire_str.contains("Content-Length"));
        assert!(wire_str.contains("5\r\nhello\r\n"));
        assert!(wire_str.ends_with("0\r\n\r\n"));
    }
    #[test]
    fn chunked_encode_multiple_chunks() {
        let resp = Response::new(Status::Ok, b"abcdefgh".to_vec());
        let wire = resp.serialize_chunked(3);
        let wire_str = String::from_utf8(wire).unwrap();
        assert!(wire_str.contains("3\r\nabc\r\n"));
        assert!(wire_str.contains("3\r\ndef\r\n"));
        assert!(wire_str.contains("2\r\ngh\r\n"));
        assert!(wire_str.ends_with("0\r\n\r\n"));
    }
    #[test]
    fn chunked_encode_empty_body() {
        let resp = Response::new(Status::Ok, Vec::new());
        let wire = resp.serialize_chunked(64);
        let wire_str = String::from_utf8(wire).unwrap();
        assert!(wire_str.ends_with("0\r\n\r\n"));
    }
    #[test]
    fn chunked_roundtrip() {
        let original = Response::new(Status::Ok, b"the quick brown fox".to_vec());
        let wire = original.serialize_chunked(4);
        let parsed = Response::parse(&wire).expect("should parse");
        assert_eq!(parsed.status, Status::Ok);
        assert!(parsed.is_chunked());
        assert_eq!(parsed.body, b"the quick brown fox");
    }
    #[test]
    fn parse_rejects_oversized_body() {
        let mut resp = format!(
            "SONION/1.0 200 ok\r\nContent-Length: {}\r\n\r\n",
            limits::MAX_BODY + 1
        );
        resp.push_str(&"x".repeat(100));
        assert!(Response::parse(resp.as_bytes()).is_err());
    }
    #[test]
    fn parse_rejects_bad_chunk_size() {
        let wire =
            b"SONION/1.0 200 ok\r\nTransfer-Encoding: chunked\r\n\r\nZZ\r\nhello\r\n0\r\n\r\n";
        assert!(Response::parse(wire).is_err());
    }
    #[test]
    fn parse_rejects_truncated_chunk() {
        let wire = b"SONION/1.0 200 ok\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhel";
        assert!(Response::parse(wire).is_err());
    }
    #[test]
    fn binary_body_roundtrip() {
        let body: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let mut resp = Response::new(Status::Ok, body.clone());
        resp.set_header("Content-Type", "image/png");
        assert_eq!(Response::parse(&resp.serialize()).unwrap().body, body);
        assert_eq!(
            Response::parse(&resp.serialize_chunked(777)).unwrap().body,
            body
        );
    }
    #[test]
    fn parse_head_response_ignores_content_length() {
        let wire = b"SONION/1.0 200 ok\r\nContent-Type: text/html\r\nContent-Length: 13\r\n\r\n";
        let resp = Response::parse_head(wire).unwrap();
        assert_eq!(resp.status, Status::Ok);
        assert_eq!(resp.get_header("Content-Length"), Some("13"));
        assert!(resp.body.is_empty());
    }
}
