# OxDM — Development Guidelines

## Project overview

`oxdm` is a Dioxus desktop app for managing ONVIF IP cameras, built on top of
`oxvif`. Single crate, no workspace. Desktop-only (Dioxus 0.7.9, wry window).

## Working principles

Behavioral guidelines to reduce common LLM coding mistakes, merged with this
project's specifics. **Tradeoff:** these bias toward caution over speed — for
trivial tasks, use judgment.

### 1. Think before coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity first

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes,
simplify.

### 3. Surgical changes

**Touch only what you must. Clean up only your own mess.** Every changed line
should trace directly to the user's request.

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style (the "Coding rules" below are the source of truth),
  even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

### 4. Goal-driven execution

**Define success criteria. Loop until verified.** The "Before every commit"
gate below (`cargo fmt` / `clippy` / `build` / `test`, including the i18n
parity check) is the default verification loop — run it, don't assume.

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass."
- "Fix the bug" → "Write a test that reproduces it, then make it pass."
- "Refactor X" → "Ensure tests pass before and after."

For multi-step tasks, state a brief plan with a verify step per item:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it
work") require constant clarification.

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer
rewrites due to overcomplication, and clarifying questions come before
implementation rather than after mistakes.

## Before every commit

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo build
cargo test
```

All four must pass cleanly. `tests/i18n_tests.rs` includes an exhaustive
i18n-key parity check — adding a string to `i18n/en.rs` without the other
locales will fail the suite.

## Running locally

```
dx serve --platform desktop
```

Requires `dioxus-cli` (`cargo install dioxus-cli`).

Verbose logging: `RUST_LOG=oxdm=debug dx serve --platform desktop`
(every `api::*` call is `#[instrument]`-ed).

## Styling

`assets/main.css` is the single, **hand-authored** stylesheet (Catppuccin
Mocha theme + semantic component classes like `.icon-btn`,
`.content-header`). It is `include_str!`'d into the binary at
`src/main.rs` and is git-tracked — edit it directly, no build step.

There is **no Tailwind or other CSS pipeline**: no `tailwind.css`, no
`package.json`, no npm. Components use semantic class names, not utility
classes. Just edit `main.css`.

## Architecture

No Dioxus Router. `App` composes a fixed four-pane shell (Topbar + sidebar
DeviceList + DevicePanel + MainContent), and `MainContent` switches on the
`View` enum in `state.rs`.

