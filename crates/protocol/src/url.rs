use crate::{ProtocolError, DEFAULT_PORT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SonionUrl {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub query: Option<String>
}

impl Default for SonionUrl {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: DEFAULT_PORT,
            path: "/".into(),
            query: None
        }
    }
}

impl SonionUrl {
    pub fn parse(input: &str) -> Result<Self, ProtocolError> {
        let rest = input
            .strip_prefix("sonion://")
            .ok_or_else(|| ProtocolError::InvalidUrl(format!("missing scheme: {input}")))?;
        let (authority, path_and_query) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        if authority.is_empty() {
            return Err(ProtocolError::InvalidUrl("empty host".into()));
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => {
                let port: u16 = p
                    .parse()
                    .map_err(|_| ProtocolError::InvalidUrl(format!("bad port: {p}")))?;
                (h.to_string(), port)
            }
            None => (authority.to_string(), DEFAULT_PORT),
        };
        if host.is_empty() {
            return Err(ProtocolError::InvalidUrl("empty host".into()));
        }
        let (raw_path, raw_query) = match path_and_query.find('?') {
            Some(i) => (
                &path_and_query[..i],
                Some(&path_and_query[i + 1..])
            ),
            None => (path_and_query, None)
        };
        let path = canonicalize_path(&decode_path(raw_path)?)?; 
        let query = match raw_query {
            Some(q) => {
                let decoded = decode_path(q)?;
                if decoded.bytes().any(|b| b < 0x20 || b ==0x7f) {
                    return Err(ProtocolError::InvalidUrl("control char in query".into()));
                }
                Some(decoded)
            }
            None => None
        };
        Ok(Self { host, port, path, query })
    }
    pub fn encoded_path(&self) -> String {
        let mut out = encode_path(&self.path);
        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(&encode_query(q));
        }
        out
    }
    pub fn authority(&self) -> String {
        if self.port == DEFAULT_PORT {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

impl std::fmt::Display for SonionUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sonion://{}{}", self.authority(), self.encoded_path())
    }
}

pub fn decode_path(s: &str) -> Result<String, ProtocolError> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err(ProtocolError::InvalidUrl(format!(
                        "truncated % escape in: {s}"
                    )));
                }
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .map_err(|_| ProtocolError::InvalidUrl(format!("bad % escape in: {s}")))?;
                let byte = u8::from_str_radix(hex, 16).map_err(|_| {
                    ProtocolError::InvalidUrl(format!("bad % escape %{hex} in: {s}"))
                })?;
                out.push(byte);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ProtocolError::InvalidUrl(format!("non-utf8 path: {s}")))
}

pub fn encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/'
            | b'=' | b'&' | b'+' | b':' | b'@' | b',' | b';' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn parse_query(query: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for part in query.split('&') {
        if part.is_empty() {
            continue;
        }
        match part.split_once('=') {
            Some((k, v)) => {
                let k = decode_path(k).unwrap_or_else(|_| k.to_string());
                let v = decode_path(v).unwrap_or_else(|_| v.to_string());
                pairs.push((k, v));
            }
            None => {
                pairs.push((decode_path(part).unwrap_or_else(|_| part.to_string()), String::new()));
            }
        }
    }
    pairs
}

pub fn canonicalize_path(decoded: &str) -> Result<String, ProtocolError> {
    if !decoded.starts_with('/') {
        return Err(ProtocolError::InvalidUrl(format!(
            "path must start with '/': {decoded}"
        )));
    }
    if decoded.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(ProtocolError::InvalidUrl(format!("control char in path: {decoded:?}")));
    }
    if decoded.contains('\\') {
        return Err(ProtocolError::InvalidUrl(format!("backslash not allowed in path: {decoded}")));
    }
    let mut segments: Vec<&str> = Vec::new();
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(ProtocolError::InvalidUrl(format!(
                        "path escapes root: {decoded}"
                    )));
                }
            }
            s => segments.push(s),
        }
    }
    let mut out = String::from("/");
    out.push_str(&segments.join("/"));
    if decoded.ends_with('/') && !out.ends_with('/') {
        out.push('/');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_port_is_6767() {
        let u = SonionUrl::parse("sonion://hello.son/").unwrap();
        assert_eq!(u.port, 6767);
        assert_eq!(u.host, "hello.son");
        assert_eq!(u.path, "/");
    }
    #[test]
    fn parses_host_and_port() {
        let u = SonionUrl::parse("sonion://example.son:1234/a/b").unwrap();
        assert_eq!(u.host, "example.son");
        assert_eq!(u.port, 1234);
        assert_eq!(u.path, "/a/b");
        assert_eq!(u.query, None);
    }
    #[test]
    fn parses_query() {
        let u = SonionUrl::parse("sonion://h/search?q=h").unwrap();
        assert_eq!(u.path, "/search");
        assert_eq!(u.query.as_deref(), Some("q=h"));
        assert_eq!(u.encoded_path(), "/search?q=h")   
    }
    #[test]
    fn query_pairs_roundtrip() {
        let pairs = parse_query("a=2&j=23%2039");
        assert_eq!(
            pairs,
            vec![
                ("a".to_string(), "2".to_string()),
                ("j".to_string(), "23 39".to_string())
            ]
        );
    }
    #[test]
    fn display_roundtrip() {
        let s = "sonion://a.son:6767/a%20?x=1";
        let u = SonionUrl::parse(s).unwrap();
        assert_eq!(u.to_string(), s);
    }
    #[test]
    fn rejects_backslash() {
        assert!(SonionUrl::parse("sonion://h/a\\b").is_err());
        assert!(SonionUrl::parse("sonion://h/%5c%5c").is_err());
    }
    #[test]
    fn rejects_control_chars() {
        assert!(SonionUrl::parse("sonion://h/a%00b").is_err());
        assert!(SonionUrl::parse("sonion://h/a%0ab").is_err());
    }
    #[test]
    fn rejects_traversal_past_root() {
        assert!(SonionUrl::parse("sonion://h/../../etc").is_err());
    }
    #[test]
    fn canonicalizes_dot_segments() {
        assert_eq!(SonionUrl::parse("sonion://h/a/.b/../c").unwrap().path, "/a/c");
    }
}
