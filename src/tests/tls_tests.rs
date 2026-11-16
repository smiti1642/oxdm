use crate::{api::fetch_snapshot, persist::ConfigFile, state::Credentials};
use oxvif::health::snapshot::{self, SnapshotOptions};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{
    rustls::{pki_types::PrivatePkcs8KeyDer, ServerConfig},
    TlsAcceptor,
};

#[test]
fn snapshot_tls_loads_legacy_configs_without_a_bypass() {
    let config: ConfigFile =
        toml::from_str("theme = 'dark'\ntls_strict = false").expect("legacy config should load");
    assert_eq!(config.theme, "dark");
    let saved = toml::to_string(&config).expect("config should serialize");
    assert!(!saved.contains("tls_strict"));
}

#[tokio::test]
async fn snapshot_tls_checks_trust_and_hostname_before_sending_http() {
    let mut ca_params = rcgen::CertificateParams::default();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    ca_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "OxDM TLS test CA");
    let ca_key = rcgen::KeyPair::generate().expect("test CA key should generate");
    let ca = ca_params.self_signed(&ca_key).expect("test CA should sign");
    let issuer = rcgen::Issuer::from_params(&ca_params, &ca_key);
    let mut server_params = rcgen::CertificateParams::new(vec!["localhost".into()])
        .expect("test server names should parse");
    server_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "localhost");
    server_params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    server_params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    server_params.use_authority_key_identifier_extension = true;
    let server_key = rcgen::KeyPair::generate().expect("test server key should generate");
    let certificate = server_params
        .signed_by(&server_key, &issuer)
        .expect("CA should sign the test server certificate");
    let tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.der().clone()],
            PrivatePkcs8KeyDer::from(server_key.serialize_der()).into(),
        )
        .expect("test TLS server should configure");
    let acceptor = TlsAcceptor::from(Arc::new(tls));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener should bind");
    let port = listener
        .local_addr()
        .expect("listener should have a port")
        .port();
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&requests);
    let server = tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.expect("loopback should accept");
            let Ok(mut stream) = acceptor.accept(socket).await else {
                continue;
            };
            let mut buffer = [0; 4096];
            let mut request = Vec::new();
            loop {
                let size = stream.read(&mut buffer).await.expect("request should read");
                if size == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..size]);
                if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    break;
                }
            }
            if request.is_empty() {
                continue;
            }
            observed.fetch_add(1, Ordering::SeqCst);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: 5\r\nConnection: close\r\n\r\n\xff\xd8\xff\xff\xd9")
                .await
                .expect("response should write");
        }
    });
    let url = format!("https://localhost:{port}/snapshot");
    assert!(fetch_snapshot(&url, &url, &Credentials::default())
        .await
        .is_err());
    assert_eq!(requests.load(Ordering::SeqCst), 0);

    let options = SnapshotOptions {
        ca_certificates: vec![ca.pem().into_bytes()],
        ..SnapshotOptions::default()
    };
    let (image, format) = snapshot::fetch(&url, &url, None, &options)
        .await
        .expect("trusted localhost should connect");
    assert_eq!(image, b"\xff\xd8\xff\xff\xd9");
    assert_eq!(format, "jpeg");
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    let mismatched = format!("https://127.0.0.1:{port}/snapshot");
    assert!(snapshot::fetch(&mismatched, &mismatched, None, &options)
        .await
        .is_err());
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    server.abort();
}
