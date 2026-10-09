use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sonion_protocol::{Client, Request, SonionUrl, Status, fetch, send};
use sonion_server::Server;

fn test_certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let kp = rcgen::KeyPair::generate().unwrap();
    let p = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let c = p.self_signed(&kp).unwrap();
    (vec![c.der().clone()], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(kp.serialize_der())))
}

struct Site {
    _tmp: tempfile::TempDir,
    port: u16
}

async fn spawn(files: &[(&str, &str)]) -> Site {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("site");
    for (name, contents) in files {
        let p = root.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }
    let (certs, key) = test_certs();
    let server = Server::bind("127.0.0.1:0".parse().unwrap(), &root, certs, key).await.unwrap();
    let port = server.local_addr().unwrap().port();
    tokio::spawn(server.run());
    Site {_tmp: tmp, port}
}

impl Site {
    fn url(&self, path: &str) -> SonionUrl {
        SonionUrl::parse(&format!("sonion://localhost:{}{path}", self.port)).unwrap()
    }
}

#[tokio::test]
async fn directory_without_slash_redirects() {
    let site = spawn(&[("docs/index.html", "docs home")]).await;
    let r = fetch(&site.url("/docs"), true).await.unwrap();
    assert_eq!(r.status, Status::MovedPermanently);
    assert_eq!(r.get_header("Location"), Some("/docs/"));
    let r = fetch(&site.url("/docs/"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.body, b"docs home");
}
#[tokio::test]
async fn last_modified_then_304() {
    let site = spawn(&[("page.txt", "cache me")]).await;
    let r = fetch(&site.url("/page.txt"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    let lm = r.get_header("Last-Modified").expect("must send Last-Modified");
    let req = Request::get("/page.txt").header("Host", "localhost").header("If-Modified-Since", lm);
    let r = send(&site.url("/page.txt"), &req, true).await.unwrap();
    assert_eq!(r.status, Status::NotModified);
    assert!(r.body.is_empty());
}
#[tokio::test]
async fn stale_if_modified_since_gets_200() {
    let site = spawn(&[("fresh.txt", "new")]).await;
    let req = Request::get("/fresh.txt").header("Host", "localhost").header("If-Modified-Since", "Thu, 01 Jan 1970 00:00:00 GMT");
    let r = send(&site.url("/fresh.txt"), &req, true).await.unwrap();
    assert_eq!(r.status, Status::NotFound);
    assert_eq!(r.get_header("Content-Type"), Some("text/html"));
    assert_eq!(r.body, b"<h1>lost son</h1>");
}
#[tokio::test]
async fn keepalive_two_requests_one_connection() {
    let site = spawn(&[("a.txt", "aaa"), ("b.txt", "bbb")]).await;
    let client = Client::new(true);
    let r = client.send(&site.url("/a.txt"), &Request::get("/a.txt").header("Host", "localhost")).await.unwrap();
    assert_eq!(r.body, b"aaa");
    let r = client.send(&site.url("/b.txt"), &Request::get("b.txt").header("Host", "localhost")).await.unwrap();
    assert_eq!(r.body, b"bbb");
    assert!(client.idle_connection_count().await >= 1);
    client.close_all().await;
}
#[tokio::test]
async fn query_string_ignored_by_static_server() {
    let site = spawn(&[("data.json", r#"{"ok":true}"#)]).await;
    let r = fetch(&site.url("/data.json?v=2"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.body, br#"{"ok":true}"#);
}