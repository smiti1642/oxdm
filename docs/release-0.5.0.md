# OxDM 0.5.0

2026-11-19 · [繁體中文](https://github.com/smiti1642/oxdm/blob/v0.5.0/docs/release-0.5.0_zh.md)

- **Run oxvif commands inside OxDM.** The console supports syntax highlighting,
  command and parameter completion, and contextual help in English,
  Traditional Chinese and Russian. Use a dark terminal or collapsible result
  cards. Select a camera for commands such as `device info` and `media profiles`.
- **Query a camera by address.** Run `inspect 192.168.1.100` and enter the
  camera's account in the credential form. Use `--read profiles` to find
  tokens and `--read stream-uri --profile TOKEN` to read the live RTSP URI.
  The query uses temporary credentials.
- **Share a diagnostic report.** Use `diagnose --share` to export a JSON
  report that omits credentials, device addresses and local paths.
- **Recover event monitoring.** The device Events page reconnects after
  interruption. It can read stored events on compatible devices. With current
  state only, OxDM requests a snapshot and marks the missing interval. Live
  `events watch` can also be started and stopped from the console.
- **Updated Media2 and Quirks.** Edit advertised fractional frame rates on
  Media2 cameras. Quirks preserves independent results for recordings with
  colliding keys and exposes ambiguous baseline comparisons.

Local Windows checks include the MSI's bundled CLI and the portable ZIP's
executable pair. Hardware checks cover
reconnecting without stored event history and one-shot queries on a GeoVision
camera. Reading stored event history still requires compatible hardware.
See the [changelog](https://github.com/smiti1642/oxdm/blob/v0.5.0/CHANGELOG.md).

OxDM 0.5.0 targets oxvif 0.18.0. HTTPS snapshots require certificate and
hostname verification. For a private camera CA, trust the CA in the operating
system before connecting.

The release targets a Windows x86-64 MSI and portable ZIP, an Apple Silicon
DMG and an Ubuntu x86-64 DEB. Keep both executables together in the Windows
portable ZIP. Source installs use the `oxvif-device-manager` crate.