```
src/
  main.rs           Entry point; App component + window config + tracing
                    setup (optional daily-rolling file log) + one-time
                    install of both video backends.
  state.rs          Ctx (global signals), View, SettingsTab, DeviceListTab,
                    DeviceEntry, Credentials, AuthStatus, Theme, Locale,
                    Toast/ConfirmDialog, GlobalKey (keyboard-shortcut bus).
  api.rs            Async wrappers around oxvif (discovery, device info,
                    media, imaging, PTZ, network, users, maintenance,
                    events, OSD, profiles) + snapshot fetch with
                    Digest/Basic auth fallback. Every wrapper funnels
                    through `crate::sessions` for session reuse — see
                    "Session reuse" below. Discovery delegates a single
                    round to oxvif::discovery::probe (multi-NIC +
                    IP_MULTICAST_IF pinning handled upstream); the
                    multi-round scan loop lives in device_list.rs.
  sessions.rs       Process-wide cache of `oxvif::OnvifSession` keyed by
                    `(addr, creds)`. First call per (addr, creds) builds
                    the session (one GetCapabilities round-trip); every
                    subsequent call returns a cheap Arc clone. Cred change
                    or device removal calls `sessions::invalidate(addr)` /
                    `invalidate_all()` to drop stale entries.
  persist.rs        ~/.oxdm/config.toml (theme/locale/log/tls),
                    ~/.oxdm/devices.toml (manual devices),
                    + single JSON blob in system keychain for ALL
                    credentials (one keychain prompt, not N)
  device_ops.rs     Background firmware fetch + auth re-verification
  util.rs           extract_ip, urldecode, copy_to_clipboard

  video/                   Live-video backends behind the VideoBackend trait
    mod.rs                 Trait + global registry (MJPEG default + go2rtc);
                           VideoSource / EmbedKind
    mjpeg.rs               Pure-Rust 127.0.0.1 HTTP server: polls
                           GetSnapshotUri per stream, pushes JPEG frames as
                           multipart/x-mixed-replace. Always-on default.
    go2rtc.rs              Spawns the go2rtc helper binary for RTSP-grade
                           playback (H.265 transcode). Optional + lazy.

  components/
    mod.rs
    topbar.rs              Theme + locale toggle + help (opens About)
    device_list.rs         Left sidebar: tabs (Discovered / Manual),
                           search, scan/add buttons, DeviceCard + context
                           menu; owns the multi-round discovery scan loop
    device_panel.rs        Middle pane: selected-device nav + NVT
                           profile thumbnails (3 s auto-refresh)
    credentials_dialog.rs  Global creds + Add Device modals
    edit_device_dialog.rs  Edit manual device + per-device creds
    about_dialog.rs        About modal: versions, repo link, log-to-file +
                           TLS-strict toggles
    dialog.rs              Generic confirm modal (ConfirmDialogModal)
    dialog_overlay.rs      Reusable modal scaffold (click-outside +
                           Esc-to-close) wrapping every dialog body
    tab_error.rs           Inline error + Retry block for failed
                           use_resource fetches
    toast.rs               Toast container + auto-dismiss
    context_menu.rs        Right-click menu primitive
    status_bar.rs
    icon.rs                SVG icon set (feather-style)
    shared.rs              PropRow, PasswordField

  views/
    mod.rs
    main_content.rs        Switch on View; renders WelcomeView /
                           DeviceSettingsView (with SettingsTab bar) /
                           LiveVideoView / ImagingView / PtzControlView /
                           EventsView / OsdView
    live_video.rs          Live preview with Snapshot/RTSP mode tabs; also
                           exports LiveVideoStage/LiveModeTabs reused by
                           Imaging + PTZ
    imaging.rs             Imaging service controls (auto + manual exposure /
                           WB gains / focus limits) over an embedded live
                           preview, plus the embedded VideoEncoderSection
    video_encoder.rs       VideoEncoderSection — per-profile encoder edit
                           (resolution / bitrate / gov_length / quality);
                           embedded inside ImagingView, not a top-level View.
                           H.265 profiles auto-route through Media2 via
                           api::set_video_encoder_configuration
    ptz.rs                 PTZ pad + zoom + preset list over a live preview
    osd.rs                 OSD text/overlay read + create / update / delete
    io_control.rs          Relay output control (Activate/Deactivate for
                           Bistable, Pulse for Monostable, settings editor)
                           + digital input config list. Uses oxvif 0.9.9
                           GetDigitalInputs / GetRelayOutputs / Set*.
    events.rs              PullPoint event subscription + rolling event log
    settings/
      mod.rs
      identification.rs    GetDeviceInformation + GetScopes (IdentificationTab)
      network.rs           Hostname / IPv4 + IPv6 manual interfaces / MTU /
                           DNS / NTP / Gateway / Protocols — read + write
                           (NetworkTab). IPv6 panel uses
                           api::set_network_interfaces_full and the oxvif
                           NetworkInterfaceConfig struct
      time.rs              Get/SetSystemDateAndTime (TimeTab)
      users.rs             Users list + create / delete / set (UsersTab)
      maintenance.rs       Reboot + factory reset, both confirm-gated
                           (MaintenanceTab)
      health.rs            On-demand oxvif::health::HealthCheck run +
                           rendered Pass/Warn/Fail/Skip per category +
                           Profile S/T/G verdict (HealthTab). Save-as-
                           baseline writes a JSON HealthReport to
                           ~/.oxdm/baselines/<sanitized-addr>.json;
                           subsequent runs render report.diff(&baseline)
                           below the per-check table

  i18n/
    mod.rs                 t(locale, key) with English fallback
    en.rs                  Canonical key set
    zh_tw.rs
    ru.rs

  tests/                   All #[cfg(test)] modules, kept out of src files
    mod.rs
    api_tests.rs
    i18n_tests.rs          Exhaustive i18n-key parity across en / zh_tw / ru
    state_tests.rs
    util_tests.rs

tests/                     Top-level integration tests (NOT in src/tests)
  healthtab_smoke.rs       Boots oxvif::mock::MockServer, runs HealthCheck
                           against it, asserts JSON round-trips and that
                           report.diff(&report) is empty. Gated by the
                           [dev-dependencies] entry that pulls in oxvif's
                           mock-server feature — release builds don't carry
                           axum

assets/
  main.css                 Hand-authored stylesheet (git-tracked,
                           include_str!'d into the binary; edit directly)
```

