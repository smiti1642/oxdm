# OxDM: oxvif 0.16 upgrade

Date: 2026-09-09

## Scope

Upgrade the published oxvif dependency from 0.15 to 0.16 and expose discovery
failures through the existing scan error toast. The implementation was first
validated at application version 0.4.0; publication is now prepared as 0.4.1.
Preserve the three-round progressive scan.
Network-interface selection and private-CA settings are follow-up features.

## Implementation and verification

- [x] Record `cargo outdated` and `cargo audit` before changing dependencies.
- [x] Update both oxvif dependency entries and the lockfile from crates.io;
  update the displayed/baseline oxvif version and current development guidance.
  Confirm the resolved version and toolchain compatibility (oxvif requires
  Rust 1.88 or newer).
- [x] Call `probe_result` and propagate its error through the existing UI path.
  Do not treat an empty successful result as an error. Upstream still ignores
  some send/receive errors, so this is not complete network diagnosis.
- [x] Preserve an existing discovered device's address when still advertised:
  oxvif 0.16 merges and sorts XAddrs, which can otherwise change the address
  chosen by OxDM. Test reordered addresses, a withdrawn address, and new devices.
- [x] Run `cargo outdated` and `cargo audit` after resolution; apply compatible
  security fixes and document remaining advisories or upgrades requiring
  separate work. Avoid unrelated major dependency migrations.
- [x] Run formatting, `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  and `cargo test` (including i18n, PTZ, imaging, IO, recording, health and
  baseline tests). Review the final diff and update the changelog.

## Acceptance limits

Automated mock tests do not establish real-camera multicast reachability or
cross-platform GUI behavior. Manual acceptance should cover a normal scan,
an empty network, unavailable interfaces, and a camera advertising multiple
XAddrs. Record any acceptance not exercised here explicitly.

## Results

### Dependency checks

The initial full `cargo outdated` completed successfully. It reported many
transitive updates and warned that the keyring 4.2 feature set differs from
the current keyring 3 configuration. The post-upgrade direct-dependency report
is saved in [oxvif-0.16-outdated.txt](oxvif-0.16-outdated.txt), generated with
`cargo outdated --depth 1`. Neither invocation modifies the project manifest.

The initial `cargo audit` failed with four vulnerability entries:

| Dependency | Before | After | Advisory |
| --- | --- | --- | --- |
| h2 | 0.4.13 | 0.4.19 | RUSTSEC-2026-0258 |
| quick-xml via wayland-scanner | 0.39.2 | 0.41.0 (shared with oxvif) | RUSTSEC-2026-0194, RUSTSEC-2026-0195 |
| webbrowser | 1.2.0 | 1.2.4 | RUSTSEC-2026-0257 |

Updated wayland-scanner to 0.31.11 to remove the vulnerable XML dependency.
Also updated anyhow 1.0.102 → 1.0.104, memmap2 0.9.10 → 0.9.11 and rand
0.8.5 → 0.8.8 to resolve their unsoundness warnings, and chacha20 0.10.0 →
0.10.2 to remove the yanked version. The registry oxvif version is 0.16.0;
there is no path dependency on the adjacent development checkout.

The post-upgrade `cargo audit --json` exited successfully: **0 vulnerabilities,
5 warnings**, with no ignored advisory configuration added. Raw evidence is in
[oxvif-0.16-audit.json](oxvif-0.16-audit.json). Advisory database commit:
`bf25f6575a93a35f30796c65c0ed91bee7fa19fd` (updated 2026-09-08).

| Remaining warning | Dependency path / reason retained |
| --- | --- |
| fxhash 0.2.1 — RUSTSEC-2025-0057, unmaintained | Dioxus → wry → kuchikiki → selectors |
| paste 1.0.15 — RUSTSEC-2024-0436, unmaintained | Image codecs → rav1e |
| proc-macro-error 1.0.4 — RUSTSEC-2024-0370, unmaintained | GTK/glib macros in desktop dependencies |
| glib 0.18.5 — RUSTSEC-2024-0429, unsound | Linux GTK stack requires 0.18; advisory fix is >=0.20 |
| rand 0.7.3 — RUSTSEC-2026-0097, unsound | wry → kuchikiki → selectors → phf_codegen 0.8 → phf_generator 0.8; no patched 0.7 release |

These warnings remain open upstream dependency work, not a claim of a clean
security inventory. The glib issue is in the Linux dependency graph; local
Windows compilation does not validate that platform. No reachability-based
suppression was applied to the rand warning.

`cargo outdated` still lists compatible direct updates for async-trait,
reqwest, serde, serde_json, time, tokio and toml. The separately pinned Dioxus
0.7.9 also has 0.7.10 available, while base64 0.23, dirs 7 and keyring 4 require
manifest changes. These routine/framework migrations are deferred to keep this
change focused on oxvif and the audit findings.

### Validation

Local toolchain: rustc 1.97.0, Windows x86-64 MSVC. This exceeds oxvif's 1.88
requirement; the complete application was not tested on Rust 1.88.

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed, including a
  final check after adding the previous-version baseline regression.
- `cargo build --locked`: passed.
- `cargo test --locked -- --quiet`: **174 passed, 0 failed, 0 ignored**:
  127 binary unit tests; integration targets healthtab 1, imaging_focus 13,
  io_control 10, ptz_absolute 13, recordings 10. The integration targets also
  include shared session tests through their existing source modules.
- `git diff --check`: passed. The initial validation used application version
  0.4.0. The subsequent 0.4.1 version/documentation preparation and packaging
  checks are tracked in [release-0.4.1.md](release-0.4.1.md).
- Real-camera multicast and GUI acceptance: not exercised.
- Linux/macOS build and runtime acceptance: not exercised locally.
