use crate::{ProtocolError, DEFAULT_PORT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SonionUrl {
    pub host: String,
    pub port: u16,
    pub path: String, 
}

impl Default for SonionUrl {
    fn default() -> Self {
        Self { host: String::new(), port: DEFAULT_PORT, path: "/".into() }
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
        };
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
        Ok(Self { host, port, path: path.to_string() })
    }
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
}