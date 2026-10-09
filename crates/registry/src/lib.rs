use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sonion_protocol::head::find_header_end;
use sonion_protocol::{ALPN, Request, Response, Status, limits};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Instant, timeout_at};
use tokio_rustls::TlsAcceptor;
use tracing::warn;

const HEAD_TIMEOUT: Duration = Duration::from_secs(15);
const BODY_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_TTL_SECS: u64 = 300;
const DOMAIN_TTL_SECS: u64 = 3600;
const MAX_TTL_SECS: u64 = 3600;
const LOGIN_MAX_FAILURES: u32 = 5;
const LOGIN_COOLDOWN_SECS: u64 = 60;
const MAX_PASSWORD_LEN: usize = 256;
const MAX_ENDPOINTS: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to load tls identity: {0}")]
    TlsIdentity(String),
    #[error("corrupt database: {0}")]
    CorruptDb(String)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    accounts: HashMap<String, Account>,
    api_keys: HashMap<String, String>, 
    domains: HashMap<String, Domain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Account {
    salt: String,
    password_hash: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Domain {
    owner: String,
    endpoints: Vec<String>,
    expires_at: u64,
    ttl: u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveRecord {
    pub name: String, 
    pub endpoints: Vec<String>,
    pub expires_at: u64,
    pub ttl: u64,
    pub key: String,
    pub signature: String,
    pub not_before: u64
}

fn canonical_resolve(name: &str, endpoints: &[String], expires_at: u64, ttl: u64, not_before: u64) -> Vec<u8> {
    format!("{name}|{}|{expires_at}|{not_before}|{ttl}", endpoints.join(",")).into_bytes()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hash_password(salt: &[u8], password: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt);
    h.update(password.as_bytes());
    hex(&h.finalize())
}

fn sha256_hex(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

fn b64_encode(b: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b)
}

fn b64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
}

pub struct Registry {
    dir: PathBuf, 
    store: Mutex<Store>,
    signing: SigningKey,
    login_failures: Mutex<HashMap<IpAddr, LoginPenalty>>
}

#[derive(Debug, Clone, Copy, Default)]
struct LoginPenalty {
    failures: u32, 
    locked_until: u64
}

impl Registry {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let dir = dir.as_ref().to_path_buf(); 
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("db.json");
        let store = match std::fs::read(&db_path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                RegistryError::CorruptDb(format!("{}: {e}", db_path.display()))
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store::default(),
            Err(e) => return Err(RegistryError::Io(e))
        };
        let key_path = dir.join("signing.key");
        let signing = match std::fs::read(&key_path) {
            Ok(bytes) if bytes.len() == 32 => {
                SigningKey::from_bytes(bytes.as_slice().try_into().unwrap())
            }
            Ok(_) => {
                return Err(RegistryError::CorruptDb(format!(
                    "{}: bad signing key length",
                    key_path.display()
                )))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut sk_bytes = [0u8; 32];
                rand::rngs::OsRng.fill_bytes(&mut sk_bytes);
                std::fs::write(&key_path, sk_bytes)?;
                SigningKey::from_bytes(&sk_bytes)
            }
            Err(e) => return Err(RegistryError::Io(e))
        };
        Ok(Self {
            dir, 
            store: Mutex::new(store),
            signing,
            login_failures: Mutex::new(HashMap::new())
        })
    }
    pub fn verifying_key_b64(&self) -> String {
        b64_encode(self.signing.verifying_key().as_bytes())
    }
    fn save(&self, store: &Store) -> Result<(), RegistryError> {
        let temp = self.dir.join("db.json.tmp");
        {
            let mut f = std::fs::File::create(&temp)?;
            use std::io::Write;
            f.write_all(&serde_json::to_vec_pretty(store)?)?;
            f.sync_all()?;
        }
        std::fs::rename(&temp, self.dir.join("db.json"))?;
        Ok(())
    }
    fn create_account(&self, body: &[u8]) -> (Status, String) {
        #[derive(Deserialize)]
        struct In {
            username: String, 
            password: String
        }
        let Ok(input) = serde_json::from_slice::<In>(body) else {
            return (Status::BadRequest, err_json("bad json"));
        };
        if !valid_name(&input.username) || input.password.len() < 8 || input.password.len() > MAX_PASSWORD_LEN {
            return (Status::BadRequest, err_json("invalid username or password length"));
        }
        let mut store = self.store.lock().unwrap();
        if store.accounts.contains_key(&input.username) {
            return (Status::Conflict, err_json("username already exists"));
        }
        let mut salt = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        store.accounts.insert(
            input.username.clone(),
            Account {
                salt: hex(&salt),
                password_hash: hash_password(&salt, &input.password),
            },
        );
        if self.save(&store).is_err() {
            return (Status::InternalServerError, err_json("failed to save account"));
        }
        (Status::Ok, r#"{"ok":true}"#.to_string())
    }
    fn login(&self, body: &[u8], peer: Option<IpAddr>) -> (Status, String) {
        if let Some(ip) = peer {
            let penalties = self.login_failures.lock().unwrap();
            if let Some(p) = penalties.get(&ip) {
                if p.locked_until > now() {
                    return (Status::TooManyRequests, err_json("too many failed logins, try again later"));
                }
            }
        }
        #[derive(Deserialize)]
        struct In {
            username: String,
            password: String
        }
        let Ok(input) = serde_json::from_slice::<In>(body) else {
            return (Status::BadRequest, err_json("bad json"));
        };
        let mut store = self.store.lock().unwrap();
        let Some(acct) = store.accounts.get(&input.username) else {
            self.record_login_failure(peer);
            return (Status::Unauthorized, err_json("account does not exist"));
        };
        let Ok(salt) = (0..acct.salt.len() / 2)
            .map(|i| u8::from_str_radix(&acct.salt[2 * i..2 * i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
        else {
            return (Status::InternalServerError, err_json("corrupt salt"))
        };
        if hash_password(&salt, &input.password) != acct.password_hash {
            self.record_login_failure(peer);
            return (Status::Unauthorized, err_json("wrong password"))
        }
        if let Some(ip) = peer {
            self.login_failures.lock().unwrap().remove(&ip);
        }
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        let api_key = hex(&key);
        store.api_keys.insert(sha256_hex(api_key.as_bytes()), input.username.clone());
        if self.save(&store).is_err() {
            return (Status::InternalServerError, err_json("save failed"));
        }
        (
            Status::Ok,
            format!(r#"{{"api_key":"{api_key}","username":"{}"}}"#, input.username)
        )
    }
    fn record_login_failure(&self, peer: Option<IpAddr>) {
        let Some(ip) = peer else {return};
        let mut penalties = self.login_failures.lock().unwrap();
        let p = penalties.entry(ip).or_default();
        p.failures += 1;
        if p.failures >= LOGIN_MAX_FAILURES {
            p.locked_until = now() + LOGIN_COOLDOWN_SECS;
            p.failures = 0;
        }
    }
    fn logout(&self, req: &Request) -> (Status, String) {
        let Some(token) = req.get_header("Authorization").and_then(|v| v.strip_prefix("Bearer "))
        else {
            return (Status::Unauthorized, err_json("auth required"));
        };
        let mut store = self.store.lock().unwrap();
        let hash = sha256_hex(token.as_bytes());
        if store.api_keys.remove(&hash).is_none() {
            return (Status::Unauthorized, err_json("unknown api key"));
        }
        if self.save(&store).is_err() {
            return (Status::InternalServerError, err_json("save failed"));
        }
        (Status::Ok, r#"{"ok":true}"#.to_string())
    }
    fn authed_user<'a>(&self, store: &Store, req: &'a Request) -> Option<String> {
        let token = req.get_header("Authorization")?.strip_prefix("Bearer ")?;
        store.api_keys.get(&sha256_hex(token.as_bytes())).cloned()
    }
    fn put_domain(&self, req: &Request, name: &str) -> (Status, String) {
        #[derive(Deserialize)]
        struct In {
            endpoints: Vec<String>,
            ttl: Option<u64>
        }
        let Ok(input) = serde_json::from_slice::<In>(&req.body) else {
            return (Status::BadRequest, err_json("bad json"));
        };
        if !valid_name(name) || input.endpoints.is_empty() {
            return (Status::BadRequest, err_json("invalid domain or endpoints"));
        }
        if input.endpoints.len() > MAX_ENDPOINTS || input.endpoints.iter().any(|e| !valid_endpoint(e)) {
            return (Status::BadRequest, err_json("invalid endpoints"));
        }
        let ttl = input.ttl.unwrap_or(DEFAULT_TTL_SECS).clamp(1, MAX_TTL_SECS);
        let mut store = self.store.lock().unwrap();
        let Some(user) = self.authed_user(&store, req) else {
            return (Status::Unauthorized, err_json("auth required"));
        };
        match store.domains.get(name) {
            Some(d) if d.owner != user && d.expires_at > now() => {
                return (Status::Conflict, err_json("domain owned by someone else"));
            }
            _ => {}
        }
        store.domains.insert(
            name.to_string(),
            Domain {
                owner: user, 
                endpoints: input.endpoints,
                expires_at: now() + DOMAIN_TTL_SECS,
                ttl
            },
        );
        if self.save(&store).is_err() {
            return (Status::InternalServerError, err_json("save failed"));
        }
        (Status::Ok, format!(r#"{{"ok":true,"expires_at":{}}}"#, now() + DOMAIN_TTL_SECS))
    }
    fn get_domain(&self, req: &Request, name: &str) -> (Status, String) {
        let store = self.store.lock().unwrap();
        let Some(user) = self.authed_user(&store, req) else {
            return (Status::Unauthorized, err_json("auth required"));
        };
        let Some(d) = store.domains.get(name) else {
            return (Status::NotFound, err_json("no such domain"));
        };
        if d.owner != user {
            return (Status::Forbidden, err_json("not your domain"));
        }
        (
            Status::Ok,
            serde_json::to_string(&serde_json::json!({
                "name": name,
                "owner": d.owner,
                "endpoints": d.endpoints,
                "expires_at": d.expires_at,
                "ttl": d.ttl,
                "expired": d.expires_at <= now()
            }))
            .unwrap()
        )
    }
    fn delete_domain(&self, req: &Request, name: &str) -> (Status, String) {
        let mut store = self.store.lock().unwrap();
        let Some(user) = self.authed_user(&store, req) else {
            return (Status::Unauthorized, err_json("auth required"));
        };
        let Some(d) = store.domains.get(name) else {
            return (Status::NotFound, err_json("no such domain"));
        };
        if d.owner != user {
            return (Status::Forbidden, err_json("not your domain"));
        }
        store.domains.remove(name);
        if self.save(&store).is_err() {
            return (Status::InternalServerError, err_json("save failed"));
        }
        (Status::Ok, r#"{"ok":true}"#.to_string())
    }
    fn list_domains(&self, req: &Request) -> (Status, String) {
        let store = self.store.lock().unwrap();
        let Some(user) = self.authed_user(&store, req) else {
            return (Status::Unauthorized, err_json("auth required"));
        };
        let mine: Vec<serde_json::Value> = store.domains.iter()
            .filter(|(_, d)| d.owner == user)
            .map(|(name, d)| {
                serde_json::json!({
                    "name": name, 
                    "endpoints": d.endpoints,
                    "expires_at": d.expires_at,
                    "ttl": d.ttl,
                    "expired": d.expires_at <= now()
                })
            })
            .collect();
        (Status::Ok, serde_json::to_string(&serde_json::json!({"domains": mine})).unwrap())
    }
    pub fn resolve_record(&self, name: &str) -> Option<ResolveRecord> {
        let store = self.store.lock().unwrap();
        let d = store.domains.get(name)?;
        let at = now();
        if d.expires_at <= at {
            return None;
        }
        let msg = canonical_resolve(name, &d.endpoints, d.expires_at, d.ttl, at);
        let sig: Signature = self.signing.sign(&msg);
        Some(ResolveRecord {
            name: name.to_string(),
            endpoints: d.endpoints.clone(),
            expires_at: d.expires_at,
            not_before: at,
            ttl: DEFAULT_TTL_SECS,
            key: self.verifying_key_b64(),
            signature: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, sig.to_bytes())
        })
    }
    fn resolve(&self, name: &str) -> (Status, String) {
        match self.resolve_record(name) {
            Some(rec) => (Status::Ok, serde_json::to_string(&rec).unwrap()),
            None => (Status::NotFound, err_json("no such domain"))
        }
    }
    fn route(&self, req: &Request, peer: Option<IpAddr>) -> Response {
        let (status, body) = match (req.method.as_str(), req.path.as_str()) {
            ("POST", "/v1/accounts") => self.create_account(&req.body),
            ("POST", "/v1/login") => self.login(&req.body, peer),
            ("POST", "/v1/logout") => self.logout(req),
            ("GET", "/v1/domains") => self.list_domains(req),
            ("PUT", p) if p.starts_with("/v1/domains/") => {
                self.put_domain(req, &p["/v1/domains/".len()..])
            }
            ("GET", p) if p.starts_with("/v1/domains/") => {
                self.get_domain(req, &p["/v1/domains/".len()..])
            }
            ("DELETE", p) if p.starts_with("/v1/domains/") => {
                self.delete_domain(req, &p["/v1/domains/".len()..])
            }
            ("GET", p) if p.starts_with("/v1/resolve/") => {
                self.resolve(&p["/v1/resolve/".len()..])
            }
            ("GET", "/v1/key") => (
                Status::Ok,
                format!(r#"{{"key":"{}"}}"#, self.verifying_key_b64()),
            ),
            _ => (Status::NotFound, err_json("unknown endpoint")),
        };
        let mut resp = Response::new(status, body.into_bytes());
        resp.set_header("Content-Type", "application/json");
        resp
    }
    pub fn sweep_expired(&self) -> usize {
        let mut store = self.store.lock().unwrap();
        let at = now();
        let before = store.domains.len();
        store.domains.retain(|_, d| d.expires_at > at);
        let removed = before - store.domains.len();
        if removed > 0 {
            let _ = self.save(&store);
        }
        removed
    }
}

fn err_json(msg: &str) -> String {
    format!(r#"{{"error":"{msg}"}}"#) 
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 63 
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && s.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        && s.bytes().last().is_some_and(|b| b.is_ascii_alphanumeric())
}

fn valid_endpoint(s: &str) -> bool {
    let Some((host, port)) = s.rsplit_once(':') else {
        return false;
    };
    if port.parse::<u16>().is_err() {
        return false;
    }
    !host.is_empty()
        && host.len() <= 253
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
}

pub fn verify_record(rec: &ResolveRecord, at_unix: u64) -> bool {
    if rec.expires_at <= at_unix || rec.not_before > at_unix {
        return false;
    }
    let Ok(key_bytes) = b64_decode(&rec.key) else {
        return false;
    };
    let Ok(sig_bytes) = b64_decode(&rec.signature) else {
        return false;
    };
    let (Ok(key_arr), Ok(sig_arr)): (Result<[u8; 32], _>, Result<[u8; 64], _>) = (
        key_bytes.as_slice().try_into().map_err(|_| ()),
        sig_bytes.as_slice().try_into().map_err(|_| ())
    ) else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_bytes(&key_arr) else {
        return false;
    };
    let msg = canonical_resolve(&rec.name, &rec.endpoints, rec.expires_at, rec.ttl, rec.not_before);
    vk.verify_strict(&msg, &Signature::from_bytes(&sig_arr)).is_ok()
}

pub struct RegistryServer {
    registry: Arc<Registry>,
    acceptor: TlsAcceptor,
    listener: TcpListener
}

impl RegistryServer {
    pub async fn bind(
        addr: SocketAddr, 
        registry: Registry, 
        certs: Vec<rustls::pki_types::CertificateDer<'static>>,
        key: rustls::pki_types::PrivateKeyDer<'static>
    ) -> Result<Self, RegistryError> {
        let mut config =
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| RegistryError::TlsIdentity(e.to_string()))?;
        config.alpn_protocols = vec![ALPN.as_bytes().to_vec()];
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            registry: Arc::new(registry),
            acceptor: TlsAcceptor::from(Arc::new(config)),
            listener
        })
    }
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }
    pub async fn run(self) -> Result<(), RegistryError> {
        loop {
            let (tcp, peer) = self.listener.accept().await?;
            let acceptor = self.acceptor.clone();
            let registry = self.registry.clone();
            tokio::spawn(async move {
                if let Err(e) = handle(acceptor, tcp, registry, peer.ip()).await {
                    warn!(%peer, error = %e, "registry connection err");
                }
            });
        }
    }
}

async fn handle(
    acceptor: TlsAcceptor,
    tcp: tokio::net::TcpStream,
    registry: Arc<Registry>,
    peer: IpAddr
) -> anyhow::Result<()> {
    let mut tls = acceptor.accept(tcp).await?;
    let deadline = Instant::now() + HEAD_TIMEOUT;
    let mut buf = Vec::with_capacity(16 * 1024);
    let max_head = limits::MAX_REQUEST_LINE + limits::MAX_HEADERS + 4;
    loop {
        if find_header_end(&buf).is_some() {
            break;
        }
        if buf.len() > max_head + limits::MAX_BODY {
            anyhow::bail!("request too large");
        }
        let mut tmp = [0u8; 16 * 1024];
        let n = match timeout_at(deadline, tls.read(&mut tmp)).await {
            Ok(Ok(0)) => anyhow::bail!("closed"),
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => anyhow::bail!("head timeout"),
        };
        buf.extend_from_slice(&tmp[..n]);
    }
    let body_deadline = Instant::now() + BODY_TIMEOUT;
    loop {
        match Request::parse(&buf) {
            Ok(req) => {
                let resp = registry.route(&req, Some(peer));
                tls.write_all(&resp.serialize()).await?;
                tls.flush().await?;
                return Ok(());
            }
            Err(sonion_protocol::ProtocolError::UnexpectedEof) => {
                if buf.len() > max_head + limits::MAX_BODY {
                    let resp = Response::new(Status::PayloadTooLarge, b"payload too large".to_vec());
                    tls.write_all(&resp.serialize()).await?;
                    anyhow::bail!("request too large");
                }
                let mut tmp = [0u8; 16 * 1024];
                let n = match timeout_at(body_deadline, tls.read(&mut tmp)).await {
                    Ok(Ok(0)) => {
                        let resp = Response::new(Status::BadRequest, b"incomplete request".to_vec());
                        tls.write_all(&resp.serialize()).await?;
                        return Ok(());
                    }
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => return Err(e.into()),
                    Err(_) => {
                        let resp = Response::new(Status::BadRequest, b"request timeout".to_vec());
                        tls.write_all(&resp.serialize()).await?;
                        return Ok(());
                    }
                };
                buf.extend_from_slice(&tmp[..n]);
            },
            Err(e) => {
                let resp = Response::new(Status::BadRequest, b"bad request".to_vec());
                tls.write_all(&resp.serialize()).await?;
                anyhow::bail!("malformed: {e}")
            }
        }
    }
}