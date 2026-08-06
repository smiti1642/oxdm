# OxDM — Roadmap

A prioritised backlog of ONVIF coverage gaps and the UI work needed to surface
them.

**Rewritten 2026-08-05, against oxvif 0.15.0.** The previous edition was
sequenced around a constraint that has largely dissolved. It scored every item
on *two* costs — the protocol work in `oxvif` and the UI work in OxDM — and the
oxvif column was usually the binding one: firmware upgrade was `oxvif M`,
recording configuration `oxvif L`, PTZ preset tours `oxvif M`. Almost all of
that has since shipped. The question is no longer "what should oxvif implement"
but "what should OxDM surface".

## The measurement this is derived from

Counted 2026-08-05 against oxvif 0.15.0, by intersecting every `pub async fn`
on `OnvifSession` with every `s.<method>(` call site under `src/`:

| | |
|---|---|
| `OnvifSession` methods | **159** |
| called from OxDM | **60** |
| never called | **99** |

That ratio is the roadmap. OxDM drives 38% of the protocol surface it already
depends on, and the unused 62% needs no new protocol work — only a place to
put it.

Two consequences worth stating plainly:

- **An item's oxvif cost is now usually zero, so it no longer discriminates.**
  Tier 0 below is defined by exactly that property: nothing in it requires a
  new oxvif release. Sequencing inside Tier 0 is about UI cost and user value
  alone.
- **A "new feature" is now mostly a rendering problem.** PTZ preset tours are
  seven working client methods and zero pixels. That is a different kind of
  task from what this file used to describe, and it is a much cheaper one.

## Scoring

- **Value** — usefulness for IP-camera management, 1 (niche) … 5 (flagship).
- **oxdm cost** — S(mall) / M(edium) / L(arge) UI effort.
- **oxvif** — `—` when the protocol side already exists at 0.15.0.

---

## Tier 0 — catch up to oxvif 0.15 (no oxvif work)

Every row here is reachable today. Nothing waits on a crates.io release.

| # | Item | Value | oxdm | oxvif | Landing |
|---|------|:----:|:----:|:----:|---------|
| 1 | ~~**Capability-driven UI** — hide what the device does not support~~ | 4 | S | — | **Landed 2026-08-06** — `api::DeviceGate` |
| 2 | **PTZ completeness** — absolute / relative move, live position readout, node limits | **5** | M | — | `views/ptz.rs` |
| 3 | **PTZ preset tours** | 3 | M | — | A "Tours" sub-tab in `views/ptz.rs` |
| 4 | **Imaging: read the move options before driving the motor** | 3 | S | — | `views/imaging.rs` focus block |
| 5 | **Firmware upgrade + backup / restore** | 4 | M | — | `settings/maintenance.rs` |
| 6 | **On-device recording configuration** (Profile G write side) | 4 | L | — | A "Schedule" sub-tab in `views/recordings.rs` |
| 7 | **Audio configuration** | 2 | M | — | New `SettingsTab` or a block in the encoder view |
| 8 | **Media2 parity** — profiles, stream / snapshot URI, source + metadata config | 3 | M | — | Cross-cutting; no new view |
| 9 | **Storage configuration** | 3 | S | — | New `SettingsTab` |
| 10 | **Multi-sensor audit of OxDM's own views** | 4 | S | — | Cross-cutting; see below |

### 1 — Capability-driven UI — **landed 2026-08-06**

`api::DeviceGate` decides which per-device entry points to offer;
`components/device_panel.rs` renders against it. Both layers are read, because
they answer different questions and neither subsumes the other: the device-level
`GetCapabilities` answers *"is there a service URL"*, and a service's own
`GetServiceCapabilities` answers *"what can that service actually do"* — e.g.
whether PTZ supports absolute move at all, which decides half of item 2.

Gated today: OSD, IO Control, Events and Recordings (NavLinks), plus the Imaging
and PTZ jump buttons on each profile thumbnail. Device Settings is not gated —
device management is mandatory on a conformant device, so there is nothing to
gate it on and nothing to fall back to. Live Video is not gated either; the
thumbnail grid already resolves to `no_profiles` when Media1 is absent.

