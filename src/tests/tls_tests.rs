use crate::{api::snapshot_client_builder, persist::ConfigFile};
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
fn snapshot_tls_defaults_apply_to_new_and_legacy_configs() {
    assert!(ConfigFile::default().tls_strict);
    for input in ["", "theme = 'dark'\nlocale = 'en'\n"] {
        let config: ConfigFile = toml::from_str(input).expect("legacy config should load");
        assert!(
            config.tls_strict,
            "omitted TLS setting must verify certificates"
        );
    }
}

#[test]
fn snapshot_tls_preserves_explicit_saved_choices() {
    for strict in [true, false] {
        let config: ConfigFile = toml::from_str(&format!("tls_strict = {strict}"))
            .expect("explicit TLS setting should load");
        assert_eq!(config.tls_strict, strict);
        let saved = toml::to_string(&config).expect("config should serialize");
        let loaded: ConfigFile = toml::from_str(&saved).expect("saved config should load");
        assert_eq!(loaded.tls_strict, strict);
    }
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
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: 8\r\nConnection: close\r\n\r\nsnapshot")
                .await
                .expect("response should write");
        }
    });
    let url = format!("https://localhost:{port}/snapshot");
    let strict = ConfigFile::default().tls_strict;
    let client = snapshot_client_builder(strict).no_proxy().build().unwrap();
    let error = client
        .get(&url)
        .send()
        .await
        .expect_err("untrusted certificate must fail");
    assert!(
        error.is_connect(),
        "failure must occur during connection: {error}"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 0);

    // A private CA works without disabling verification on native verifiers too.
    let trusted = snapshot_client_builder(strict)
        .no_proxy()
        .tls_certs_merge([reqwest::Certificate::from_der(ca.der().as_ref()).unwrap()])
        .build()
        .unwrap();
    let response = trusted
        .get(&url)
        .send()
        .await
        .expect("trusted localhost should connect");
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"snapshot");
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    let mismatched = format!("https://127.0.0.1:{port}/snapshot");
    let error = trusted
        .get(&mismatched)
        .send()
        .await
        .expect_err("wrong hostname must fail");
    assert!(
        error.is_connect(),
        "hostname failure must occur before HTTP: {error}"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    // The persisted opt-out remains available only when explicitly selected.
    let config: ConfigFile = toml::from_str("tls_strict = false").unwrap();
    let compatible = snapshot_client_builder(config.tls_strict)
        .no_proxy()
        .build()
        .unwrap();
    let response = compatible
        .get(&mismatched)
        .send()
        .await
        .expect("explicit opt-out should connect");
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"snapshot");
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    server.abort();
}
