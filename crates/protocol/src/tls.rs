use crate::ALPN;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::version::TLS13;
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use std::sync::Arc;
use rustls::crypto::WebPkiSupportedAlgorithms;
use sha2::{Digest, Sha256};

pub fn client_config(insecure_dev: bool) -> ClientConfig {
    let builder = ClientConfig::builder_with_protocol_versions(&[&TLS13]);
    let mut config = if insecure_dev {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureDevVerifier))
            .with_no_client_auth()
    } else {
        let roots = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        builder.with_root_certificates(roots).with_no_client_auth()
    };
    config.alpn_protocols = vec![ALPN.as_bytes().to_vec()];
    config.enable_early_data = false;
    config
}

pub fn pinned_client_config(cert_sha256: &str) -> ClientConfig {
    let builder = ClientConfig::builder_with_protocol_versions(&[&TLS13]);
    let algorithms = builder.crypto_provider().signature_verification_algorithms;
    let mut config = builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedVerifier {
            cert_sha256: cert_sha256.to_string(),
            algorithms
        }))
        .with_no_client_auth();
    config.alpn_protocols = vec![ALPN.as_bytes().to_vec()];
    config.enable_early_data = false;
    config
}

pub fn cert_fingerprint(der: &[u8]) -> String {
    Sha256::digest(der).iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug)]
struct PinnedVerifier {
    cert_sha256: String,
    algorithms: WebPkiSupportedAlgorithms
}
impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if cert_fingerprint(end_entity) == self.cert_sha256 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate does not match the fingerprint in the domain record".into()))
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

#[derive(Debug)]
struct InsecureDevVerifier;
impl ServerCertVerifier for InsecureDevVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}
