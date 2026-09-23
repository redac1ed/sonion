use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use sonion_protocol::{fetch, Request, Response, SonionUrl, Status};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

fn dev_server_config(alpn: &str) -> Arc<rustls::ServerConfig> {
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    let mut config =
        rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.der().clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der())),
            )
            .unwrap();
    config.alpn_protocols = vec![alpn.as_bytes().to_vec()];
    Arc::new(config)
}
// accept tls connection -> request head -> response thru channel
async fn spawn_server(
    response: Vec<u8>,
    alpn: &str,
) -> (u16, tokio::sync::oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn({
        let acceptor = TlsAcceptor::from(dev_server_config(alpn));
        async move {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let Ok(mut tls) = acceptor.accept(tcp).await else {
                return;
            };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let Ok(n) = tls.read(&mut tmp).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            if tls.write_all(&response).await.is_ok() {
                let _ = tls.flush().await;
            }
            let _ = tx.send(buf);
        }
    });
    (port, rx)
}

fn localhost_url(port: u16) -> SonionUrl {
    SonionUrl::parse(&format!("sonion://localhost:{port}/")).unwrap()
}

#[tokio::test]
async fn fetch_over_tls13_insecure_dev() {
    let mut resp = Response::new(Status::Ok, b"hello over tls".to_vec());
    resp.set_header("Content-Type", "text/plain");
    let (port, rx) = spawn_server(resp.serialize(), "sonion/1").await;
    let got = fetch(&localhost_url(port), true).await.unwrap();
    assert_eq!(got.status, Status::Ok);
    assert_eq!(got.body, b"hello over tls");
    assert_eq!(got.get_header("Content-Type"), Some("text/plain"));
    let req = Request::parse(&rx.await.unwrap()).unwrap(); // for good format bytes
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/");
    assert!(req
        .headers
        .iter()
        .any(|(n, v)| n == "Host" && v == "localhost"));
}
#[tokio::test]
async fn fetch_chunked_over_tls() {
    let mut resp = Response::new(Status::Ok, b"konichiwa son".to_vec());
    resp.set_header("Content-Type", "text/html");
    let (port, _rx) = spawn_server(resp.serialize_chunked(4), "sonion/1").await;
    let got = fetch(&localhost_url(port), true).await.unwrap();
    assert!(got.is_chunked());
    assert_eq!(got.body, b"konichiwa son");
}
#[tokio::test]
async fn rejects_self_signed_without_insecure_dev() {
    let resp = Response::new(Status::Ok, b"nope".to_vec());
    let (port, _rx) = spawn_server(resp.serialize(), "sonion/1").await;
    let result = fetch(&localhost_url(port), false).await;
    assert!(result.is_err());
}
#[tokio::test]
async fn rejects_alpn_mismatch() {
    let resp = Response::new(Status::Ok, b"nope".to_vec());
    let (port, _rx) = spawn_server(resp.serialize(), "http/1.1").await;
    let result = fetch(&localhost_url(port), true).await;
    assert!(result.is_err(), "fetch must fail when ALPN is not sonion/1");
    assert!(!matches!(result, Ok(_))); // must not be parsed response
}
