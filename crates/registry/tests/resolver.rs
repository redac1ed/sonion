use registry::client::{RegistryClient, SonionResolver, endpoint_url};
use registry::{Registry, RegistryServer};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sonion_protocol::SonionUrl;

fn certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let kp = rcgen::KeyPair::generate().unwrap();
    let p = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let c = p.self_signed(&kp).unwrap();
    (
        vec![c.der().clone()],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(kp.serialize_der()))
    )
}

async fn spawn() -> (tempfile::TempDir, u16) {
    let tmp = tempfile::tempdir().unwrap();
    let reg = Registry::open(tmp.path()).unwrap();
    let (c, k) = certs();
    let srv = RegistryServer::bind("127.0.0.1:0".parse().unwrap(), reg, c, k).await.unwrap();
    let port = srv.local_addr().unwrap().port();
    tokio::spawn(srv.run());
    (tmp, port)
}

fn base(port: u16) -> SonionUrl {
    SonionUrl::parse(&format!("sonion://localhost:{port}/")).unwrap()
}

async fn setup_domain(port: u16, user: &str, domain: &str, endpoints: &[&str]) -> String {
    let client = RegistryClient::new(base(port), true);
    client.create_account(user, "skibidi123").await.unwrap();
    let key = client.login(user, "skibidi123").await.unwrap();
    let eps: Vec<String> = endpoints.iter().map(|s| s.to_string()).collect();
    client.put_domain(&key, domain, &eps, None).await.unwrap();
    key
}

#[tokio::test]
async fn resolver_fetches_and_caches() {
    let (_t, port) = spawn().await;
    setup_domain(port, "sobble", "cached-site.son", &["127.0.0.1:6767"]).await;
    let resolver = SonionResolver::new(base(port), true, None);
    let rec1 = resolver.resolve("cached-site.son").await.unwrap();
    assert_eq!(rec1.name, "cached-site.son");
    assert_eq!(resolver.cache_len(), 1);
    let rec2 = resolver.resolve("cached-site.son").await.unwrap();
    assert_eq!(rec1.signature, rec2.signature);
}
#[tokio::test]
async fn resolver_rejects_unknown_domain() {
    let (_t, port) = spawn().await;
    let resolver = SonionResolver::new(base(port), true, None);
    let err = resolver.resolve("ajlaj").await.unwrap_err();
    assert!(err.to_string().contains("no such domain"));
}
#[tokio::test]
async fn resolver_pin_rejects_wrong_key() {
    let (_t, port) = spawn().await;
    setup_domain(port, "sobble", "erm.son", &["127.0.0.1:6767"]).await;
    let wrong_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string();
    let resolver = SonionResolver::new(base(port), true, Some(wrong_key));
    let err = resolver.resolve("erm.son").await.unwrap_err();
    assert!(err.to_string().contains("verification"));
}
#[tokio::test]
async fn resolver_pin_accepts_correct_key() {
    let (_t, port) = spawn().await;
    setup_domain(port, "sobble", "trusted.son", &["127.0.0.1:6767"]).await;
    let probe = SonionResolver::new(base(port), true, None);
    let rec = probe.resolve("trusted.son").await.unwrap();
    let resolver = SonionResolver::new(base(port), true, Some(rec.key.clone()));
    assert!(resolver.resolve("trusted.son").await.is_ok());
}
#[tokio::test]
async fn invalidate_forces_refetch() {
    let (_t, port) = spawn().await;
    setup_domain(port, "sobble", "watesiggma.son", &["127.0.0.1:6767"]).await;
    let resolver = SonionResolver::new(base(port), true, None);
    let rec1 = resolver.resolve("watesiggma.son").await.unwrap();
    resolver.invalidate("watesiggma.son");
    assert_eq!(resolver.cache_len(), 0);
    let rec2 = resolver.resolve("watesiggma.son").await.unwrap();
    assert!(rec2.not_before >= rec1.not_before);
}
#[tokio::test]
async fn clear_expired_prunes() {
    let (_t, port) = spawn().await;
    setup_domain(port, "sobble", "sasda.son", &["127.0.0.1:6767"]).await;
    let resolver = SonionResolver::new(base(port), true,  None);
    resolver.resolve("sasda.son").await.unwrap();
    assert_eq!(resolver.cache_len(), 1);
    assert_eq!(resolver.clear_expired(), 0);
}
#[tokio::test]
async fn endpoint_url_parsing() {
    let u = endpoint_url("example.son:6767", "/index.html").unwrap();
    assert_eq!(u.host, "example.son");
    assert_eq!(u.port, 6767);
    assert_eq!(u.path, "/index.html");
    assert!(endpoint_url("no-port", "/").is_none());
    assert!(endpoint_url(":6767", "/").is_none());
    assert!(endpoint_url("h:notaport", "/").is_none());
}
#[tokio::test]
async fn registry_client_full_flow() {
    let (_t, port) = spawn().await;
    let client = RegistryClient::new(base(port), true);
    client.create_account("sobble", "skibidi123").await.unwrap();
    let err = client.create_account("sobble", "skibidi123").await.unwrap_err();
    assert!(err.to_string().contains("409") || err.to_string().contains("exists"));
    let key = client.login("sobble", "skibidi123").await.unwrap();
    assert_eq!(key.len(), 64);
    let expires = client.put_domain(&key, "sumshi.son", &["127.0.0.1:6767".to_string()], Some(600)).await.unwrap();
    assert!(expires > 0);
    let domains = client.list_domains(&key).await.unwrap();
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name, "sumshi.son");
    assert_eq!(domains[0].ttl, 600);
    assert!(!domains[0].expired);
    client.delete_domain(&key, "sumshi.son").await.unwrap();
    let domains = client.list_domains(&key).await.unwrap();
    assert!(domains.is_empty());
    client.logout(&key).await.unwrap();
    let err = client.list_domains(&key).await.unwrap_err();
    assert!(err.to_string().contains("401") || err.to_string().contains("auth"));
}
#[tokio::test]
async fn registry_client_login_failures() {
    let (_t, port) = spawn().await;
    let client = RegistryClient::new(base(port), true);
    client.create_account("sobble", "skibidi123").await.unwrap();
    let err = client.login("sobble", "asdfs").await.unwrap_err();
    assert!(err.to_string().contains("401"));
    let err = client.login("sobbel", "skibidi123").await.unwrap_err();
    assert!(err.to_string().contains("401"));
}
#[tokio::test]
async fn verifying_key_endpoint_matches_records() {
    let (_t, port) = spawn().await;
    setup_domain(port, "sobble", "meowmeow.son", &["127.0.0.1:6767"]).await;
    let client = RegistryClient::new(base(port), true);
    let key = client.verifying_key().await.unwrap();
    let rec = client.resolve_record("meowmeow.son").await.unwrap();
    assert_eq!(key, rec.key);
}