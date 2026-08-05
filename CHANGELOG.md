# Changelog

All notable changes to oxdm (the `oxvif-device-manager` binary) are documented
here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Changelog tracking starts at 0.1.5.

---

## [Unreleased]

### Changed
- **Upgraded to oxvif 0.15.0** (from crates.io). Of its breaking changes, one
  reached oxdm's source: `get_video_encoder_configuration_options` takes
  `config_token: &str` instead of `Option<&str>`, so `api.rs` and its single
  caller in `views/video_encoder.rs` drop the `Option`. The caller already
  passed `Some(&token)` — the `None` arm had never been reachable, which is the
  multi-sensor bug oxvif closed by removing it. The other four options getters
  that gained the same requirement are not called here. The displayed oxvif
  version is now 0.15.0.

### Fixed
- **The IO Control tab would have shown a red error on any camera without a
  DeviceIO endpoint.** oxvif 0.15 moves `GetDigitalInputs` onto the DeviceIO
  service, where the schema puts it; a device advertising no DeviceIO URL now
  fails *locally*, before the request is sent, with `Missing required field:
  DeviceIO service URL`. `is_action_unsupported` matched only the three
  `ActionNotSupported`-family fault texts a device answers with, so the new
  message fell through to the error banner instead of the soft "no IO hardware"
  empty state. It now matches that message too — on the whole field name, not on
  the `missing required field` prefix, so a genuine parse failure is still an
  error. No existing test could see this: `tests/io_control_smoke.rs` runs
  against oxvif's mock, which *does* advertise DeviceIO, so it stayed green
  through the upgrade.

---

## [0.3.0] - 2026-07-27

Headline: **the Quirks tab becomes something you point at a real camera**, not
only at a recorded clone — pick the operations, watch them run, and see what
moved since last time. Built on oxvif 0.14's selectable read surface. Alongside
it, three fixes to things that were quietly losing data or blocking a click.

### Added
- **Test a live camera from the Quirks tab** — point it at a real device and get
  the same quirk / parse results a recorded clone produces, without recording
  one. The tab was previously only reachable for a served clone; a real device
  now gets a test panel, while a served clone still renders its recorded
  analysis unchanged. The sweep is strictly read-only (`Get*` only), so it is
  safe against a production camera.
- **Per-operation selection** — pick individual operations, not just service
  zones: 52 operations across 7 collapsible zones with a tri-state zone box.
  Prerequisites cross zone boundaries (the PTZ preset/status reads need
  `GetProfiles` from Media; all four Imaging reads need `GetVideoSources`), so an
  implied operation shows as checked, disabled and dimmed with an `auto` badge
  naming the pick that pulled it in. The count beside Run is exactly what will
  execute. The selection persists to `~/.oxdm/quirk-surface.json`.
- **Progress** across all four phases (connecting, sweep, verify, diff), with an
  indeterminate state while the session is still being built.
- **Parse-verification layer** — each operation carries a badge for whether
  oxvif's own typed parser accepts the device's response, plus a banner listing
  the operations it cannot parse. This catches value/type quirks the structural
  SOAP diff is blind to, including on operations with no structural drift at
  all. (Landed after 0.2.0 but was not recorded here at the time.)
- **Diff a device's quirks against a saved baseline** — "Save as baseline" on the
  Quirks tab stores the current report to
  `~/.oxdm/quirk-baselines/<addr>.json`, and every later run renders what moved:
  newly drifting operations, ones that stopped drifting, and ones still drifting
  but in different places. Answers "did this firmware update change the camera?"
  and "are these two same-model cameras quirk-identical?" — the questions that
  stopped being answerable by eye once a sweep covered 52 operations. The
  Diagnostics tab has had this flow for health reports since 0.1.5; quirks had
  the report but no baseline. Uses oxvif 0.14's `QuirkReport::diff`.
- **Delete a saved mock** — the "Saved mocks" list rows gained a trash button
  (with a confirmation, since the recording cannot be recovered). Previously a
  recorded clone could only be opened, never removed, and the only way to clear
  one was to delete its directory under `~/.oxdm/clones/` by hand.