Only OSD needs layer 2 so far (`MediaServiceCapabilities.osd`); the other six
are answered by layer 1 alone. The remaining eight `*_get_service_capabilities`
methods are wired to nothing yet — that is deliberate, not an oversight. Add the
call when a row below needs the answer, per item 2's `status_position` /
`move_and_track`, rather than paying eight round-trips for booleans no one reads.

**The invariant to preserve when extending it: silence is not a denial.** A
field goes `false` only on a *positive* statement — no service URL, or an
attribute explicitly `false`. An omitted attribute, a faulted
`GetServiceCapabilities`, an unreachable device or a still-pending probe all
leave the entry point offered. Hiding a working feature is unrecoverable from
the UI; showing a dead one costs a click and lands on an empty state that
already exists.

**Two claims in this section's previous text were wrong** — both stated before
the code was read, and both listed under Corrections below.

### 2 — PTZ completeness

OxDM's PTZ view drives seven methods: continuous move, stop, get/set/remove/goto
preset, goto home. oxvif offers twenty-seven. Unused today:

`ptz_absolute_move`, `ptz_relative_move`, `ptz_get_status`, `ptz_get_nodes`,
`ptz_get_node`, `ptz_get_configurations`, `ptz_get_configuration`,
`ptz_get_configuration_options`, `ptz_set_configuration`,
`ptz_set_home_position`, `ptz_get_compatible_configurations`,
`ptz_send_auxiliary_command`, plus the seven tour methods in item 3.

The two that change the view most:

- **`ptz_get_status`** — a live pan/tilt/zoom readout with a move state. The
  view currently drives a head it cannot see. 0.15 also made the mock report a
  real clock here instead of a frozen timestamp, so it is testable.
- **`ptz_absolute_move` + `ptz_get_node`** — absolute positioning needs the
  node's coordinate spaces and limits to be meaningful. Read the node, then
  offer the control; do not offer it blind.

Scored 5 because PTZ is the one view where a user expects to see what the device
is doing, and it is the largest single unused block in the measurement.

### 3 — PTZ preset tours

Seven operations, all new in oxvif 0.15 and all stateful in the mock (a tour
created by `CreatePresetTour` comes back from a later `GetPresetTours`, and
`OperatePresetTour` moves observable state). That makes the mock a real
integration harness for this feature, not a fixture printer — so this can be
built and tested end to end with no camera.

This was `P2 / oxvif M` in the previous edition. The oxvif half is done.

### 4 — Imaging: read the move options before driving the motor

`api::imaging_move` sends a continuous focus move with a speed OxDM never
validates, because it never calls `imaging_get_move_options`. The device
declares the legal speed range there and OxDM does not ask.

Small, and worth doing early for a second reason: `GetMoveOptions` is one of the
rows oxvif 0.15's schema-shape check corrected (the mock had rendered the focus
ranges as `PositionSpace` / `SpeedSpace`; `tt:AbsoluteFocusOptions` declares
`Position` then `Speed`). Exercising it from OxDM puts a second pair of eyes on
a path that was wrong in three artefacts at once until recently.

`imaging_get_status` (focus position + move status) is the natural companion.

### 5 — Firmware upgrade + backup / restore

Both were `oxvif M` in the previous edition. Both already exist:

- `start_firmware_upgrade` — `src/client/device.rs:878`
- `start_system_restore` — `src/client/device.rs:891`
- `SystemUris::system_backup_uri` — the download half of backup; oxvif has no
  `GetSystemBackup`, and does not need one, because the URI path is what a
  desktop app wants anyway (stream to a file, show progress).

Both are destructive and go through `ConfirmDialog` with `dangerous: true`.
Restore especially: it is the one control in the app that can leave a camera
unreachable.

### 6 — On-device recording configuration

