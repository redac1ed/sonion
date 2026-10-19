use registry::client::SonionResolver;
use sonion_protocol::{Client, Request, Response, SonionUrl};
use std::sync::Arc;

pub struct NetStack {
    resolver: SonionResolver,
    client: Client,
    insecure_dev: bool, 
}

pub struct FetchedPage {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("bad url: {0}")]
    BadUrl(String),
    #[error("resolve failed: {0}")]
    Resolve(#[from] registry::client::ResolveError),
    #[error("request failed: {0}")]
    Request(#[from] sonion_protocol::ClientError)
}

impl NetStack {
    pub fn new(registry: Option<SonionUrl>, pin_key: Option<String>, insecure_dev: bool) -> Self {
        let resolver = match registry {
            Some(base) => SonionResolver::new(base, insecure_dev, pin_key), 
            None => SonionResolver::new(
                SonionUrl {
                    host: "127.0.0.1".into(),
                    port: 6768,
                    path: "/".into(),
                    query: None
                }, 
                insecure_dev, 
                pin_key
            ),
        };
        Self {
            resolver, 
            client: Client::new(insecure_dev),
            insecure_dev
        }
    }
    pub async fn fetch_page(&self, url: &str) -> Result<FetchedPage, FetchError> {
        let parsed = SonionUrl::parse(url).map_err(|e| FetchError::BadUrl(e.to_string()))?;
        let is_name = is_domain_name(&parsed.host);
        let resp = if is_name {
            let rec = self.resolver.resolve(&parsed.host).await?;
            let mut last = None;
            let mut out = None;
            for ep in &rec.endpoints {
                let Some(target) = registry::client::endpoint_url(ep, &parsed.encoded_path()) else { continue; };
                let req = Request::get(&target.encoded_path()).header("Host", &parsed.host);
                match self.client.send(&target, &req).await {
                    Ok(r) => {
                        out = Some(r);
                        break;
                    }
                    Err(e) => last = Some(e)
                }
            }
            match out {
                Some(r) => r,
                None => {
                    return Err(FetchError::Request(
                        last.unwrap_or(sonion_protocol::ClientError::Timeout("all endpoints"))
                    ));
                }
            }
        } else {
            let req = Request::get(&parsed.encoded_path()).header("Host", &parsed.host);
            self.client.send(&parsed, &req).await?
        };
        Ok(page_from_response(resp))
    }
    pub fn resolver(&self) -> &SonionResolver {
        &self.resolver
    }
}

pub fn is_domain_name(host: &str) -> bool {
    !host.is_empty()
        && host != "localhost"
        && host.parse::<std::net::IpAddr>().is_err()
        && host.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
}

fn page_from_response(resp: Response) -> FetchedPage {
    let content_type = resp.get_header("Content-Type").unwrap_or("application/octet-stream").to_string();
    FetchedPage {
        status: resp.status.code(),
        content_type,
        body: resp.body
    }
}

pub type SharedNet = Arc<NetStack>;