## Coding rules

- Components are `#[component] pub fn Foo() -> Element` (PascalCase).
- All `oxvif` calls go in `src/api.rs`. Views call `api::*`, never
  `OnvifClient` directly.
- `use_resource` for async fetches; `use_signal` for local state; a single
  `Ctx` (in `state.rs`) carries global signals via `use_context_provider`.
- No `unwrap()` in component code — handle `None`/`Err` gracefully
  (show a toast, render an empty state, or surface the error in the tab).
- Every destructive action (reboot, factory reset, delete user/device,
  firmware upgrade) must go through `ctx.dialog.set(Some(ConfirmDialog {
  dangerous: true, … }))`.
- User-visible strings go through `i18n::t(locale, key)`. Every key must
  exist in `en.rs`, `zh_tw.rs`, and `ru.rs` (tests enforce this).
- Themes are class variants on the root (`theme-dark` / `theme-light` /
  `theme-classic`) — never `prefers-color-scheme`.
- Credentials: always use `ctx.credentials_for(&device)` so per-device
  overrides (manual devices only) beat the global default.

## Session reuse

`api::*` never builds an `OnvifClient`/`OnvifSession` directly — every
wrapper goes through `crate::sessions::get(addr, u, p)` which returns
the cached `Arc<OnvifSession>` (or builds + caches a new one on first
call for that key). The session caches `Capabilities` and reuses the
underlying reqwest connection pool, so once a device is touched in
the process, every subsequent SOAP call costs exactly one round-trip.

When credentials change (global creds dialog or per-device edit) or a
device is removed, the corresponding `sessions::invalidate(addr)` /
`invalidate_all()` call drops the stale entries so the next API call
rebuilds with the new creds. Adding a new write-path that mutates
creds means adding the matching `invalidate` call.

The two functions in `api.rs` that don't go through `sessions::` are
`fetch_snapshot_data_uri` (custom HTTP + Digest, byte-identical to
curl for Hikvision/Uniview compat) and `discover_one_round`
(WS-Discovery is its own protocol).

## Adding a new view

1. Add a variant to `View` in `src/state.rs`.
2. Create `src/views/<name>.rs` with `#[component] pub fn <Name>View(addr:
   ReadSignal<String>, creds: Memo<Credentials>) -> Element`.
3. Add a `match` arm for the new variant in
   `src/views/main_content.rs::MainContent`.
4. Add a `NavLink { view: View::<Name>, icon, label }` in
   `src/components/device_panel.rs` (or a thumbnail action if it's a
   per-profile view), plus an `i18n::t` key triple.

## Adding a settings tab

1. Add a variant to `SettingsTab` in `state.rs`.
2. Create `src/views/settings/<name>.rs` with a `#[component] pub fn
   <Name>Tab(...)` (tabs are named `<Name>Tab`, not `<Name>View`) using the
   same `addr + creds` signature as the existing tabs.
3. Re-export `<Name>Tab` from `src/views/settings/mod.rs`.
4. Append to `SETTINGS_TABS` in `src/views/main_content.rs` and add a
   match arm inside `DeviceSettingsView`.

## Credentials + persistence

- Global credentials + every per-device override are serialized to one
  JSON blob stored under keychain service `com.oxdm`, user `credentials`.
  This is deliberate — one keychain entry means one macOS permission
  prompt, not N prompts.
