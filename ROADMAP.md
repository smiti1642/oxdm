# OxDM — Roadmap

A prioritised backlog of ONVIF coverage gaps and the UI work needed to surface
them. Derived from comparing the `oxvif` protocol reference (`../oxvif/docs/`,
the per-service WSDL catalogue) against what `oxvif` implements and what OxDM
currently exposes.

Most OxDM features are gated by `oxvif` service coverage, so each item notes the
cost on **both** layers. This roadmap tracks the *application* side (which view
or settings tab surfaces each capability); the *protocol* side is tracked in
[`oxvif/ROADMAP.md`](https://github.com/smiti1642/oxvif/blob/main/ROADMAP.md).

## Scoring

- **Value** — usefulness for IP-camera management, 1 (niche) … 5 (flagship).
- **oxvif / oxdm cost** — S(mall) / M(edium) / L(arge) implementation effort.
- **Priority** — P0 (cheap, do now) … P3 (defer / maybe never), derived from
  value ÷ cost.

## Backlog

| # | Item | Value | oxvif | oxdm | Pri | OxDM UI landing |
|---|------|:----:|:----:|:----:|:---:|-----------------|
| 1 | **Capability-driven UI** — harvest each service's `GetServiceCapabilities`, hide tabs the device doesn't support | 4 | S | S | **P0** | Not a new view; filter NavLinks / SettingsTabs in `device_panel.rs` by capabilities |
| 2 | **Imaging scene presets** — `GetPresets` / `GetCurrentPreset` / `SetCurrentPreset` | 3 | S | S | **P0** | Dropdown in `views/imaging.rs` (Day / Night / Indoor …) |
| 3 | **DeviceIO completion + doc fix** — docs mark DeviceIO ❌ but relay / digital-input ops already exist via the Device service (should be ◐); add the `tmd`-only `GetVideoSources` / `GetSerialPorts` | 2 | S | S | P0 | Extend existing `views/io_control.rs` |
| 4 | **Profile G playback** — add `GetRecordingSummary` / `GetRecordingInformation`, wire to the existing `find_recordings` / `get_replay_uri` | **5** | M | L | **P1** | New `View::Recordings` + `views/recordings.rs`: timeline + RTSP replay (reuse the go2rtc / ffmpeg player) |
| 5 | **Firmware upgrade** — `UpgradeFirmware` | 4 | M | M | **P1** | "Upload firmware" block in `settings/maintenance.rs`, confirm-gated, reusing the existing firmware fetch |
| 6 | **Config backup / restore** — `GetSystemBackup` / `RestoreSystem` | 4 | M | S | **P1** | Export / import buttons in `settings/maintenance.rs` (restore via dangerous ConfirmDialog) |
| 7 | **Event broker / MQTT** — `Get/Add/DeleteEventBrokers` | 3 | M | M | P1 | A "forwarding" panel atop `views/events.rs` (or a new EventsTab) |
| 8 | **Analytics rules** — modules + rules + metadata | **5** | L | L | **P2** | New `View::Analytics`: rule list (line-cross / intrusion) + bounding-box overlay on the live preview |
| 9 | **On-device recording config** — recording / job / track `Get/Set` | 4 | L | L | P2 | A "Schedule" sub-tab inside the same Recordings view as #4 |
| 10 | **PTZ preset tours** — full tour family + `GeoMove` | 3 | M | M | P2 | A "Tours" sub-tab in `views/ptz.rs` (create / start / stop) |
| 11 | **Advanced device settings (bundle)** — DDNS, ZeroConf, IP filter, storage config, geolocation, user roles, password policies, RemoteUser | 3 | M | M | P2 | Fold into `settings/network.rs` (DDNS / ZeroConf / IP filter), `settings/users.rs` (roles / password policy), new StorageTab (storage config) |
| 12 | **Multicast streaming control** — `Start/StopMulticastStreaming` + `SetSynchronizationPoint` | 2 | S | S | P3 | Toggle in the `live_video.rs` RTSP tab (niche) |
| 13 | **Events Seek / Pause / Resume** | 2 | S | S | P3 | Controls on the existing `events.rs` log |
| 14 | **Audio configuration (full set)** | 2 | M | M | P3 | Only worth it alongside two-way audio; config-only value is low |
| 15 | **Unicast discovery `Resolve`** | 1 | S | — | P3 | No UI; enterprise-subnet use |
| 16 | **Receiver / access-control family** (Profiles A/C/D) | 1 | L | L | **P3 / won't do** | Door-control etc., outside the camera-management scope |

## Sequencing

**P0 — cheap, low-risk add-value (do first)**
- **#1 capability harvest** is the multiplier: it lets every later tab hide
  itself when the device returns `NotSupported`, so do it before the new tabs.
- **#2 imaging presets** and **#3 DeviceIO completion** are cheap, visible wins.

**P1 — flagship line for the next milestone**
- **#4 Profile G playback** is the highest-leverage item: `oxvif` already has
  `find_recordings` / `get_replay_uri`, so the remaining work is mostly the
  OxDM timeline view plus feeding the replay URI into the existing
  go2rtc / ffmpeg player. Make it the milestone headline.
- **#5 firmware upgrade** + **#6 backup/restore** slot into the Maintenance tab
  together, turning it from "reboot / factory-reset" into a real ops panel.

**P2 — larger bets, one milestone each**
- **#8 analytics** is the "settings panel → real monitoring tool" divide, but
  it's the most expensive and depends on #1 plus a metadata stream. Schedule it
  after Profile G.
- **#9 on-device recording config** shares the Recordings view with #4; doing
  both yields half an NVR.

**P3 — finishing / on demand**
- **#11** advanced device settings should be split across the existing
  network / users tabs rather than becoming a new view, to avoid bloat.
- **#14 audio** is only worth it once two-way audio is on the table, and the
  real blocker there is RTSP backchannel, not these config operations.
- **#16** access-control family: explicitly won't do.

## Cross-cutting UI principles

- **Capability gate first.** Once #1 lands, every new tab renders its NavLink
  only when the device's capabilities advertise the service — no dead buttons.
- **Destructive actions go through `ConfirmDialog`.** Firmware (#5), restore
  (#6) and recording-job deletion (#9) all set `dangerous: true`.
- **New view vs new settings tab.** Anything with a live preview or timeline
  (playback, analytics, PTZ tours) is a `View` enum variant + a `device_panel`
  NavLink; pure config forms (backup, storage, DDNS) are `SettingsTab`s.
- **i18n.** Every new user-visible string needs en / zh_tw / ru in lockstep, or
  `tests/i18n_tests.rs` fails.
