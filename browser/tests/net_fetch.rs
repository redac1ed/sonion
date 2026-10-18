use registry::{Registry, RegistryServer};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sonion_browser::net::NetStack;
use sonion_protocol::SonionUrl;

fn certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let kp = rcgen::KeyPair::generate().unwrap();
    let p = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let c = p.self_signed(&kp).unwrap();
    (
        vec![c.der().clone()],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(kp.serialize_der())),
    )
}

#[tokio::test]
async fn end_to_end_named_fetch() {
    let site_tmp = tempfile::tempdir().unwrap();
    let site_root = site_tmp.path().join("site");
    std::fs::create_dir_all(&site_root).unwrap();
    std::fs::write(site_root.join("index.html"), "<h1>via browser net</h1>").unwrap();
    let (c1, k1) = certs();
    let site = sonion_server::Server::bind("127.0.0.1:0".parse().unwrap(), &site_root, c1, k1)
        .await
        .unwrap();
    let site_port = site.local_addr().unwrap().port();
    tokio::spawn(site.run());
    let reg_tmp = tempfile::tempdir().unwrap();
    let reg = Registry::open(reg_tmp.path()).unwrap();
    let (c2, k2) = certs();
    let reg_srv = RegistryServer::bind("127.0.0.1:0".parse().unwrap(), reg, c2, k2)
        .await
        .unwrap();
    let reg_port = reg_srv.local_addr().unwrap().port();
    tokio::spawn(reg_srv.run());
    let reg_url = SonionUrl::parse(&format!("sonion://localhost:{reg_port}/")).unwrap();
    let client = registry::client::RegistryClient::new(reg_url.clone(), true);
    client.create_account("tester", "hunter2pass").await.unwrap();
    let key = client.login("tester", "hunter2pass").await.unwrap();
    client
        .put_domain(&key, "browsertest.son", &[format!("127.0.0.1:{site_port}")], None)
        .await
        .unwrap();
    let stack = NetStack::new(Some(reg_url), None, true);
    let page = stack.fetch_page("sonion://browsertest.son/").await.unwrap();
    assert_eq!(page.status, 200);
    assert_eq!(page.body, b"<h1>via browser net</h1>");
    assert!(page.content_type.contains("html"));
}