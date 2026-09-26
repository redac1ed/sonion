use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use sonion_protocol::tls::client_config;
use sonion_protocol::{Request, SonionUrl, Status, fetch, send};
use sonion_server::Server;
use std::sync::Arc;
use std::time::Duration;
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

async fn spawn_site(files: &[(&str, Vec<u8>)]) -> (tempfile::TempDir, u16) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("site");
    std::fs::create_dir_all(&root).unwrap();
    for (name, contents) in files {
        std::fs::write(root.join(name), contents).unwrap();
    }
    let (certs, key) = test_certs();
    let server = Server::bind("127.0.0.1:0".parse().unwrap(), &root, certs, key)
        .await
        .unwrap();
    let port = server.local_addr().unwrap().port();
    tokio::spawn(server.run());
    (tmp, port)
}

fn url(port: u16, path: &str) -> SonionUrl {
    SonionUrl::parse(&format!("sonion://localhost:{port}{path}")).unwrap()
}

async fn raw_tls(port: u16) -> tokio_rustls::client::TlsStream<tokio::net::TcpStream> {
    let connector = TlsConnector::from(Arc::new(client_config(true)));
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let name = ServerName::try_from("localhost".to_string()).unwrap();
    connector.connect(name, tcp).await.unwrap()
}

#[tokio::test]
async fn slow_loris_killed_by_head_deadline() {
    let (_tmp, port) = spawn_site(&[("index.html", b"hi".to_vec())]).await;
    let mut tls = raw_tls(port).await;
    let req = b"GET / SONION/1.0\r\nHost: x\r\n\r\n";
    let start = std::time::Instant::now();
    let mut server_cut_us = false;
    for (i, byte) in req.iter().enumerate() {
        if tls.write_all(&[*byte]).await.is_err() || tls.flush().await.is_err() {
            server_cut_us = true;
            break;
        }
        if i + 1 < req.len() {
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
    }
    let mut buf = [0u8; 64];
    let read = tokio::time::timeout(Duration::from_secs(3), tls.read(&mut buf)).await;
    match (server_cut_us,read) {
        (_, Ok(Ok(0)) | Ok(Err(_)) | Err(_)) => {}
        (_, Ok(Ok(n))) => {
            let text = String::from_utf8_lossy(&buf[..n]);
            assert!(
                !text.starts_with("SONION/1.0 200"),
                "slow request past head deadline must not succeed (elapsed {:?})",
                start.elapsed()
            );
        }
    }
}

#[tokio::test]
async fn client_closing_mid_request_does_not_wedge_server() {
    let (_tmp, port) = spawn_site(&[("index.html", b"hi".to_vec())]).await;
    for _ in 0..20 {
        let mut tls = raw_tls(port).await;
        tls.write_all(b"GET / SONION/1.0\r\nHost:").await.unwrap();
        drop(tls); 
    }
    let r = fetch(&url(port, "/"), true).await.unwrap();
    assert_eq!(r.status, Status::Ok);
}

#[tokio::test]
async fn hundred_concurrent_fetches_all_succeed() {
    let body = b"concurrent body".to_vec();
    let (_tmp, port) = spawn_site(&[("index.html", body.clone())]).await;
    let mut handles = Vec::new();
    for _ in 0..100 {
        handles.push(tokio::spawn(async move {
            fetch(&url(port, "/"), true).await.unwrap()
        }));
    }
    for h in handles {
        let r = h.await.unwrap();
        assert_eq!(r.status, Status::Ok);
        assert_eq!(r.body, body);
    }
}

#[tokio::test]
async fn file_over_max_body_gets_413_not_silence() {
    let big = vec![0u8; 11 * 1024 * 1024]; // 11 MiB > 10 MiB cap
    let (_tmp, port) = spawn_site(&[("big.bin", big)]).await;
    let result = fetch(&url(port, "/big.bin"), true).await;
    match result {
        Ok(resp) => assert_eq!(
            resp.status,
            Status::PayloadTooLarge,
            "server must refuse >10MiB files with 413"
        ),
        Err(e) => panic!(
            "server streamed an oversized file and let the client fail: {e} — \
             server should enforce MAX_BODY itself and answer 413"
        ),
    }
}

#[tokio::test]
#[ignore = "takes ~60s by design (client body deadline); run explicitly"]
async fn partial_body_eventually_times_out() {
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (certs, key) = test_certs();
    let mut config =
        rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap();
    config.alpn_protocols = vec![sonion_protocol::ALPN.as_bytes().to_vec()];
    let acceptor = TlsAcceptor::from(Arc::new(config));
    tokio::spawn(async move {
        let Ok((tcp, _)) = listener.accept().await else { return };
        let Ok(mut tls) = acceptor.accept(tcp).await else { return };
        let mut buf = [0u8; 4096];
        let _ = tls.read(&mut buf).await; 
        tls.write_all(b"SONION/1.0 200 ok\r\nContent-Length: 100\r\n\r\nhello")
            .await
            .unwrap();
        tls.flush().await.unwrap();
        tokio::time::sleep(Duration::from_secs(120)).await; 
    });
    let start = std::time::Instant::now();
    let result = fetch(&url(port, "/"), true).await;
    assert!(result.is_err(), "truncated body must error, not succeed");
    assert!(
        start.elapsed() >= Duration::from_secs(59),
        "client gave up too early: {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn nul_byte_in_path_rejected() {
    let (_tmp, port) = spawn_site(&[("index.html", b"hi".to_vec())]).await;
    let req = Request::get("/%00index.html").header("Host", "localhost");
    let r = send(&url(port, "/"), &req, true).await.unwrap();
    assert!(
        matches!(r.status, Status::BadRequest | Status::NotFound),
        "NUL in path must not reach the filesystem, got {}",
        r.status.code()
    );
}