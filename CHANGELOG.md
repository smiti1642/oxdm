# Changelog

All notable changes to oxdm (the `oxvif-device-manager` binary) are documented
here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Changelog tracking starts at 0.1.5.

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