`create_recording`, `delete_recording`, `create_track`, `delete_track`,
`create_recording_job`, `delete_recording_job`, `set_recording_job_mode`,
`get_recording_jobs`, `get_recording_job_state`, `get_recordings`,
`get_recording_search_results`, `find_recordings`, `end_search` — all present,
none called.

`View::Recordings` and `views/recordings.rs` already exist (the previous edition
still listed creating them as future work), and `tests/recordings_smoke.rs`
drives search + replay against the mock. So this is a sub-tab on a view that is
already there, not a new view.

Largest UI cost in Tier 0. It is half an NVR.

### 7 — Audio configuration

OxDM calls **no** audio method at all. oxvif has the full family on both
services, and 0.15 gave the mock a real audio catalogue rather than static
fixtures, so the round trip is testable.

Value stays 2: config-only audio is genuinely low-value without two-way audio,
and the blocker there is an RTSP backchannel, not these operations. Listed so
the gap is recorded, not because it should jump the queue.

### 8 — Media2 parity

OxDM is Media1 almost everywhere; the single Media2 call is
`set_video_encoder_configuration_media2`, used only to reroute H265 writes.
Unused: `get_profiles_media2`, `get_stream_uri_media2`,
`get_snapshot_uri_media2`, the Media2 encoder / source / metadata configuration
family, `get_video_encoder_instances_media2`, `get_video_source_modes_media2`.

Worth doing incrementally rather than as a project: prefer Media2 where the
device advertises it (item 1 tells you), fall back to Media1. Note 0.15 made
the mock's Media2 `GetProfiles` inline full configurations rather than emit bare
tokens, which is what a conformant device does — so `MediaProfile2` fields that
were permanently `None` against the mock are now exercisable.

### 9 — Storage configuration

`get_storage_configurations` / `set_storage_configuration`, unused. The mock
seeds three entries as of 0.15 (SD, NAS, CIFS), which is enough to build a real
list view against.

### 10 — Multi-sensor audit of OxDM's own views

Not a feature — a correctness sweep, and the one Tier 0 item that can find bugs
rather than add surface.

oxvif 0.15's headline breaking change was making `config_token` **required** on
five options getters, because a token-less per-channel query is answered for the
device's *default* channel and is indistinguishable from correct on a
single-sensor camera. Measured on a real two-sensor device: a token-less
`GetVideoEncoderConfigurationOptions` returned lens 0's resolution list, which a
caller would then show for lens 1, whose real maximum is lower.

That change caught OxDM's one affected call site at compile time. What it cannot
catch is OxDM's own fallbacks, which are lexically fine and silently
channel-wrong:

- `api::get_video_source_token` falls back to `profiles.first()` when the
  selected profile is stale. Documented and deliberate, but on a dual-lens
  camera it silently shows lens 0's image settings.
- `views/video_encoder.rs` falls back to the first profile carrying any
  video-encoder token, the same way.

The audit: for each per-channel view, decide whether the fallback should be
"lens 0" or "tell the user nothing is selected", and make a two-channel fixture
where the channels *disagree* on the value the assertion reads. A single-sensor
fixture passes just as well against code that ignores the token entirely.

---

## Tier 1 — needs oxvif work first

Each of these is blocked on a protocol addition, so each implies an oxvif
release before OxDM can start. Verified absent from `src/client/` at 0.15.0.

| # | Item | Value | oxvif | oxdm | Landing |
|---|------|:----:|:----:|:----:|---------|
| 11 | **Analytics rules** — modules + rules + metadata | **5** | L | L | New `View::Analytics`: rule list + bounding-box overlay |
| 12 | **Event broker / MQTT** — `Get/Add/DeleteEventBrokers` | 3 | M | M | A forwarding panel atop `views/events.rs` |
| 13 | **Imaging scene presets** — `timg:GetPresets` / `Get/SetCurrentPreset` | 3 | S | S | Dropdown in `views/imaging.rs` |
| 14 | **DeviceIO completion** — `GetSerialPorts`, `tmd:GetVideoSources` | 2 | S | S | Extend `views/io_control.rs` |
| 15 | **Multicast streaming control** — `Start/StopMulticastStreaming` | 2 | S | S | Toggle in the `live_video.rs` RTSP tab |
| 16 | **Events Seek / Pause / Resume** | 2 | S | S | Controls on the existing events log |
| 17 | **Unicast discovery `Resolve`** | 1 | S | — | No UI; enterprise-subnet use |