- `config.toml` stores only theme + locale (plus legacy fields read for
  migration). Credentials are *never* written to disk in plaintext.
- `devices.toml` stores only `{ name, addr, has_credentials }` for manual
  devices. The actual credentials (when `has_credentials = true`) live in
  the keychain entry, keyed by the device `addr`.

## oxvif version

Currently `oxvif = "0.9.9"`, pinned to the crates.io registry, with the
`health` feature enabled in `[dependencies]` and `mock-server, health` in
`[dev-dependencies]` (the latter only for `tests/healthtab_smoke.rs`; the
release binary never pulls axum). Notable surfaces oxdm relies on:

- `oxvif::{RelayOutput, DigitalInput}` — IO Control view (`views/io_control.rs`)
  reads both via `api::get_relay_outputs` / `api::get_digital_inputs` and
  writes via `api::set_relay_output_state` / `api::set_relay_output_settings`
  (0.9.9 added `GetDigitalInputs`). Mock state under
  `oxvif::mock::DeviceState::{relay_outputs, digital_inputs}` plus the
  `/mock/digital-input/{token}/pulse|set` REST hooks let tests drive
  simulated input transitions without a real camera (see
  `tests/io_control_smoke.rs`).

- `oxvif::health::{HealthCheck, HealthReport, ReportDiff, SlowedCheck}` —
  serde-derived report types, used by `views/settings/health.rs` for the
  baseline diff flow.
- `oxvif::{NetworkInterfaceConfig, IpStackConfig, ManualAddress}` — the
  struct-shaped `set_network_interfaces` API (breaking change vs 0.9.7);
  `api::set_network_interfaces` still exposes the old positional shape to
  the rest of oxdm, and `api::set_network_interfaces_full` is the
  struct-passing variant the IPv6 panel calls.
- `VideoEncoderConfiguration2` / Media2 set path — `api::set_video_encoder_configuration`
  detects `VideoEncoding::H265` and auto-routes through Media2 (Media1's
  schema doesn't carry H.265; oxvif now rejects it with `InvalidArgument`
  at the boundary).
- `ImagingSettings` gained eight `Option<...>` fields for manual exposure /
  WB Cr/Cb gains / focus near-far limits — consumed by `views/imaging.rs`.

When iterating on oxvif locally before a release, switch to a path dep:

```toml
oxvif = { version = "0.9.9", path = "../oxvif", features = ["health"] }
```

After publishing the new oxvif version to crates.io, drop the `path` to
pin back to the registry — CI builds on a runner without `../oxvif`, so a
path dep makes CI fail to resolve the dependency. Also bump
`OXVIF_VERSION` in `src/components/about_dialog.rs` to match (shown in the
About dialog).

To upgrade oxvif further, bump the version and re-verify every call site
in `src/api.rs` still compiles — types like `ImagingSettings`,
`ImagingOptions`, `FloatRange`, `NetworkInterfaceConfig`, and the
service-URL fields on `Capabilities` are the usual breakage points.

## Known quirks

- WS-Discovery runs a single round per call via `oxvif::discovery::probe`,
  wrapped by `api::discover_one_round`. The multi-round scan loop (3 rounds,
  2 s timeout, 800 ms interval — the `ROUNDS` / `PROBE_TIMEOUT` /
  `PROBE_INTERVAL` constants) lives in `device_list.rs`. oxvif handles
  multi-NIC enumeration and `IP_MULTICAST_IF` pinning internally (the latter
  is critical on Windows — without it, multicast leaks out through Hyper-V /
  WSL virtual adapters and never reaches the camera subnet). Don't
  reintroduce a hand-rolled discovery layer in oxdm; if the upstream
  behaviour needs tweaking, fix it in oxvif.
- `api::fetch_snapshot_data_uri` patches the `digest_auth` crate output
  for Hikvision compatibility: `qop=auth` → `qop="auth"`, and `, ` → `,`
  between parameters. Removing these patches breaks Hikvision snapshots.
- Snapshot URIs returned by some cameras are relative or schemeless;
  `device_panel.rs::ProfileThumbnails` resolves them against the device
  base URL before fetching.