### Fixed
- **Editing one device's credentials could overwrite another's, or silently
  clear them.** The Edit Device dialog kept its username and password fields
  across open/close — only the name was reset — and skipped re-reading the
  device whenever that device's name was empty, which the dialog itself could
  produce (it trims, so a name of only spaces becomes empty). The dialog then
  opened showing the previous device's values and Save wrote them onto this one.
  When the leftover pair happened to be blank the save recorded "no
  credentials", which reads as working — the session falls back to the global
  credentials — right up until the next launch, when nothing had been written to
  the keychain. The fields are now keyed to the device they were loaded from and
  every exit clears them.
- **The right-click menu ran off the bottom of the screen.** Menu position was
  the raw click coordinate with no clamping, so right-clicking a device low in
  the sidebar put most of the menu below the viewport with no way to scroll to
  it. The position is now clamped against the viewport in CSS, and the menu has
  a max height. A click with room to spare lands exactly where it always did.

### Changed
- Results are grouped by service zone and default to showing only problems — a
  flat list was workable for a ~12-operation clone and is not for 52.
- Skipped operations are counted and coloured apart from failures. A fixed
  camera with no PTZ profiles now reports "no such path" instead of eight
  apparently broken commands.
- A device answering with a SOAP Fault (e.g. `NotAuthorized`) is shown as
  **declined** rather than as a parse failure — it is behaving correctly. Uses
  oxvif's new `ParseStatus::Faulted`.
- **Upgraded to oxvif 0.14.0** (from crates.io). Nothing in oxdm had to change
  for its three breaking changes: `FixtureStore::lookup` is never called here,
  `get_discovery_mode` is not used, and no caller gates on
  `SweepReport::is_complete`. The displayed oxvif version is now 0.14.0.

---

## [0.2.0] - 2026-07-24

Headline: **clone a real camera into an in-app mock and inspect its quirks** —
built on oxvif 0.13's metamorph — plus a raw-SOAP capture toggle in batch health.

### Added
- **Clone a camera into an in-app mock.** Right-click a device →
  **"Clone this camera"** records its standard read surface and serves it from
  an in-app mock (an offline replay server), then adds it to the device list
  labeled "mock". The clone drives through every existing tab with the real
  camera unplugged. Clones persist to `~/.oxdm/clones/`; the **"Saved mocks"**
  list in the Manual tab reopens them. Removing a mock device stops its server.
  Requires oxvif's `metamorph-server` feature (enabled in the default build).
- **Quirks tab** on a mock device — a git-style side-by-side diff of each
  recorded operation's response against oxvif's reference (baseline) response,
  with word-level intra-line highlighting, and export of the selected operations
  to timestamped JSON. Scope is structural (element presence), not ONVIF-schema
  conformance, and covers the standard read surface only.
- **"Write round-trip" toggle** in the batch health view (off by default). When
  enabled, the health check performs one non-destructive Set — it reads the
  first video-encoder configuration and writes it back unchanged — to catch
  devices that reject our serialized request body (a class of interop bug that
  read-only probes can't see). The single-device Diagnostics tab stays
  read-only.
- **"Capture SOAP" toggle** in the batch health view (off by default) — records
  the raw request/response of every check that *fails* into the exported report,
  so the exact evidence of why a brand rejected a call travels with the export.
  Only failing exchanges are stored, and WS-Security password/nonce are redacted.

### Changed
- **Upgraded to oxvif 0.13.0** (from crates.io) — the new clone/mock feature is
  built on oxvif's `metamorph-server`; the dependency moved off the local path to
  the published crate. The displayed oxvif version is now 0.13.0.

---

## [0.1.5] - 2026-07-09

### Changed
- **Upgraded to oxvif 0.12.0** (from crates.io). The batch and per-device health
  checks now consume oxvif's reshaped health report directly, so a check that
  *couldn't be verified* (auth-blocked) is no longer mistaken for a conformance
  failure, and the fragile client-side result re-parsing was removed.

### Added
- **Active liveness verification** in the health check — instead of only
  confirming the device answered each SOAP call, it now opens the RTSP stream,
  fetches the snapshot and validates it as a real image, and exercises Profile G
  recording search / replay.
- **Security probe** — flags a camera that serves data without requiring
  authentication.
- **"Force-verify undeclared services"** toggle in the batch health view —
  probes profile-gating services the device does not advertise and flags any
  that actually respond as under-declared.
- **"Export JUnit"** button in the batch health view — exports fleet results as
  JUnit XML for ingestion by CI systems and test dashboards, alongside the
  existing rich JSON bundle.