Note on **13**: oxvif has `ptz_get_presets` (`tptz:GetPresets`) but nothing in
the Imaging namespace. They are unrelated operations that share a name — a
scene preset (Day / Night / Indoor) is not a PTZ position.

Note on **14**: the previous edition filed this as a documentation fix, on the
grounds that relay and digital-input operations already existed via the Device
service. That is no longer the whole story. oxvif 0.15 moved `GetDigitalInputs`
onto the **DeviceIO endpoint**, where the schema puts it, and taught the mock to
answer one. So DeviceIO is now a real endpoint in this stack, and the remaining
gap is genuinely two missing operations rather than a mislabelled table.

---

## Won't do

- **Receiver / access-control family (Profiles A/C/D).** Door control and
  credential management are outside camera management. Unchanged from the
  previous edition.

---

## Deferred, and why it is not in either tier

**ONVIF replay-aware playback.** Many cameras' replay RTSP requires
`Require: onvif-replay` and `Range: clock=` headers that go2rtc / ffmpeg / VLC
do not send, so the frame stays blank. This is not an ONVIF-operation gap — the
replay URI is already retrievable and OxDM already offers "copy replay URI" —
it is an RTSP client problem. The fix is a replay-aware client (Rust `retina`
can send custom headers and a `Range`) bridged into the WebView via a go2rtc
`exec` source, with `ExportRecordedData` → download → play file as the
fallback for export-only devices.

Kept out of the tiers because it is the only item whose cost is dominated by
something neither crate is about, and because item 6 delivers most of the
Recordings view's value without it.

---

## Sequencing

**Item 1 — done (2026-08-06).** Cheap, OxDM-only, and the precondition for every
row that adds an entry point.

**Now — items 2, 4, 10 together.** All three are about the same thing: the
views OxDM already has do not ask the device enough. 2 and 4 add the missing
reads; 10 checks the reads it already makes are aimed at the right channel.
They share fixtures, and 10 is the only item that can find an existing bug.

**Then — item 5**, which turns Maintenance from "reboot / factory reset" into
an ops panel, and is self-contained.

**Milestone after that — item 6**, the largest and the one that changes what
OxDM *is*. Items 3, 8 and 9 are good filler around it: each is independently
shippable and none blocks the others.

**Only then reconsider Tier 1.** Opening an oxvif release for item 13 while 99
of its methods are unrendered is the wrong trade. The exception is item 11
(analytics), which is the "settings panel → monitoring tool" divide and is worth
its own milestone in both crates — but it should follow item 1, because it needs
the capability gate more than anything else here.

---

## Cross-cutting UI principles

- **Capability gate first.** Item 1 landed, so every new tab gets a field on
  `api::DeviceGate` and renders its NavLink only when the device advertises the
  service *and* its `GetServiceCapabilities` does not deny the feature — no dead
  buttons. Put the decision in `DeviceGate::from_caps`, which is unit-tested
  without a device; oxvif's mock advertises every service, so a mock-driven test
  can only ever prove the all-true case.
- **Destructive actions go through `ConfirmDialog` with `dangerous: true`** —
  firmware (5), restore (5), recording-job deletion (6).
- **New view vs new settings tab.** Anything with a live preview or timeline
  (analytics, PTZ tours) is a `View` variant plus a `device_panel` NavLink;
  pure config forms (backup, storage, audio) are `SettingsTab`s.
- **Send a token on every per-channel call.** Not "send the profile token" —
  send *a* token. See item 10; this is the rule oxvif 0.15 enforced at the type
  level, and OxDM should not reintroduce it above the API boundary.
