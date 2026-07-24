# Changelog

All notable changes to oxdm (the `oxvif-device-manager` binary) are documented
here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Changelog tracking starts at 0.1.5.

---

## [Unreleased]

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
