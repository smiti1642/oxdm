# OxDM — ONVIF Device Manager

**OxDM** (*oxvif Device Manager*) is a modern, cross-platform ONVIF IP camera
manager — a contemporary successor to the classic **ONVIF Device Manager
(ODM)**. It is written in Rust with [Dioxus](https://dioxuslabs.com/) and built
on the [`oxvif`](https://github.com/smiti1642/oxvif) ONVIF client library.

![OxDM managing an ONVIF camera — device list, profile panel, and the device identification settings tab](https://raw.githubusercontent.com/smiti1642/oxdm/main/docs/screenshot.png)

> **Project status — pre-release (v0.3.0).** Core device management works
> end-to-end against real cameras and the `oxvif` mock server. Release bundles
> are not yet code-signed, so the operating system may warn about an
> unidentified developer on first launch.

## Contents

- [Installation](#installation)
- [Features](#features)
- [Diagnostics — the ONVIF health check](#diagnostics--the-onvif-health-check)
- [Camera clones and the Quirks diff](#camera-clones-and-the-quirks-diff)
- [Trying it without a camera](#trying-it-without-a-camera)
- [Development](#development)

## Installation

### Prebuilt bundles

Bundles for each release are attached to the corresponding
[GitHub Release](https://github.com/smiti1642/oxdm/releases):

| Platform | Asset | Notes |
|----------|-------|-------|
| macOS (Apple Silicon) | `oxdm-<version>-macos-aarch64.dmg` | `aarch64` only |
| Windows (x86-64) — installer | `oxdm-<version>-windows-x86_64.msi` | Start-menu shortcut |
| Windows (x86-64) — portable | `oxdm-<version>-windows-x86_64-portable.zip` | unzip and run `oxdm.exe` |
| Linux — Ubuntu / Debian (x86-64) | `oxdm-<version>-ubuntu-x86_64.deb` | `sudo apt install ./<file>.deb` |

The bundles are **not code-signed**, so every OS shows a first-run warning.
Notes:

- **macOS: "oxdm is damaged and can't be opened".** The app is not damaged —
  this is Gatekeeper blocking an unsigned, un-notarized app. Drag **oxdm** to
  **Applications**, then clear the quarantine flag and launch normally:
  ```sh
  xattr -dr com.apple.quarantine /Applications/oxdm.app
  ```
  Apple Silicon (`aarch64`) only — the build will not run on an Intel Mac; on
  Intel, build from source.
- **Windows: SmartScreen "Windows protected your PC".** Click **More info** →
  **Run anyway**. The bundles rely on the **WebView2 runtime**, preinstalled on
  Windows 10/11; if the window stays blank on an older or stripped-down system,
  install the WebView2 runtime from Microsoft.
- **Fedora / RHEL-based distributions are not yet supported** as a prebuilt
  package (a different WebKitGTK layout, no `.deb`). Native support is planned
  via Flatpak. Until then, build from source (below).

### Build from source

OxDM builds with a standard Rust toolchain — no extra tooling is required to
produce a runnable binary (`dx` is only needed for hot-reload development and
for producing installer bundles). Install from
[crates.io](https://crates.io/crates/oxvif-device-manager):

```sh
cargo install oxvif-device-manager
```

or build the latest commit directly from Git:

```sh
cargo install --git https://github.com/smiti1642/oxdm
```

Either way the installed command is **`oxdm`** (the crate is published as
`oxvif-device-manager` because the shorter name was already taken).

On Linux, install the WebKitGTK/wry development packages first. For example, on
Debian/Ubuntu:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev libxdo-dev
```

The equivalent Fedora packages are `webkit2gtk4.1-devel`, `gtk3-devel`,
`libayatana-appindicator-gtk3-devel`, and `libxdo-devel`.

## Features

- **Discovery** — WS-Discovery scan of the local network, plus manually-added
  devices. Discovered devices persist across restarts.
- **Live video** — always-on MJPEG snapshot stream, or RTSP (H.264/H.265) via a
  bundled go2rtc bridge with H.265 → H.264 transcode and MSE fallback.
- **Snapshots** — save a JPEG from any profile thumbnail or the Live Video view.
- **Device settings** — identification and scopes; network (hostname, IPv4 and
  IPv6 manual interfaces, MTU, DNS, NTP, gateway, protocols); system time (with
  PC sync and timezone/DST); user management (create/read/update/delete); and
  maintenance (reboot and factory reset, both confirmation-gated).
- **Media** — profile create/delete, video-encoder configuration (H.264 and
  H.265, with H.265 automatically routed through Media2), imaging controls
  (brightness/contrast plus manual exposure, white-balance gains, and focus
  limits), and OSD management.
- **PTZ** — preset create/read/update/delete, continuous and absolute moves,
  and home position.
- **Events** — live PullPoint subscription with a scrolling, filterable log.
- **Diagnostics** — an on-demand ONVIF health check with baseline diffing and
  fleet-wide batch export. [See below](#diagnostics--the-onvif-health-check).
- **Camera clones (mocks)** — record a real camera, serve it offline, and diff
  its response shapes against a reference.
  [See below](#camera-clones-and-the-quirks-diff).
- **Localisation and theming** — three themes (Dark / Light / Classic);
  English, 繁體中文, and Русский locales; keyboard shortcuts; an in-app log
  viewer; and an optional on-disk log file.

Credentials (a global default plus optional per-device overrides) are stored in
the operating-system keychain and are never written to disk in plaintext.

## Diagnostics — the ONVIF health check

![Health Overview in OxDM: one row per camera with pass/warn/fail/skip counts, Profile S/T/G badges, and live stream and recording probe results](https://raw.githubusercontent.com/smiti1642/oxdm/main/docs/health-check.png)

A readable alternative to the official ONVIF Device Test Tool. Run it on one
device from its **Diagnostics** tab, or on the whole fleet at once from **Health
Overview** — every camera reports **Pass / Warn / Fail / Skip** counts and ends in
a **Profile S/T/G verdict**. Devices can be sorted into named **groups**
(right-click → add to group) so a run can target a floor, a site, or a vendor
rather than everything.

`Declared: M` next to a verdict is the camera's *own* claim read from its scopes,
shown beside what the run actually **assessed** — a device that declares Profile G
and fails replay is the interesting case, and it is only visible when both numbers
are on screen.

**It verifies, it does not just ask.** A check that only confirmed the device
answered a SOAP call would pass a camera whose stream is dead. So the check opens
the RTSP stream, fetches the snapshot and validates it as a real image (rejecting a
0-byte body or an HTML error page served with a `200`), and genuinely exercises
Profile G recording search and replay.

That is what the `snapshot 291 KB` and `RTSP OK` badges in the shot above are:
bytes that actually arrived, not a URL the camera claimed would work.

Beyond that:

- **Baseline diff.** "Save as baseline" stores the run per-device; the next run
  diffs against it automatically. Regressions to FAIL, checks that appeared or
  disappeared, and checks that slowed by **2× or more** are all flagged.
- **Security probe.** With credentials supplied, a credential-free
  `GetDeviceInformation` probe checks the camera actually enforces
  authentication. A camera that serves device info anonymously is flagged.
- **Under-declared services.** Optionally force-verify services the device does
  *not* advertise, to catch firmware that under-declares its own capabilities.
- **Write round-trip (opt-in, batch).** Re-`Set` the first video-encoder config
  unchanged. This catches devices that reject our serialized request body — an
  interop bug no read-only probe can see.
- **Fleet export.** Batch a run across every device and export the rich JSON
  bundle, or **JUnit XML** for a CI dashboard.

The engine is [`oxvif`'s `health` feature](https://github.com/smiti1642/oxvif#health-check-health-feature),
so the same verdicts are available headlessly from a script or CI job — OxDM is
the interactive front end to it, not a separate implementation.

## Camera clones and the Quirks diff

![The Quirks tab in OxDM: a quirk report grouped by service area, each operation showing added and removed element counts, above a note that operations the device declined with a SOAP Fault are correct device behaviour rather than a client problem](https://raw.githubusercontent.com/smiti1642/oxdm/main/docs/quirks.png)

Right-click a device → **"Clone this camera"**. OxDM records its standard read
surface and serves the recording from an **in-app mock server**, then adds it to
the device list labeled *mock*. You can then operate the clone through every tab
— settings, media, PTZ, imaging — **with the real camera unplugged**.

Clones persist to `~/.oxdm/clones/`; the **Saved mocks** list in the Manual tab
reopens one at any time.

A mock device also gains a **Quirks** tab, grouped by service area with an
issues / clean / skipped count per group. Since 0.3.0 it also works against a
**live camera** — pick the operations, watch them run, and see what moved since
last time, without recording a clone first. Selected operations export to JSON.

Two details in that first shot are the ones that make the tab usable rather than
alarming:

- **A declined operation is not a bug.** Operations the camera refused with a SOAP
  Fault are called out as *correct device behaviour, not an oxvif problem* — a
  camera is allowed to not implement something, and burying those in the issue
  count would make every device look broken.
- **Diff vs baseline.** *"unchanged since the baseline — the same operations
  drift, in the same places"* is the answer you want most of the time. Firmware
  upgrades are when it stops saying that.

Expanding an operation gives the git-style side-by-side: `oxvif` reference on the
left, the camera on the right, with word-level highlighting on what differs.

![A git-style side-by-side diff of one operation: oxvif's reference response on the left, the cloned camera's on the right, with word-level highlighting on the values that differ and extra vendor blocks the reference does not emit](https://raw.githubusercontent.com/smiti1642/oxdm/main/docs/quirks-diff.png)

This is where a device's personality shows up: the same `GetProfiles` call, and
this camera names its profile `R_H264` rather than `mainStream`, runs the stream at
640×360 rather than 1920×1080, and returns an `<Extension><Rotate>` block and an
`<AudioSourceConfiguration>` the reference never emits. None of that is an error —
it is exactly the shape a client has to survive.

The `__MASKED__` tokens are deliberate: instance values are normalised before the
comparison so the diff shows *shape* drift rather than every serial number, and a
saved clone carries no credential.

**Honest scope.** Both of these are deliberately narrower than they might look:

- A clone covers the **standard read surface**, not the whole device. It is a
  standard-surface snapshot, not a 100% clone.
- Recorded `GetServices` responses and stream URIs embed the **real camera's
  addresses**, so some media/PTZ calls on a clone may still route to the real
  device. Rewriting those to point at the container is later work.
- The Quirks diff is **structural** — which elements are present — not ONVIF
  schema conformance.

These limits are surfaced in the UI too, not only here.

Built on `oxvif`'s
[`metamorph-server`](https://github.com/smiti1642/oxvif#metamorph-metamorph--metamorph-server-features)
feature, which is enabled in the default build.

## Trying it without a camera

OxDM pairs with the `oxvif` mock server, which implements enough of ONVIF to
exercise most of the UI. The `oxvif` library is pulled in from
[crates.io](https://crates.io/crates/oxvif) automatically, but the standalone
mock server ships as an `oxvif` *example*, so it requires a local checkout:

```sh
# One-time: clone the oxvif repository
git clone https://github.com/smiti1642/oxvif ../oxvif

# Terminal 1: start the mock server (default port 18080)
cd ../oxvif && cargo run --example mock_server --features mock-server

# Terminal 2: start OxDM
dx serve --platform desktop
```

In OxDM, open the **Manual** tab → **Add** → enter `127.0.0.1:18080` (no
credentials required). Snapshot thumbnails and the settings tabs will show live
data from the mock device, and the **Diagnostics** tab works against it as well.

## Usage

Once installed, launch OxDM and use the left sidebar to scan for devices or add
one manually. Select a device to access its settings, live video, PTZ, events,
and diagnostics.

## Development

```sh
dx serve --platform desktop
```

`dx serve` provides hot-reload during development and requires
[`dioxus-cli`](https://dioxuslabs.com/learn/0.6/CLI/installation)
(`cargo install dioxus-cli`). A plain `cargo run` also works without it.

Verbose logging:

```sh
RUST_LOG=oxdm=debug dx serve --platform desktop
```

RTSP mode additionally requires `ffmpeg` on `PATH` for H.265 transcoding;
snapshot (MJPEG) mode needs nothing extra.

## License

Released under the [MIT License](./LICENSE). © 2026 smiti1642
