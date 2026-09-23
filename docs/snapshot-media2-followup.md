# Shared snapshots and native Media2 editing

Implementation and acceptance record, 2026-09-23. Follow-up to the oxvif 0.17
upgrade; the registry dependency remains 0.17.0.

## Implemented behavior

- Thumbnails, MJPEG frames, saved snapshots and health checks use
  `oxvif::health::snapshot`. The application passes both the camera address and
  snapshot URI, so the returned URI cannot redefine the credential boundary.
- The shared downloader validates image signatures, permits only the camera's
  host, rejects redirects and HTTPS downgrade, enforces a 10-second total
  deadline and caps bodies at 16 MiB. Digest failures do not fall back to Basic;
  Basic must be explicitly offered without Digest. No hand-written raw TCP
  authentication remains in OxDM.
- HTTPS snapshots now verify certificates. The old About checkbox and runtime
  bypass are removed. Existing configuration files still load; an obsolete
  `tls_strict = false` entry is ignored. About describes the active policy.
- MJPEG sends original bytes with the detected image MIME type. Both snapshot
  save flows retain JPEG/PNG/BMP format and use its corresponding extension.
  Signature recognition does not establish full image decoding.
- The encoder form loads native Media2 profiles, encoder configuration and
  options whenever the camera advertises that service. It preserves fractional
  FPS through read, edit and save, exposes the device's discrete FPS choices,
  and retains an existing value even if omitted from those choices. Without
  a discrete list it accepts finite, nonnegative decimal FPS. Invalid input
  disables Save and is checked again in the handler.
- Media1-only cameras retain integer FPS; fractional input is rejected both
  in the form and at the API boundary. Saving preserves the original Media1
  encoding interval, multicast and other unedited fields. A failed Media2
  write is not retried through Media1. Profile fallback remains visibly marked.
- A successful save reloads actual device values and reseeds the form, including
  GOV length, codec profile and changed options.

## Automated checks

Windows MSVC, Rust 1.97.0; published oxvif 0.17.0.

- `cargo fmt --check`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo build --locked`
- `cargo test --locked --all-targets`: 215 passed, one opt-in live test ignored.

New coverage checks shared-download authentication, unquoted Digest `qop`,
rejection without Basic downgrade, chunked images, signature-derived MIME,
invalid and oversized bodies, off-host destinations and redirects. Encoder
tests round-trip 12.5 and 29.97 FPS through the application and mock SOAP server,
preserve FPS while another field changes, reject nonfinite/negative/unsupported
rates, isolate selected encoders and preserve Media1 fields. Image-save tests
retain the detected extension. Existing i18n parity checks pass.

## Read-only camera acceptance

The installed CLI was 0.16.0; a local 0.17.0 CLI was built from the adjacent
oxvif checkout for discovery and diagnosis. Its Agent guide and command
descriptors were read before use. Credentials were read from the user's
existing `ONVIF/OnvifTestApp/OnvifTestApp.ini` and injected only into child
process environments. No passwords were placed in source, arguments or reports.
Both user-specified password groups were used; no new credentials were guessed.

The following cameras passed CLI diagnosis, then the application's opt-in
test for Media2 configuration/options and snapshot downloads on the first two
profiles. Frame rates are reported configuration values, not measured playback.

| Device | Address | Reported FPS, first two profiles | FPS options | Result |
| --- | --- | --- | --- | --- |
| GeoVision GV-SD2722-IR | 192.168.5.131 | 15 / 7 | integers 1–60 | Pass |
| GeoVision GV-EBD8800 | 192.168.6.105 | 20 / 15 | integers 1–20 | Pass |
| GeoVision GV-EBD4700 | 192.168.4.152 | 15 / 7 | integers 1–25 | Pass |
| GeoVision GV-TBL8710 | 192.168.5.178 | 15 / 15 | integers 1–15 | Pass |
| GeoVision GV-TBLP5800 | 192.168.5.67 | 30 / 30 | integers 1–30 | Pass |
| Uniview IPC3634SE-ADZK-WL-I0 | 192.168.4.248 | 23 / 25 | integers 1–25 | Pass |
| TVT TD-9521S4 | 192.168.5.69 | 25 / 7 | integers 1–25 | Pass |

The first five cameras authenticated with the requested lowercase-password
group; the Uniview and TVT cameras authenticated with the requested uppercase
group. Seven devices / fourteen profiles passed application reads and image
signature validation. No images were saved and no camera configuration was
changed.

Hanwha XND-C6083RV at 192.168.5.66 authenticated and returned Media2 settings
(first profile: 2 FPS, integer choices 1–30). Its snapshot failed the shared
image-signature check in both CLI diagnosis and the application. That opt-in
acceptance run failed, as expected from the observed response; it is not counted
among the seven successful devices. The cause of its invalid snapshot response
has not been diagnosed. Other sampled endpoints rejected these credentials or
were unavailable and are not evidence of compatibility.

## Remaining manual coverage

No tested camera advertised fractional FPS. Fractional read/write fidelity is
therefore established by mock round-trips, not a real-camera fractional Set.
Real encoder writes, desktop visual interaction, HTTPS camera certificate
acceptance and Linux/macOS execution have not been exercised in this follow-up.

To repeat the read-only acceptance, inject `OXDM_TEST_TARGET`,
`OXVIF_USERNAME` and `OXVIF_PASSWORD` into the process, then run:

```text
cargo test --locked --test snapshot_encoder_smoke live_camera_readonly_snapshot_and_encoder -- --ignored --nocapture
```

Do not include passwords in shell arguments. The test performs no Set calls.
