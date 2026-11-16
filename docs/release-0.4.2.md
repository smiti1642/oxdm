# OxDM 0.4.2

Released: 2026-10-23.

This security patch keeps the published oxvif 0.16 dependency and updates the
application's own locked TLS/QUIC graph. It does not require an oxvif API migration.

- Update rustls 0.23.46, rustls-webpki 0.103.16, webpki-root-certs 1.0.10,
  quinn-proto 0.11.18 and the required aws-lc graph for the reviewed TLS/QUIC
  advisories, including the compromised emSign intermediate keys.
- Verify HTTPS snapshot certificates and hostnames by default on fresh installs
  and when the tls_strict setting is absent. Existing explicit false settings
  remain opt-outs. Before enabling verification for a private camera CA, install
  that CA in the operating-system trust store. The setting affects snapshots;
  ONVIF SOAP retains verified transport.
- Replace yanked clipboard-win 5.4.1 with 5.4.3. Update async-trait and the
  CA-signed TLS test fixture for current Clippy and Windows compatibility.
- Exercise Linux and Windows checks in CI and preserve Cargo.lock when bundling.

The reviewed upstream maintenance and unsoundness warnings remain visible;
notably the Linux glib 0.18 defect has no compatible patched release. The source
review found no selected application/dependency calls to the affected iterator,
which is static evidence rather than proof of absence. See the
[dependency review](https://github.com/smiti1642/oxdm/blob/v0.4.2/docs/dependency-audit-2026-10-23.md).
No advisory ignores were added. Keep OS trust/revocation data current.

Install source with `cargo install oxvif-device-manager --version 0.4.2 --locked`,
or use the GitHub Release bundles: macOS ARM64 DMG, Windows x86-64 MSI/portable
ZIP and Ubuntu x86-64 DEB. Bundles remain unsigned.
