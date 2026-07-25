# Changelog

All notable changes to oxdm (the `oxvif-device-manager` binary) are documented
here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Changelog tracking starts at 0.1.5.

---

## [Unreleased]

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

### Changed
- Results are grouped by service zone and default to showing only problems — a
  flat list was workable for a ~12-operation clone and is not for 52.
- Skipped operations are counted and coloured apart from failures. A fixed
  camera with no PTZ profiles now reports "no such path" instead of eight
  apparently broken commands.
- A device answering with a SOAP Fault (e.g. `NotAuthorized`) is shown as
  **declined** rather than as a parse failure — it is behaving correctly. Uses
  oxvif's new `ParseStatus::Faulted`.

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
