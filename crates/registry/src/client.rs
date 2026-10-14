use crate::{ResolveRecord, verify_record};
use sha2::{Digest, Sha256};
use sonion_protocol::{Request, Response, SonionUrl, Status, send};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("client error: {0}")]
    Client(#[from] sonion_protocol::ClientError),
    #[error("registry returned {0}: {1}")]
    Registry(u16, String), 
    #[error("record failed verification")]
    BadSignature,
    #[error("record expired or not yet valid")]
    BadValidity,
    #[error("bad json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no endpoints for {0}")]
    NoEndpoints(String)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub struct RegistryClient {
    base: SonionUrl,
    insecure_dev: bool
}

impl RegistryClient {
    pub fn new(base: SonionUrl, insecure_dev: bool) -> Self {
        Self {base, insecure_dev}
    }
    async fn call(&self, req: &Request) -> Result<Response, ResolveError> {
        Ok(send(&self.base, req, self.insecure_dev).await?)
    }
    fn check(resp: Response) -> Result<Response, ResolveError> {
        if resp.status.is_success() {
            return Ok(resp);
        }
        let msg = serde_json::from_slice::<serde_json::Value>(&resp.body).ok()
            .and_then(|v| v["error"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| String::from_utf8_lossy(&resp.body).into_owned());
        Err(ResolveError::Registry(resp.status.code(), msg))
    }
    pub async fn create_account(&self, username: &str, password: &str) -> Result<String, ResolveError> {
        let body = serde_json::json!({"username": username, "password": password});
        let req = Request::post("/v1/accounts", body.to_string().into_bytes())
            .header("Content-Type", "application/json");
        let resp = Self::check(self.call(&req).await?)?;
        let v: serde_json::Value = serde_json::from_slice(&resp.body)?;
        Ok(v["api_key"].as_str().unwrap_or_default().to_string())
    }
    pub async fn logout(&self, api_key: &str) -> Result<(), ResolveError> {
        let req = Request::post("/v1/logout", Vec::new())
            .header("Authorization", &format!("Bearer {api_key}"));
        Self::check(self.call(&req).await?)?;
        Ok(())
    }
    pub async fn login(&self, username: &str, password: &str) -> Result<String, ResolveError> {
        let body = serde_json::json!({"username": username, "password": password});
        let req = Request::post("/v1/login", body.to_string().into_bytes())
            .header("Content-Type", "application/json");
        let resp = Self::check(self.call(&req).await?)?;
        let v: serde_json::Value = serde_json::from_slice(&resp.body)?;
        Ok(v["api_key"].as_str().unwrap_or_default().to_string())
    }
    pub async fn put_domain(&self, api_key: &str, name: &str, endpoints: &[String], ttl: Option<u64>) -> Result<u64, ResolveError> {
        let mut body = serde_json::json!({"endpoints": endpoints});
        if let Some(t) = ttl {
            body["ttl"] = t.into();
        }
        let req = Request::put(&format!("/v1/domains/{name}"), body.to_string().into_bytes())
            .header("Authorization", &format!("Bearer {api_key}"))
            .header("Content-Type", "application/json");
        let resp = Self::check(self.call(&req).await?)?;
        let v: serde_json::Value = serde_json::from_slice(&resp.body)?;
        Ok(v["expires_at"].as_u64().unwrap_or(0))
    }
    pub async fn delete_domain(&self, api_key: &str, name: &str) -> Result<(), ResolveError> {
        let req = Request::delete(&format!("/v1/domains/{name}"))
            .header("Authorization", &format!("Bearer {api_key}"));
        Self::check(self.call(&req).await?)?;
        Ok(())
    }
    pub async fn list_domains(&self, api_key: &str) -> Result<Vec<DomainInfo>, ResolveError> {
        let req = Request::get("/v1/domains").header("Authorization", &format!("Bearer {api_key}"));
        let resp = Self::check(self.call(&req).await?)?;
        let v: serde_json::Value = serde_json::from_slice(&resp.body)?;
        let domains = v["domains"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|d| {
                Some(DomainInfo {
                    name: d["name"].as_str()?.to_string(),
                    endpoints: d["endpoints"].as_array()?.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string())).collect(),
                    expires_at: d["expires_at"].as_u64()?,
                    ttl: d["ttl"].as_u64()?,
                    expired: d["expired"].as_bool().unwrap_or(false)
                })
            }).collect();
        Ok(domains)
    }
    pub async fn resolve_record(&self, name: &str) -> Result<ResolveRecord, ResolveError> {
        let req = Request::get(&format!("/v1/resolve/{name}"));
        let resp = self.call(&req).await?;
        if resp.status == Status::NotFound {
            return Err(ResolveError::Registry(404, format!("no such domain: {name}")));
        }
        let resp = Self::check(resp)?;
        let rec: ResolveRecord = serde_json::from_slice(&resp.body)?;
        Ok(rec)
    }
    pub async fn verifying_key(&self) -> Result<String, ResolveError> {
        let req = Request::get("/v1/key");
        let resp = Self::check(self.call(&req).await?)?;
        let v: serde_json::Value = serde_json::from_slice(&resp.body)?;
        Ok(v["key"].as_str().unwrap_or_default().to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DomainInfo {
    pub name: String, 
    pub endpoints: Vec<String>,
    pub expires_at: u64, 
    pub ttl: u64,
    pub expired: bool
}

struct CacheEntry {
    record: ResolveRecord, 
    cached_at: Instant, 
}

pub struct SonionResolver {
    client: RegistryClient,
    trusted_key: Option<String>,
    cache: Mutex<HashMap<String, CacheEntry>>
}

impl SonionResolver {
    pub fn new(registry_base: SonionUrl, insecure_dev: bool, trusted_key: Option<String>) -> Self {
        Self {
            client: RegistryClient::new(registry_base, insecure_dev), 
            trusted_key,
            cache: Mutex::new(HashMap::new())
        }
    }
    pub fn client(&self) -> &RegistryClient {
        &self.client
    }
    pub async fn resolve(&self, name: &str) -> Result<ResolveRecord, ResolveError> {
        if let Some(rec) = self.cached(name) {
            return Ok(rec);
        }
        let rec = self.client.resolve_record(name).await?;
        self.validate(&rec)?;
        let ttl = rec.ttl.max(1);
        self.cache.lock().unwrap().insert(
            name.to_string(),
            CacheEntry {
                record: rec.clone(),
                cached_at: Instant::now()
            }
        );
        let _ = ttl;
        Ok(rec)
    }
    fn cached(&self, name: &str) -> Option<ResolveRecord> {
        let cache = self.cache.lock().unwrap();
        let entry = cache.get(name)?;
        let age = entry.cached_at.elapsed();
        if age > Duration::from_secs(entry.record.ttl) {
            return None;
        }
        if entry.record.expires_at <= now() {
            return None;
        }
        Some(entry.record.clone())
    }
    fn validate(&self, rec: &ResolveRecord) -> Result<(), ResolveError> {
        if let Some(trusted) = &self.trusted_key {
            if &rec.key != trusted {
                return Err(ResolveError::BadSignature);
            }
        }
        let at = now();
        if rec.expires_at <= at || rec.not_before > at {
            return Err(ResolveError::BadValidity);
        }
        if !verify_record(rec, at) {
            return Err(ResolveError::BadSignature);
        }
        Ok(())
    }
    pub fn invalidate(&self, name: &str) {
        self.cache.lock().unwrap().remove(name);
    }
    pub fn clear_expired(&self) -> usize {
        let mut cache = self.cache.lock().unwrap();
        let before = cache.len();
        let at = now();
        cache.retain(|_, e| {
            e.cached_at.elapsed() <= Duration::from_secs(e.record.ttl) && e.record.expires_at > at
        });
        before - cache.len()
    }
    pub fn cache_len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }
    pub async fn fetch(&self, name: &str, path: &str, insecure_dev: bool) -> Result<Response, ResolveError> {
        let rec = self.resolve(name).await?;
        if rec.endpoints.is_empty() {
            return Err(ResolveError::NoEndpoints(name.to_string()));
        }
        let mut last_err = None;
        for endpoint in &rec.endpoints {
            let url = match endpoint_url(endpoint, path) {
                Some(u) => u,
                None => continue
            };
            let req = Request::get(path).header("Host", name);
            match send(&url, &req, insecure_dev).await {
                Ok(resp) => return Ok(resp),
                Err(e) => last_err = Some(e)
            }
        }
        match last_err {
            Some(e) => Err(e.into()),
            None => Err(ResolveError::NoEndpoints(name.to_string())),
        }
    }
}

pub fn endpoint_url(endpoint: &str, path: &str) -> Option<SonionUrl> {
    let (host, port) = endpoint.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some(SonionUrl {
        host: host.to_string(),
        port, 
        path: path.to_string(),
        query: None
    })
}

pub fn cache_key_fingerprint(rec: &ResolveRecord) -> String {
    let mut h = Sha256::new();
    h.update(rec.name.as_bytes());
    for e in &rec.endpoints {
        h.update(e.as_bytes());
    }
    h.update(rec.expires_at.to_be_bytes());
    hex(&h.finalize()[..8])
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
