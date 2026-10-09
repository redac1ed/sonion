use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use sonion_protocol::tls::client_config;
use sonion_protocol::{Client, Request, Response, SonionUrl, Status};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

fn test_certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let kp = rcgen::KeyPair::generate().unwrap();
    let p = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let c = p.self_signed(&kp).unwrap();
    (vec![c.der().clone()], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(kp.serialize_der())))
}

fn server_config() -> Arc<rustls::ServerConfig> {
    let (certs, key) = test_certs();
    let mut config = rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13]).with_no_client_auth().with_single_cert(certs, key).unwrap();
    config.alpn_protocols = vec![b"sonion/1".to_vec()];
    Arc::new(config)
}

async fn spawn_keepalive_server(
    conn_count: Arc<AtomicUsize>,
    req_count: Arc<AtomicUsize>,
) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let acceptor = TlsAcceptor::from(server_config());
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            conn_count.fetch_add(1, Ordering::SeqCst);
            let acceptor = acceptor.clone();
            let req_count = req_count.clone();
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let mut buf = Vec::new();
                let mut tmp = [0u8; 8192];
                loop {
                    let Ok(n) = tls.read(&mut tmp).await else {
                        return;
                    };
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    while buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        req_count.fetch_add(1, Ordering::SeqCst);
                        let end = buf
                            .windows(4)
                            .position(|w| w == b"\r\n\r\n")
                            .unwrap()
                            + 4;
                        buf.drain(..end);
                        let mut resp = Response::new(Status::Ok, b"pong".to_vec());
                        resp.set_header("Content-Type", "text/plain");
                        if tls.write_all(&resp.serialize()).await.is_err() {
                            return;
                        }
                        let _ = tls.flush().await;
                    }
                }
            });
        }
    });
    port
}

fn url(port: u16, path: &str) -> SonionUrl {
    SonionUrl::parse(&format!("sonion://localhost:{port}{path}")).unwrap()
}

#[tokio::test]
async fn pooled_client_reuses_connection() {
    let conns = Arc::new(AtomicUsize::new(0));
    let reqs = Arc::new(AtomicUsize::new(0));
    let port = spawn_keepalive_server(conns.clone(), reqs.clone()).await;
    let client = Client::new(true);
    for _ in 0..5 {
        let req = Request::get("/ping").header("Host", "localhost");
        let resp = client.send(&url(port, "/ping"), &req).await.unwrap();
        assert_eq!(resp.status, Status::Ok);
        assert_eq!(resp.body, b"pong");
    }
    assert_eq!(reqs.load(Ordering::SeqCst), 5);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(conns.load(Ordering::SeqCst), 1, "keep-alive must reuse one connection");
    assert_eq!(client.idle_connection_count().await, 1);
    client.close_all().await;
    assert_eq!(client.idle_connection_count().await, 0);
}
#[tokio::test]
async fn connection_close_disables_pooling() {
    let conns = Arc::new(AtomicUsize::new(0));
    let reqs = Arc::new(AtomicUsize::new(0));
    let port = spawn_keepalive_server(conns.clone(), reqs.clone()).await;
    let client = Client::new(true);
    for _ in 0..3 {
        let req = Request::get("/ping").header("Host", "localhost").header("Connection", "close");
        let resp = client.send(&url(port, "/ping"), &req).await.unwrap();
        assert_eq!(resp.status, Status::Ok);
    }
    assert_eq!(client.idle_connection_count().await, 0);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(conns.load(Ordering::SeqCst), 3);
}
#[tokio::test]
async fn raw_tls_head_then_two_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let acceptor = TlsAcceptor::from(server_config());
    tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(tcp).await.unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut answered = 0;
        loop {
            let n = tls.read(&mut tmp).await.unwrap();
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&tmp[..n]);
            while buf.windows(4).any(|w| w == b"\r\n\r\n") {
                let end = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                buf.drain(..end);
                answered += 1;
                let mut resp = Response::new(Status::Ok, format!("answer-{answered}").into_bytes());
                resp.set_header("Content-Type", "text/plain");
                tls.write_all(&resp.serialize()).await.unwrap();
                tls.flush().await.unwrap();
            }
        }
    });
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config(true)));
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let name = ServerName::try_from("localhost".to_string()).unwrap();
    let mut tls = connector.connect(name, tcp).await.unwrap();
    let r1 = Request::get("/one").header("Host", "localhost").serialize();
    tls.write_all(&r1).await.unwrap();
    tls.flush().await.unwrap();
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n")
        || buf.len()
            < buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4 + 8
    {
        let n = tls.read(&mut tmp).await.unwrap();
        buf.extend_from_slice(&tmp[..n]);
    }
    let r2 = Request::get("/two").header("Host", "localhost").serialize();
    tls.write_all(&r2).await.unwrap();
    tls.flush().await.unwrap();
    let need = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4 + 8;
    while buf.len() < need + 40 {
        let n = tls.read(&mut tmp).await.unwrap();
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(8).filter(|w| w == b"answer-2").count() > 0 {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf);
    assert!(text.contains("answer-1"));
    assert!(text.contains("answer-2"));
}