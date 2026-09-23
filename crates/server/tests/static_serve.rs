use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use sonion_protocol::tls::client_config;
use sonion_protocol::{Request, SonionUrl, Status, fetch, send};
use sonion_server::Server;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsConnector;

fn test_certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    (
        vec![cert.der().clone()],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der())),
    )
}

struct TestSite {
    _tmp: tempfile::TempDir,
    port: u16,
}

async fn spawn_site() -> TestSite {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("site");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("index.html"), "<h1>konichiwa</h1>").unwrap();
    std::fs::write(root.join("style.css"), "body { color: red; }").unwrap();
    std::fs::write(root.join("app.js"), "console.log('hi');").unwrap();
    std::fs::write(root.join("data.json"), r#"{"ok":true}"#).unwrap();
    std::fs::write(
        root.join("logo.png"),
        (0..=255u8).cycle().take(1024).collect::<Vec<_>>(),
    )
    .unwrap();
    std::fs::write(
        root.join("big.bin"),
        (0..300 * 1024u32)
            .map(|i| (i % 251) as u8)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    std::fs::write(tmp.path().join("secret.txt"), "top secret").unwrap();
    let (certs, key) = test_certs();
    let server = Server::bind("127.0.0.1:0".parse().unwrap(), &root, certs, key)
        .await
        .unwrap();
    let port = server.local_addr().unwrap().port();
    tokio::spawn(server.run());
    TestSite { _tmp: tmp, port }
}

impl TestSite {
    fn url(&self, path: &str) -> SonionUrl {
        SonionUrl::parse(&format!("sonion://localhost:{}{path}", self.port)).unwrap()
    }
}

#[tokio::test]
async fn serves_html_css_js_json() {
    let site = spawn_site().await;
    let r = fetch(&site.url("/"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.get_header("Content-Type"), Some("text/html"));
    assert_eq!(r.body, b"<h1>konichiwa</h1>");
    let r = fetch(&site.url("/style.css"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert!(r.get_header("Content-Type").unwrap().contains("css"));
    assert_eq!(r.body, b"body { color: red; }");
    let r = fetch(&site.url("/app.js"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert!(r.get_header("Content-Type").unwrap().contains("javascript"));
    let r = fetch(&site.url("/data.json"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.get_header("Content-Type"), Some("application/json"));
    assert_eq!(r.body, br#"{"ok":true}"#);
}

#[tokio::test]
async fn serves_binary_exact_bytes() {
    let site = spawn_site().await;
    let r = fetch(&site.url("/logo.png"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.get_header("Content-Type"), Some("image/png"));
    let expected: Vec<u8> = (0..=255u8).cycle().take(1024).collect();
    assert_eq!(r.body, expected);
}

#[tokio::test]
async fn large_file_uses_chunked() {
    let site = spawn_site().await;
    let r = fetch(&site.url("/big.bin"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert!(r.is_chunked());
    let expected: Vec<u8> = (0..300 * 1024u32).map(|i| (i % 251) as u8).collect();
    assert_eq!(r.body, expected);
}

#[tokio::test]
async fn head_matches_get_without_body() {
    let site = spawn_site().await;
    let get = fetch(&site.url("/"), true).await.unwrap();
    let req = Request::head("/").header("Host", "localhost");
    let head = send(&site.url("/"), &req, true).await.unwrap();
    assert_eq!(head.status, Status::Ok);
    assert!(head.body.is_empty());
    assert_eq!(
        head.get_header("Content-Length"),
        get.get_header("Content-Length")
    );
    assert_eq!(
        head.get_header("Content-Type"),
        get.get_header("Content-Type")
    );
}

#[tokio::test]
async fn mission_file_is_404() {
    let site = spawn_site().await;
    let r = fetch(&site.url("/nope.txt"), true).await.unwrap();
    assert_eq!(r.status, Status::NotFound);
}

#[tokio::test]
async fn spa_fallback_serves_index() {
    let site = spawn_site().await;
    let r = fetch(&site.url("/dashboard/settings"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.body, b"<h1>konichiwa</h1>");
}

#[tokio::test]
async fn rejects_traversal() {
    let site = spawn_site().await;
    for path in [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/..%2fsecret.txt",
        "/..\\secret.txt",
        "/%2e%2e%5csecret.txt",
    ] {
        let req = Request::get(path).header("Host", "localhost");
        let r = send(&site.url("/"), &req, true)
            .await
            .unwrap_or_else(|e| panic!("request failed for {path}: {e}"));
        assert_eq!(r.status, Status::BadRequest, "path: {path}");
    }
}

#[tokio::test]
async fn malformed_request_gets_400() {
    let site = spawn_site().await;
    let connector = TlsConnector::from(Arc::new(client_config(true)));
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", site.port))
        .await
        .unwrap();
    let name = ServerName::try_from("localhost".to_string()).unwrap();
    let mut tls = connector.connect(name, tcp).await.unwrap();
    tls.write_all(b"GARBAGE LINE\r\n\r\n").await.unwrap();
    tls.flush().await.unwrap();
    let mut tmp = [0u8; 4096];
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), tls.read(&mut tmp))
        .await
        .expect("server should respond")
        .unwrap();
    let text = String::from_utf8_lossy(&tmp[..n]);
    assert!(text.starts_with("SONION/1.0 400"), "got: {text}");
}

#[tokio::test]
async fn rejects_wrong_alpn() {
    let site = spawn_site().await;
    let mut cfg = client_config(true);
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    let connector = TlsConnector::from(Arc::new(cfg));
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", site.port))
        .await
        .unwrap();
    let name = ServerName::try_from("localhost".to_string()).unwrap();
    assert!(connector.connect(name, tcp).await.is_err());
}

#[tokio::test]
async fn bind_rejects_missing_root() {
    let (certs, key) = test_certs();
    let res = Server::bind(
        "127.0.0.1:0".parse().unwrap(),
        std::path::Path::new("definitely-not-a-real-dir-12345"),
        certs,
        key,
    )
    .await;
    assert!(res.is_err());
}
