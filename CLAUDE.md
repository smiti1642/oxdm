# OxDM — Development Guidelines

## Project overview

`oxdm` is a Dioxus desktop app for managing ONVIF IP cameras, built on top of
`oxvif`. Single crate, no workspace. Desktop-only (Dioxus 0.7.4, wry window).

Scope, priorities, and the ONVIF API coverage map live in [`ODM.md`](./ODM.md).

## Before every commit

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo build
cargo test
```

All four must pass cleanly. `tests.rs` includes an exhaustive i18n-key
parity check — adding a string to `i18n/en.rs` without the other locales
will fail the suite.

## Running locally

```
dx serve --platform desktop
```

Requires `dioxus-cli` (`cargo install dioxus-cli`).

Verbose logging: `RUST_LOG=oxdm=debug dx serve --platform desktop`
(every `api::*` call is `#[instrument]`-ed).

## Rebuilding Tailwind CSS

```
npx @tailwindcss/cli -i tailwind.css -o assets/main.css --watch
```

`assets/main.css` is in `.gitignore` — always rebuild before testing or
committing UI changes.

## Architecture

No Dioxus Router. `App` composes a fixed four-pane shell (Topbar + sidebar
DeviceList + DevicePanel + MainContent), and `MainContent` switches on the
`View` enum in `state.rs`.

```
src/
  main.rs           Entry point; App component + window config
  state.rs          Ctx (global signals), View, SettingsTab, DeviceEntry,
                    Credentials, Theme, Locale, Toast/ConfirmDialog
  api.rs            Async wrappers around oxvif (discovery, device info,
                    media, imaging, network, users, maintenance) +
                    snapshot fetch with Digest/Basic auth fallback.
                    Delegates WS-Discovery to oxvif::discovery::probe_rounds
                    (multi-NIC + IP_MULTICAST_IF pinning handled upstream).
  persist.rs        ~/.oxdm/config.toml (theme/locale),
                    ~/.oxdm/devices.toml (manual devices),
                    + single JSON blob in system keychain for ALL
                    credentials (one keychain prompt, not N)
  device_ops.rs     Background firmware fetch + auth re-verification
  util.rs           extract_ip, urldecode, copy_to_clipboard
  tests.rs          i18n-key parity tests across en / zh_tw / ru

  components/
    mod.rs
    topbar.rs              Theme + locale toggle + help
    device_list.rs         Left sidebar: tabs (Discovered / Manual),
                           search, scan/add buttons, DeviceCard + context menu
    device_panel.rs        Middle pane: selected-device nav + NVT
                           profile thumbnails (3 s auto-refresh)
    credentials_dialog.rs  Global creds + Add Device modals
    edit_device_dialog.rs  Edit manual device + per-device creds
    dialog.rs              Generic confirm modal
    toast.rs               Toast container + auto-dismiss
    context_menu.rs        Right-click menu primitive
    status_bar.rs
    icon.rs                SVG icon set (feather-style)
    shared.rs              PropRow, PasswordField

  views/
    mod.rs
    main_content.rs        Switch on View; renders WelcomeView /
                           DeviceSettingsView (with SettingsTab bar) /
                           ImagingView / Placeholders for LiveVideo,
                           PtzControl, Events
    imaging.rs             Imaging service controls (sliders + selects)
    settings/
      mod.rs
      identification.rs    GetDeviceInformation + GetScopes
      network.rs           Hostname / Interfaces / DNS / NTP /
                           Gateway / Protocols (read-only today)
      time.rs              GetSystemDateAndTime (read-only)
      users.rs             GetUsers list
      maintenance.rs       Reboot + factory reset (both confirm-gated)

  i18n/
    mod.rs                 t(locale, key) with English fallback
    en.rs                  Canonical key set
    zh_tw.rs
    ru.rs

assets/
  main.css                 Tailwind output (git-ignored, build locally)
tailwind.css               Tailwind input (source of truth)
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
2. Create `src/views/settings/<name>.rs` with the same `addr + creds`
   signature as the existing tabs.
3. Re-export from `src/views/settings/mod.rs`.
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

Currently `oxvif = { version = "0.9.1", path = "../oxvif" }` — the path
dep lets us iterate on oxvif locally before a crates.io release. Once
0.9.1 is published, strip the `path` to pull from the registry:

```toml
oxvif = "0.9.1"
```

To upgrade oxvif further, bump the version and re-verify every call site
in `src/api.rs` still compiles — types like `ImagingSettings`,
`ImagingOptions`, `FloatRange`, and the service-URL fields on
`Capabilities` are the usual breakage points.

## Known quirks

- WS-Discovery is delegated to `oxvif::discovery::probe_rounds` (3 rounds,
  2 s timeout, 800 ms interval by default — see `api::discover_devices`).
  oxvif handles multi-NIC enumeration and `IP_MULTICAST_IF` pinning
  internally (the latter is critical on Windows — without it, multicast
  leaks out through Hyper-V / WSL virtual adapters and never reaches the
  camera subnet). Don't reintroduce a hand-rolled discovery layer in
  oxdm; if the upstream behaviour needs tweaking, fix it in oxvif.
- `api::fetch_snapshot_data_uri` patches the `digest_auth` crate output
  for Hikvision compatibility: `qop=auth` → `qop="auth"`, and `, ` → `,`
  between parameters. Removing these patches breaks Hikvision snapshots.
- Snapshot URIs returned by some cameras are relative or schemeless;
  `device_panel.rs::ProfileThumbnails` resolves them against the device
  base URL before fetching.
