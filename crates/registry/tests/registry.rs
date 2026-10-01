use registry::{Registry, RegistryServer, verify_record, ResolveRecord};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sonion_protocol::{Request, SonionUrl, Status, send};

fn certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let kp = rcgen::KeyPair::generate().unwrap();
    let p = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let c = p.self_signed(&kp).unwrap();
    (
        vec![c.der().clone()],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(kp.serialize_der())),
    )
}

async fn spawn() -> (tempfile::TempDir, u16) {
    let tmp = tempfile::tempdir().unwrap();
    let reg = Registry::open(tmp.path()).unwrap();
    let (c, k) = certs();
    let srv = RegistryServer::bind("127.0.0.1:0".parse().unwrap(),reg, c, k).await.unwrap();
    let port = srv.local_addr().unwrap().port();
    tokio::spawn(srv.run());
    (tmp, port)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn url(port: u16, p: &str) -> SonionUrl {
    SonionUrl::parse(&format!("sonion://localhost:{port}{p}")).unwrap()
}

async fn call(port: u16, req: &Request) -> sonion_protocol::Response {
    send(&url(port, "/"), req, true).await.unwrap()
}

#[tokio::test]
async fn register_login_resolve_flow() {
    let (_t, port) = spawn().await;
    let r = call(port, &Request::post("/v1/accounts", r#"{"username":"alice","password":"hunter2pass"}"#.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Ok);
    let r = call(port, &Request::post("/v1/accounts", r#"{"username":"alice","password":"hunter2pass"}"#.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Conflict);
    let r = call(port, &Request::post("/v1/login", r#"{"username":"alice","password":"hunter2pass"}"#.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Ok);
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    let key = v["api_key"].as_str().unwrap().to_string();
    let r = call(
        port,
        &Request::put("/v1/domains/hello", r#"{"endpoints":["127.0.0.1:6767"]}"#.as_bytes().to_vec())
            .header("Authorization", &format!("Bearer {key}")),
    )
    .await;
    assert_eq!(r.status, Status::Ok);
    let r = call(port, &Request::get("/v1/resolve/hello")).await;
    assert_eq!(r.status, Status::Ok);
    let rec: ResolveRecord = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(rec.endpoints, vec!["127.0.0.1:6767".to_string()]);
    assert!(verify_record(&rec, now_secs()), "signed record must verify");
}
#[tokio::test]
async fn unauthenticated_domain_update_rejected() {
    let (_t, port) = spawn().await;
    let r = call(port, &Request::put("/v1/domains/x", r#"{"endpoints":["a:1"]}"#.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Unauthorized);
}
#[tokio::test]
async fn tampered_record_fails_verification() {
    let (_t, port) = spawn().await;
    call(port, &Request::post("/v1/accounts", r#"{"username":"bob","password":"hunter2pass"}"#.as_bytes().to_vec())).await;
    let r = call(port, &Request::post("/v1/login", r#"{"username":"bob","password":"hunter2pass"}"#.as_bytes().to_vec())).await;
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    let key = v["api_key"].as_str().unwrap().to_string();
    call(port, &Request::put("/v1/domains/site", r#"{"endpoints":["good:1"]}"#.as_bytes().to_vec())
        .header("Authorization", &format!("Bearer {key}"))).await;
    let r = call(port, &Request::get("/v1/resolve/site")).await;
    let mut rec: ResolveRecord = serde_json::from_slice(&r.body).unwrap();
    assert!(verify_record(&rec, now_secs()));
    // tamper: flip endpoint
    rec.endpoints = vec!["evil:9".to_string()];
    assert!(!verify_record(&rec, now_secs()), "tampered record MUST be rejected");
}
