//! Application download and Media2 editing regression tests; no camera needed.
#[path = "../src/api.rs"]
#[allow(dead_code, unused_imports)]
mod api;
#[path = "../src/sessions.rs"]
#[allow(dead_code, unused_imports)]
mod sessions;
#[path = "../src/state.rs"]
#[allow(dead_code, unused_imports)]
mod state;

use oxvif::mock::MockServer;
use state::Credentials;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn endpoint(responses: Vec<Vec<u8>>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/snapshot?ticket=private",
        listener.local_addr().unwrap()
    );
    let handle = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut request = Vec::new();
            loop {
                let mut buffer = [0; 2048];
                let n = stream.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buffer[..n]);
                if request.windows(4).any(|s| s == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 32768);
            }
            requests.push(String::from_utf8(request).unwrap());
            stream.write_all(&response).await.unwrap();
        }
        requests
    });
    (url, handle)
}

fn response(body: &[u8]) -> Vec<u8> {
    let mut data = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();
    data.extend_from_slice(body);
    data
}

#[tokio::test]
async fn preview_uses_image_signature_and_mjpeg_receives_original_bytes() {
    let body = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x00\x00\x00\x00";
    let (url, server) = endpoint(vec![response(body), response(body)]).await;
    let creds = Credentials::default();
    let image = api::fetch_snapshot(&url, &url, &creds).await.unwrap();
    assert_eq!(image.bytes, body);
    assert_eq!(image.mime_type(), "image/png");
    let uri = api::fetch_snapshot_data_uri(&url, &url, &creds)
        .await
        .unwrap();
    use base64::Engine;
    let payload = uri.strip_prefix("data:image/png;base64,").unwrap();
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .unwrap(),
        body
    );
    assert_eq!(server.await.unwrap().len(), 2);
}

#[tokio::test]
async fn digest_failure_never_sends_basic_and_errors_hide_secrets() {
    let unauthorized = b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"camera\"\r\nWWW-Authenticate: Digest realm=\"camera\",nonce=\"fixed\",qop=\"auth\",algorithm=MD5\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    let (url, server) = endpoint(vec![unauthorized.to_vec(), unauthorized.to_vec()]).await;
    let creds = Credentials {
        username: "operator".into(),
        password: "test-secret".into(),
    };
    let error = api::fetch_snapshot(&url, &url, &creds).await.err().unwrap();
    assert!(
        !error.contains("private") && !error.contains("test-secret") && !error.contains("fixed")
    );
    let requests = server.await.unwrap();
    assert!(!requests[0].contains("Authorization:"));
    assert!(requests[1].contains("Authorization: Digest "));
    assert!(requests[1].contains("qop=auth") && !requests[1].contains("qop=\"auth\""));
    assert!(!requests.iter().any(|r| r.contains("Authorization: Basic")));
}

#[tokio::test]
async fn basic_is_used_only_when_offered_and_chunked_images_work() {
    let unauthorized=b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"camera\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    let chunked=b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n\xff\xd8\xff\xff\xd9\r\n0\r\n\r\n";
    let (url, server) = endpoint(vec![unauthorized.to_vec(), chunked.to_vec()]).await;
    let creds = Credentials {
        username: "operator".into(),
        password: "test-secret".into(),
    };
    assert_eq!(
        api::fetch_snapshot(&url, &url, &creds).await.unwrap().bytes,
        b"\xff\xd8\xff\xff\xd9"
    );
    let requests = server.await.unwrap();
    assert!(!requests[0].contains("Authorization:"));
    assert!(requests[1].contains("Authorization: Basic "));
}

#[tokio::test]
async fn invalid_or_oversized_images_are_rejected() {
    for reply in [
        response(b"<html>login</html>"),
        b"HTTP/1.1 200 OK\r\nContent-Length: 16777217\r\nConnection: close\r\n\r\n".to_vec(),
    ] {
        let (url, server) = endpoint(vec![reply]).await;
        assert!(api::fetch_snapshot(&url, &url, &Credentials::default())
            .await
            .is_err());
        server.await.unwrap();
    }
}

