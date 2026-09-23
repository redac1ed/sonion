use crate::{ProtocolError, DEFAULT_PORT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SonionUrl {
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Default for SonionUrl {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: DEFAULT_PORT,
            path: "/".into(),
        }
    }
}

impl SonionUrl {
    pub fn parse(input: &str) -> Result<Self, ProtocolError> {
        let rest = input
            .strip_prefix("sonion://")
            .ok_or_else(|| ProtocolError::InvalidUrl(format!("missing scheme: {input}")))?;
        let (authority, path) = match rest.find('/') {
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
        let path = canonicalize_path(&decode_path(path)?)?; // no ipv6
        Ok(Self { host, port, path })
    }
    pub fn encoded_path(&self) -> String {
        encode_path(&self.path)
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

pub fn canonicalize_path(decoded: &str) -> Result<String, ProtocolError> {
    if !decoded.starts_with('/') {
        return Err(ProtocolError::InvalidUrl(format!(
            "path must start with '/': {decoded}"
        )));
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
    fn explicit_port_and_path() {
        let u = SonionUrl::parse("sonion://localhost:8080/app.js").unwrap();
        assert_eq!(u.port, 8080);
        assert_eq!(u.path, "/app.js");
    }
    #[test]
    fn no_path_defaults_to_root() {
        assert_eq!(SonionUrl::parse("sonion://a.son").unwrap().path, "/");
    }
    #[test]
    fn rejects_wrong_scheme() {
        assert!(SonionUrl::parse("https://a.son/").is_err());
    }
    #[test]
    fn rejects_bad_port() {
        assert!(SonionUrl::parse("sonion://a.son:nope/").is_err());
        assert!(SonionUrl::parse("sonion://a.son:99999/").is_err());
    }
    #[test]
    fn decodes_percent_escapes() {
        let u = SonionUrl::parse("sonion://a.son/my%20file%2Etxt").unwrap();
        assert_eq!(u.path, "/my file.txt");
    }
    #[test]
    fn encoded_path_roundtrips() {
        let u = SonionUrl::parse("sonion://a.son/a%20b/c%25d").unwrap();
        assert_eq!(u.path, "/a b/c%d");
        assert_eq!(u.encoded_path(), "/a%20b/c%25d");
    }
    #[test]
    fn rejects_bad_escapes() {
        assert!(decode_path("/%").is_err());
        assert!(decode_path("/%2").is_err());
        assert!(decode_path("/%zz").is_err());
        assert!(decode_path("/%ff%ff").is_err());
    }
    #[test]
    fn resolves_dot_segments() {
        assert_eq!(canonicalize_path("/a/./b/../c").unwrap(), "/a/c");
        assert_eq!(canonicalize_path("/a/b/").unwrap(), "/a/b/");
        assert_eq!(canonicalize_path("/").unwrap(), "/");
    }
    #[test]
    fn rejects_traversal_above_root() {
        assert!(canonicalize_path("/../etc/passwd").is_err());
        assert!(canonicalize_path("/a/../../etc/passwd").is_err());
    }
    #[test]
    fn rejects_encoded_traversal() {
        assert!(SonionUrl::parse("sonion://a.son/%2e%2e%2fetc/passwd").is_err());
    }
    #[test]
    fn keeps_traversal_within_root() {
        assert_eq!(canonicalize_path("/a/b/../c").unwrap(), "/a/c");
    }
}