- **i18n.** Every new user-visible string needs en / zh_tw / ru in lockstep, or
  `tests/i18n_tests.rs` fails.
- **A new oxvif version is a roadmap event.** Six items on this list lost their
  entire oxvif cost between 0.13 and 0.15 without anyone editing the file — see
  the corrections below. When upgrading the dependency, re-run the 60-of-159
  measurement above before assuming any row's cost still holds.

---

## Corrections

Recorded rather than silently overwritten, because each was true when written
and each misled while it was stale.

### Found while implementing item 1 (2026-08-06)

Both were written into the 2026-08-05 rewrite from a reading of the *services*
oxvif exposes, without opening `device_panel.rs`. Neither survived contact.

- **"`device_panel.rs` renders all nine NavLinks unconditionally."** It renders
  **five** NavLinks — Device Settings, OSD, IO Control, Events, Recordings. Live
  Video, Imaging and PTZ are not NavLinks at all: they are per-profile jump
  buttons in each thumbnail card's footer, and there is no ninth entry point.
  The mistake mattered — it put the whole item in the wrong component and made
  the work look like one uniform list when it is two mechanisms.
- **"The one place OxDM gates on anything is `api.rs:1608`."** PTZ was already
  feature-detected: `views/ptz.rs:33` calls `api::has_ptz_service`, which reads
  `capabilities().ptz.url`, and renders a `ptz_unavailable` empty state. So the
  claim that a fixed dome "shows a PTZ tab that does nothing when clicked" was
  wrong about the *consequence* as well — it showed a button that explained
  itself. That existing empty state is now the fallback for when the gate fails
  open, which is why item 1 did not remove it.

### To the previous edition

- **`device_panel.rs` was filed under `views/`.** It is
  `src/components/device_panel.rs`.
- **Item 4 (Profile G playback) was listed as needing a new `View::Recordings`
  and a new `views/recordings.rs`.** Both already exist, along with
  `tests/recordings_smoke.rs`. The item had partly shipped.
- **Six items were scored with a non-zero oxvif cost and are `—` at 0.15.0:**
  capability harvest (`S`), firmware upgrade (`M`), backup / restore (`M`), PTZ
  preset tours (`M`), audio configuration (`M`) and on-device recording
  configuration (`L`).

  Only two of the six were unblocked *by* 0.15 — the nine
  `GetServiceCapabilities` and the seven preset-tour operations. The other four
  had been reachable for one or more releases and nobody re-derived the column.

  The worst case is audio, scored `oxvif M`: `get_audio_sources`,
  `get_audio_source_configurations`, `get_audio_encoder_configurations`,
  `get_audio_encoder_configuration`, `set_audio_encoder_configuration` and
  `get_audio_encoder_configuration_options` all shipped in **oxvif 0.2.0, on
  2026-04-02** — thirteen releases before this rewrite. The same 0.2.0 entry
  also shipped `ptz_get_nodes`, `ptz_get_configurations` and
  `ptz_get_configuration`, three of the reads item 2 is built on.

  This is the failure the last cross-cutting principle above exists to prevent,
  and it is not a small one: a stale cost column does not merely misprice an
  item, it hides work that is already free.
- **DeviceIO was described as a documentation fix.** See the note on item 14.
- **The header linked to `oxvif/ROADMAP.md` on branch `main`,** described as
  where the protocol side is tracked. Both halves are wrong: oxvif's default
  branch is `master`, and it has no `ROADMAP.md` on any branch. The protocol
  side is not tracked anywhere, which is part of why the oxvif costs here went
  stale unnoticed. The link is dropped rather than repointed.

One item needs no work at all and is listed nowhere above, because it already
landed for free: oxvif 0.15 added **ten** health checks under
`Category::Services` (`service_caps_*` plus `service_caps_self_consistent`).
OxDM's health view tallies checks generically by id, so all ten already render
without an OxDM change.