#[tokio::test]
async fn snapshot_cannot_leave_device_host_or_follow_redirects() {
    let (url,server)=endpoint(vec![b"HTTP/1.1 302 Found\r\nLocation: http://localhost/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()]).await;
    assert!(api::fetch_snapshot(
        "http://different.invalid/onvif",
        &url,
        &Credentials::default()
    )
    .await
    .is_err());
    assert!(api::fetch_snapshot(&url, &url, &Credentials::default())
        .await
        .is_err());
    assert_eq!(server.await.unwrap().len(), 1);
}

#[tokio::test]
async fn media2_fractional_frame_rate_survives_read_edit_save_and_readback() {
    let server = MockServer::start().await.unwrap();
    let creds = Credentials::default();
    let (pick, mut settings) = api::get_encoder_settings(server.device_url(), &creds, None)
        .await
        .unwrap();
    assert!(settings.is_media2());
    assert!(!pick.fell_back);
    settings
        .config
        .rate_control
        .as_mut()
        .unwrap()
        .frame_rate_limit = 12.5;
    api::save_encoder_settings(server.device_url(), &creds, &settings)
        .await
        .unwrap();
    let (_, mut settings) = api::get_encoder_settings(server.device_url(), &creds, None)
        .await
        .unwrap();
    assert_eq!(
        settings
            .config
            .rate_control
            .as_ref()
            .unwrap()
            .frame_rate_limit,
        12.5
    );
    assert_eq!(settings.parse_frame_rate("12.5"), Some(12.5));
    settings.config.quality = 4.0;
    api::save_encoder_settings(server.device_url(), &creds, &settings)
        .await
        .unwrap();
    let (_, again) = api::get_encoder_settings(server.device_url(), &creds, None)
        .await
        .unwrap();
    assert_eq!(again.config.rate_control.unwrap().frame_rate_limit, 12.5);
    for invalid in ["NaN", "inf", "-1", "", "12.75"] {
        assert_eq!(settings.parse_frame_rate(invalid), None);
    }
}

#[tokio::test]
async fn media1_rejects_fractional_fps_and_preserves_unedited_fields() {
    let server = MockServer::start().await.unwrap();
    let session = oxvif::OnvifSession::builder(server.device_url())
        .build()
        .await
        .unwrap();
    let token = session.get_profiles().await.unwrap()[0]
        .video_encoder_token
        .clone()
        .unwrap();
    let original = session
        .get_video_encoder_configuration(&token)
        .await
        .unwrap();
    let mut settings = api::EncoderSettings {
        config: api::to_video_encoder_configuration2(&original),
        options: Default::default(),
        media1: Some(original.clone()),
    };
    assert_eq!(settings.parse_frame_rate("12.5"), None);
    assert_eq!(settings.parse_frame_rate("12"), Some(12.0));
    settings
        .config
        .rate_control
        .as_mut()
        .unwrap()
        .frame_rate_limit = 12.5;
    assert!(
        api::save_encoder_settings(server.device_url(), &Credentials::default(), &settings)
            .await
            .is_err()
    );
    settings
        .config
        .rate_control
        .as_mut()
        .unwrap()
        .frame_rate_limit = 12.0;
    api::save_encoder_settings(server.device_url(), &Credentials::default(), &settings)
        .await
        .unwrap();
    let actual = session
        .get_video_encoder_configuration(&token)
        .await
        .unwrap();
    let mut expected = original;
    expected.rate_control.as_mut().unwrap().frame_rate_limit = 12;
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn media2_edits_only_the_selected_profiles_encoder() {
    let server = MockServer::start().await.unwrap();
    let creds = Credentials::default();
    let session = oxvif::OnvifSession::builder(server.device_url())
        .build()
        .await
        .unwrap();
    let profiles = session.get_profiles_media2().await.unwrap();
    let (_, first) =
        api::get_encoder_settings(server.device_url(), &creds, Some(&profiles[0].token))
            .await
            .unwrap();
    let (pick, mut second) =
        api::get_encoder_settings(server.device_url(), &creds, Some(&profiles[1].token))
            .await
            .unwrap();
    assert!(!pick.fell_back);
    assert_eq!(Some(&pick.token), profiles[1].video_encoder_token.as_ref());
    assert_ne!(first.config.token, second.config.token);
    second.options.frame_rates = vec![29.97, 12.5];
    assert_eq!(second.parse_frame_rate("29.97"), Some(29.97));
    second
        .config
        .rate_control
        .as_mut()
        .unwrap()
        .frame_rate_limit = 29.97;
    api::save_encoder_settings(server.device_url(), &creds, &second)
        .await
        .unwrap();
    let (_, actual) =
        api::get_encoder_settings(server.device_url(), &creds, Some(&profiles[1].token))
            .await
            .unwrap();
    assert_eq!(actual.config.rate_control.unwrap().frame_rate_limit, 29.97);
    let (_, untouched) =
        api::get_encoder_settings(server.device_url(), &creds, Some(&profiles[0].token))
            .await
            .unwrap();
    assert_eq!(
        untouched.config.rate_control.unwrap().frame_rate_limit,
        first.config.rate_control.unwrap().frame_rate_limit
    );
    let (fallback, _) =
        api::get_encoder_settings(server.device_url(), &creds, Some("missing-profile"))
            .await
            .unwrap();
    assert!(fallback.fell_back);
}

/// Opt-in read-only acceptance. Inject credentials into the test process;
/// never put them in command arguments or test output. No Set calls here.
#[tokio::test]
#[ignore = "requires OXDM_TEST_TARGET and environment-injected camera credentials"]
async fn live_camera_readonly_snapshot_and_encoder() {
    let addr = std::env::var("OXDM_TEST_TARGET").unwrap();
    let creds = Credentials {
        username: std::env::var("OXVIF_USERNAME").unwrap(),
        password: std::env::var("OXVIF_PASSWORD").unwrap(),
    };
    let profiles = api::get_profiles(&addr, &creds)
        .await
        .expect("read profiles");
    for profile in profiles.iter().take(2) {
        let (_, settings) = api::get_encoder_settings(&addr, &creds, Some(&profile.token))
            .await
            .expect("read encoder");
        println!(
            "profile={} media2={} fps={:?} supported={:?}",
            profile.token,
            settings.is_media2(),
            settings
                .config
                .rate_control
                .as_ref()
                .map(|r| r.frame_rate_limit),
            settings.options.frame_rates
        );
        let snap = api::get_snapshot_uri(&addr, &creds, &profile.token)
            .await
            .expect("get snapshot URI");
        let url = api::resolve_snapshot_url(&addr, &snap.uri);
        let image = api::fetch_snapshot(&addr, &url, &creds)
            .await
            .expect("download snapshot");
        println!("snapshot={} bytes={}", image.format, image.bytes.len());
    }
}
