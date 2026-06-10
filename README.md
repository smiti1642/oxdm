# OxDM — ONVIF Device Manager

**OxDM** (*oxvif Device Manager*) is a modern, cross-platform take on the
classic **ONVIF Device Manager (ODM)** — built in Rust with
[Dioxus](https://dioxuslabs.com/) on top of the
[`oxvif`](https://crates.io/crates/oxvif) ONVIF client library — for
discovering and managing ONVIF IP cameras.

> **Status:** early (v0.1.0). Core device management works end-to-end against
> real cameras and the `oxvif` mock server. Bundles are unsigned for now, so
> the OS may warn about an unidentified developer on first launch.

## Downloads

Prebuilt bundles are attached to each
[GitHub Release](https://github.com/smiti1642/oxdm/releases):

| Platform | Asset |
|----------|-------|
| macOS (Apple Silicon) | `oxdm-<version>-macos-aarch64.dmg` |
| Windows (x64) | `oxdm-<version>-windows-x86_64.msi` |
| Linux (x64) | `oxdm-<version>-linux-x86_64.AppImage` |

> **macOS is Apple Silicon (`aarch64`) only.** An `aarch64` build will not run
> on an Intel Mac — on Intel, build from source (see below). Apple Silicon has
> been standard since 2020 and macOS itself is sunsetting Intel, so a separate
> Intel build isn't shipped.

## Features

- **Discovery** — WS-Discovery scan of the local network, plus manually-added
  devices. Discovered devices persist across restarts.
- **Live video** — always-on MJPEG snapshot stream, or RTSP (H.264/H.265) via a
  bundled go2rtc bridge with H.265 → H.264 transcode + MSE fallback.
- **Snapshots** — save a JPEG from any profile thumbnail or the Live Video view.
- **Device settings** — identification/scopes; network (hostname, IPv4 +
  IPv6 manual interfaces, MTU, DNS, NTP, gateway, protocols); system time
  (with PC sync + timezone/DST); user management (CRUD); and maintenance
  (reboot / factory reset, both confirm-gated).
- **Media** — profile create/delete, video-encoder configuration (H.264 +
  H.265 — H.265 is auto-routed through Media2), imaging controls
  (brightness/contrast plus manual exposure / WB gains / focus limits), and
  OSD CRUD.
- **PTZ** — preset CRUD, continuous/absolute moves, home position.
- **Events** — live PullPoint subscription with a scrolling, filterable log.
- **Diagnostics** — on-demand ONVIF health check (Pass/Warn/Fail/Skip per
  service, plus a Profile S/T/G verdict), with a "Save as baseline" button
  and an automatic diff against the saved baseline on the next run — flips
  to FAIL, added/removed checks, and checks that slowed down by ≥ 2× are
  all flagged in the per-device baseline diff.
- **Niceties** — three themes (Dark / Light / Classic), English / 繁體中文 /
  Русский locales, keyboard shortcuts, an in-app log viewer, and an optional
  on-disk log file.

Credentials (a global default plus optional per-device overrides) are stored in
the OS keychain — never written to disk in plaintext.

## Requirements

- A recent stable Rust toolchain
- [`dioxus-cli`](https://dioxuslabs.com/learn/0.6/CLI/installation):
  `cargo install dioxus-cli`
- **Linux only** — the usual WebKitGTK/wry system packages, e.g. on
  Debian/Ubuntu:
  ```sh
  sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev libxdo-dev
  ```
- **RTSP mode only** — `ffmpeg` on `PATH` is required for H.265 transcoding.
  Snapshot (MJPEG) mode needs nothing extra.

## Running

```sh
dx serve --platform desktop
```

Verbose logging:

```sh
RUST_LOG=oxdm=debug dx serve --platform desktop
```

A plain `cargo run` also works, but `dx serve` gives hot-reload.

## Trying it without a camera

OxDM pairs with the `oxvif` mock server, which speaks enough ONVIF to exercise
most of the UI. The `oxvif` library is on
[crates.io](https://crates.io/crates/oxvif) and pulled in automatically when
you build OxDM, but the standalone mock-server binary ships as an `oxvif`
*example* — so you need a local checkout to run it:

```sh
# One-time — clone the oxvif repo (no need to build OxDM from it; that comes from crates.io)
git clone https://github.com/smiti1642/oxvif ../oxvif

# Terminal 1 — start the mock server (default port 18080)
cd ../oxvif && cargo run --example mock_server --features mock-server

# Terminal 2 — start OxDM
dx serve --platform desktop
```

Then in OxDM: open the **Manual** tab → **Add** → enter `127.0.0.1:18080`
(no credentials needed). Snapshot thumbnails and the settings tabs will show
live data from the mock device. The **Diagnostics** tab works against the
mock too — useful for a smoke test of the health check itself before pointing
it at a real camera.

## Architecture

Single crate, desktop-only. A fixed four-pane shell (Topbar + device list +
device panel + main content) switches views via a `View` enum — no router.
All ONVIF calls funnel through `src/api.rs` (which wraps `oxvif`), and a
process-wide session cache (`src/sessions.rs`) reuses `OnvifSession`s across
calls. See [`CLAUDE.md`](./CLAUDE.md) for the full module map and conventions,
and [`ODM.md`](./ODM.md) for the ONVIF API coverage map and roadmap.

## License

[MIT](./LICENSE) © 2025 smiti1642
