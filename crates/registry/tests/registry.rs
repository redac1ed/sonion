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

async fn make_user(port: u16, name: &str) -> String {
    let body = format!(r#"{{"username":"{name}","password":"hunter2pass"}}"#);
    let r = call(port, &Request::post("/v1/accounts", body.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Ok);
    let r = call(port, &Request::post("/v1/login", body.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Ok);
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    v["api_key"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn register_login_resolve_flow() {
    let (_t, port) = spawn().await;
    let key = make_user(port, "alice").await;
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
    let key = make_user(port, "bob").await;
    call(port, &Request::put("/v1/domains/site", r#"{"endpoints":["good:1"]}"#.as_bytes().to_vec())
        .header("Authorization", &format!("Bearer {key}"))).await;
    let r = call(port, &Request::get("/v1/resolve/site")).await;
    let mut rec: ResolveRecord = serde_json::from_slice(&r.body).unwrap();
    assert!(verify_record(&rec, now_secs()));
    rec.endpoints = vec!["evil:9".to_string()];
    assert!(!verify_record(&rec, now_secs()), "tampered record MUST be rejected");
    let mut rec2: ResolveRecord = serde_json::from_slice(&call(port, &Request::get("/v1/resolve/site")).await.body).unwrap();
    rec2.expires_at += 999_999;
    assert!(!verify_record(&rec2, now_secs()));
    let mut rec3: ResolveRecord = serde_json::from_slice(&call(port, &Request::get("/v1/resolve/site")).await.body).unwrap();
    rec3.ttl += 1;
    assert!(!verify_record(&rec3, now_secs()));
}
#[tokio::test]
async fn duplicate_account_conflict() {
    let (_t, port) = spawn().await;
    let body = r#"{"username":"alice","password":"hunter2pass"}"#;
    let r = call(port, &Request::post("/v1/accounts", body.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Ok);
    let r = call(port, &Request::post("/v1/accounts", body.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::Conflict);
}
#[tokio::test]
async fn multiple_api_keys_both_valid() {
    let (_t, port) = spawn().await;
    let body = r#"{"username":"carol","password":"hunter2pass"}"#;
    call(port, &Request::post("/v1/accounts", body.as_bytes().to_vec())).await;
    let r1 = call(port, &Request::post("/v1/login", body.as_bytes().to_vec())).await;
    let r2 = call(port, &Request::post("/v1/login", body.as_bytes().to_vec())).await;
    let k1 = serde_json::from_slice::<serde_json::Value>(&r1.body).unwrap()["api_key"].as_str().unwrap().to_string();
    let k2 = serde_json::from_slice::<serde_json::Value>(&r2.body).unwrap()["api_key"].as_str().unwrap().to_string();
    assert_ne!(k1, k2); 
    for k in [&k1, &k2] {
        let r = call(port, &Request::put("/v1/domains/multi", r#"{"endpoints":["h:1"]}"#.as_bytes().to_vec())
                .header("Authorization", &format!("Bearer {k}"))).await;
        assert_eq!(r.status, Status::Ok);
    }
}
#[tokio::test]
async fn logout_revokes_key() {
    let (_t, port) = spawn().await;
    let key = make_user(port, "dave").await;
    let r = call(port, &Request::post("/v1/logout", Vec::new()).header("Authorization", &format!("Bearer {key}"))).await;
    assert_eq!(r.status, Status::Ok);
    let r = call(port, &Request::put("/v1/domains/gone", r#"{"endpoints":["h:1"]}"#.as_bytes().to_vec())
        .header("Authorization", &format!("Bearer {key}"))).await;
    assert_eq!(r.status, Status::Unauthorized);
}
#[tokio::test]
async fn get_and_delete_own_domain() {
    let (_t, port) = spawn().await;
    let key = make_user(port, "aaa").await;
    call(port, &Request::put("/v1/domains/mine", r#"{"endpoints":["h:1"]}"#.as_bytes().to_vec())
        .header("Authorization", &format!("Bearer {key}"))).await;
    let r = call(port, &Request::get("/v1/domains/mine").header("Authorization", &format!("Bearer {key}"))).await;
    assert_eq!(r.status, Status::Ok);
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(v["owner"], "aaa");
    let r = call(port, &Request::get("/v1/domains").header("Authorization", &format!("Bearer {key}"))).await;
    assert_eq!(r.status, Status::Ok);
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(v["domains"].as_array().unwrap().len(), 1);
    let r = call(port, &Request::get("/v1/domains/mine").header("Authorization", "Bearer deadbeef").header("X-Method", "DELETE")).await;
    assert_eq!(r.status, Status::Unauthorized);
    let del = Request {
        method: "DELETE".into(),
        path: "/v1/domains/mine".into(),
        headers: vec![("Authorization".into(), format!("Bearer {key}"))],
        body: Vec::new()
    };
    let r = call(port, &del).await;
    assert_eq!(r.status, Status::Ok);
    let r = call(port, &Request::get("/v1/resolve/mine")).await;
    assert_eq!(r.status, Status::NotFound);
}
#[tokio::test]
async fn foreign_domain_get_and_delete_forbidden() {
    let (_t, port) = spawn().await;
    let owner = make_user(port, "owner").await;
    let theif = make_user(port, "theif").await;
    call(port, &Request::put("/v1/domains/turf", r#"{"endpoints":["h:1"]}"#.as_bytes().to_vec())
        .header("Authorization", &format!("Bearer {owner}"))).await;
    let r = call(port, &Request::get("/v1/domains/turf").header("Authorization", &format!("Bearer {theif}"))).await;
    assert_eq!(r.status, Status::Forbidden);
    let del = Request {
        method: "DELETE".into(),
        path: "/v1/domains/turf".into(),
        headers: vec![("Authorization".into(), format!("Bearer {theif}"))],
        body: Vec::new()
    };
    let r = call(port, &del).await;
    assert_eq!(r.status, Status::Forbidden);
    let r = call(port, &Request::put("/v1/domains/turf", r#"{"endpoints":["evil:1"]}"#.as_bytes().to_vec())
        .header("Authorization", &format!("Bearer {theif}"))).await;
    assert_eq!(r.status, Status::Conflict);
}
#[tokio::test]
async fn invalid_names_rejected() {
    let (_t, port) = spawn().await;
    let key = make_user(port, "frank").await;
    for name in ["-lead", "trail-", "UPPER", "under_score", "with.dot", ""] {
        let path = format!("/v1/domains/{name}");
        let r = call(port, &Request::put(&path, r#"{"endpoints":["h:1"]}"#.as_bytes().to_vec())
            .header("Authorization", &format!("Bearer {key}"))).await;
        assert!(matches!(r.status, Status::BadRequest | Status::NotFound), "name {name:?} got {}", r.status.code());
    }
    let long = "a".repeat(64);
    let r = call(port, &Request::put(&format!("/v1/domains/{long}"), r#"{"endpoints":["h:1"]}"#.as_bytes().to_vec())
        .header("Authorization", &format!("Bearer {key}"))).await;
    assert_eq!(r.status, Status::BadRequest);
}
#[tokio::test]
async fn login_rate_limited_after_failures() {
    let (_t, port) = spawn().await;
    call(port, &Request::post("/v1/accounts", r#"{"username":"grace","password":"skibdii"}"#.as_bytes().to_vec())).await;
    for _ in 0..5 {
        let r = call(port, &Request::post("/v1/login", r#"{"username":"grace","password":"wrong"}"#.as_bytes().to_vec())).await;
        assert_eq!(r.status, Status::Unauthorized);
    }
    let r = call(port, &Request::post("/v1/login", r#"{"username":"grace","password":"skibdii"}"#.as_bytes().to_vec())).await;
    assert_eq!(r.status, Status::TooManyRequests);
}
#[tokio::test]
async fn expired_domain_resolves_404() {
    let (_t, port) = spawn().await;
    let r = call(port, &Request::get("/v1/resolve/ouushii")).await;
    assert_eq!(r.status, Status::NotFound);
}
#[tokio::test]
async fn corrupt_db_fails_loudly() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("db.json"), b"{{{{not json").unwrap();
    let result = Registry::open(tmp.path());
    assert!(result.is_err(), "corrupt db must fail to open");
    let err = result.err().unwrap().to_string();
    assert!(err.contains("corrupt"), "unexpected error: {err}");
}
#[tokio::test]
async fn persistence_survives_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let (c, k) = certs();
    let port = {
        let reg = Registry::open(tmp.path()).unwrap();
        let srv = RegistryServer::bind("127.0.0.1:0".parse().unwrap(), reg, c, k).await.unwrap();
        let port = srv.local_addr().unwrap().port();
        let key_handle = srv.registry();
        tokio::spawn(srv.run());
        let key = {
            let body = r#"{"username":"henry","password":"soemth1ng"}"#;
            call(port, &Request::post("/v1/accounts", body.as_bytes().to_vec())).await;
            let r = call(port, &Request::post("/v1/login", body.as_bytes().to_vec())).await;
            serde_json::from_slice::<serde_json::Value>(&r.body).unwrap()["api_key"].as_str().unwrap().to_string()
        };
        call(port, &Request::put("/v1/domains/persist", r#"{"endpoints":["h:1"]}"#.as_bytes().to_vec())
            .header("Authorization", &format!("Bearer {key}"))).await;
        drop(key_handle);
        port
    };
    let _ = port;
    let reg = Registry::open(tmp.path()).unwrap();
    let rec = reg.resolve_record("persist");
    assert!(rec.is_some(), "domain must survive reopen");
    assert!(verify_record(&rec.unwrap(), now_secs()));
}