# Spec — HealthCheck Groups (batch conformance testing)

Status: **agreed, pre-implementation**. Branch: `feat/profile-g-playback`.

## 1. Motivation

`oxdm` is being used as a **community cross-brand ONVIF conformance tool**: users
run it against their own mixed-fleet IP cameras and report the results back, so
brand-specific quirks (e.g. Hanwha's `FindRecordings` occurrence-constraint
fault) can be captured and folded into oxvif's per-vendor compatibility work.

The existing batch Health Overview is an *ephemeral* checkbox selection over all
live devices. This spec upgrades it into **persistent, named, grouped device
lists** curated via right-click, each runnable and exportable, with a layered
credential model so different device batches can carry different credentials.

## 2. Concepts

- **List** — anything selectable in the Health view's left column. Two kinds:
  - **"All devices" (dynamic)** — not persisted; always reflects live
    `ctx.devices`; filterable. The current ephemeral view, retained.
  - **Group (saved)** — persisted, user-named, curated set of device
    references. Right-click adds devices here.
- **Device reference (`HealthDeviceRef`)** — a persisted pointer to a device
  (not a live `DeviceEntry`): `{ endpoint, addr, name }`.
- **Health run** — runs oxvif's read-only `HealthCheck` **plus** the active
  Profile G probe against a set of devices, producing per-device results.

## 3. UI layout

Health Overview becomes **left "lists" column + right panel**:

```
┌───────────────┬──────────────────────────────────────────────┐
│ LISTS         │  <selected list/group>        [Group creds]   │
│               │  ┌─ filters (All devices only) ─────────────┐ │
│ ▸ All devices │  │ source: All/Discovered/Manual            │ │
│               │  │ auth:   All/Logged-in/Failed/Unverified  │ │
│ GROUPS        │  └──────────────────────────────────────────┘ │
│ ▸ Hanwha      │  [✓] Front door  192.168.1.10  🔑  [cred:grp] │
│ ▸ Hikvision   │       Manufacturer Model · fw X               │
│ ▸ Site A      │       7 pass · 1 warn · 2 fail   S T G  G:… │ │
│ [+ New group] │  ...                                          │
│               │  [Redact] [Export report]  [Run selected (N)] │
└───────────────┴──────────────────────────────────────────────┘
```

- Selecting a list/group shows its devices on the right with Run + Export.
- Filters render **only for the "All devices" dynamic list**. Groups are
  already curated, so they show their members directly.

## 4. Data model

```rust
// state.rs
struct HealthGroup {
    id: String,                    // stable; survives rename (creds keyed by it)
    name: String,                  // user-editable
    devices: Vec<HealthDeviceRef>,
}

struct HealthDeviceRef {
    endpoint: String,              // WS-Discovery uuid; primary match key (may be empty)
    addr: String,                  // device service URL; fallback match + cred key
    name: String,                  // cached label for offline display
}

// Ctx gains:
health_groups: Signal<Vec<HealthGroup>>,
```

Group `id` is generated once at creation and never changes (rename edits only
`name`). This keeps credential entries attached across renames.

## 5. Persistence

- New file **`~/.oxdm/healthcheck.toml`** (same serde/toml pattern as
  `devices.toml`), storing the groups **without credentials**:

```toml
[[group]]
id = "g-1"
name = "Hanwha batch"

  [[group.device]]
  endpoint = "uuid:aaaa…"
  addr = "http://192.168.1.10/onvif/device_service"
  name = "Front door"
```

- `persist.rs`: `read_health_groups()` / `write_health_groups(&[HealthGroup])`.
- Loaded into `ctx.health_groups` at startup; saved on every mutation
  (add/remove/rename/reorder).

## 6. Device identity & matching (offline-safe)

For each `HealthDeviceRef`, resolve to a live `DeviceEntry`:

1. Match by `endpoint` when non-empty.
2. Else match by `addr`.
3. **No live match → render a gray "offline" row** using the cached `name` /
   `addr`; the device stays in the group (removable), because the list is
   persistent and devices may be offline when reviewed.

Rendering uses an **index render-key** (`{i}`) — never `addr` — because
`ctx.devices` can transiently hold duplicate addrs during a scan and dioxus
0.7.9 panics on duplicate sibling keys. Selection identity is `addr`/`endpoint`
(stable across reorder); these two concerns are deliberately separate.

## 7. Credential model

### 7.1 Resolution cascade

When device `D` is health-checked as part of group `G`, credentials resolve
high-to-low:

1. **Group-device override** — `G`'s per-device creds for `D`, if set.
2. **Group credentials** — `G`'s group-level creds, if set.
3. **App default** — existing `credentials_for(D)` (manual per-device override →
   global).

**Fallback is automatic**: a group (or device) with no creds set behaves exactly
as today. Simple groups need no credential setup. Backward compatible.

Key benefits:
- A **discovered** device can get device-specific creds *inside a group* without
  becoming a manual device — solves "multiple discovered batches, each needing
  different credentials, but global is a single value."
- The two group tiers (group-global + per-device override) cover any mix.

### 7.2 Scope

Group credentials apply **only to Health runs**. Live video, settings, PTZ, etc.
continue to use app-level `credentials_for` and are unaffected.

### 7.3 Storage

Extends the **single keychain JSON blob** (preserving the "one keychain prompt"
design):

```jsonc
{
  "global":  { ... },                       // existing: app global
  "devices": { "<addr>": { ... } },         // existing: manual per-device
  "groups": {                               // new
    "<groupId>": {
      "global":  { ... },                   // group-level creds
      "devices": { "<addr>": { ... } }      // per-device-in-group override
    }
  }
}
```

- Group creds keyed by stable **group id**; per-device by **addr**.
- Removing a group deletes its `groups.<id>` entry.

### 7.4 Credential UI

- Group selected → **"Group credentials"** entry in the right-panel header
  (reuses the existing credentials dialog body).
- Each device row → a **key icon** opening a per-device-in-group creds mini
  dialog.
- Each row shows a **source badge** — `device` / `group` / `app` — so the user
  sees which credentials will actually be used to connect.

## 8. Right-click "Add to HealthCheck list"

- New `CtxMenuItem` in `device_list.rs`'s `DeviceCard` context menu.
- Opens a **small picker dialog** (reuses `dialog_overlay`): lists existing
  groups + a **"New group…"** field. Selecting/creating adds a
  `HealthDeviceRef` built from the device's `{endpoint, addr, name}`.
- Adding a device already in the target group is a no-op (dedupe by
  endpoint/addr).

## 9. Health run behavior

Per device, the run collects (already implemented in `health_overview.rs`):

1. **Fingerprint** — `get_device_info` → structured
   `{manufacturer, model, firmware, serial, hardware_id}`; failure recorded.
2. **oxvif `HealthCheck`** — read-only; pass/warn/fail/skip + Profile S/T/G.
3. **Active Profile G probe** — really calls `search_recordings` +
   `get_replay_uri`, capturing the verbatim SOAP fault (the part oxvif's health
   check does not exercise).

Each device runs concurrently, wrapped in a **120 s per-device timeout** so a
non-responsive brand can't stall the batch. Credentials for each device resolve
via the §7 cascade (group context) or `credentials_for` (All-devices list).

### 9.1 "All devices" dynamic list specifics

- **Default filter: logged-in only** (`auth_status == Ok`).
- Filters: source (All/Discovered/Manual) + auth (All/Logged-in/Failed/
  Unverified) — mirrors the sidebar's filter vocabulary.
- **Selection is sticky** (survives scans/filter toggles; filtering only hides
  rows).
- **"Select all" applies to the currently-visible (filtered) rows only.**
- **Run acts on visible-and-selected devices.**

### 9.2 Group list specifics

- Runs the group's matched, online devices.
- Offline members are shown but skipped by the run (noted in the result).

## 10. Export (implemented — recap)

"Export report" → `rfd` save dialog → JSON bundle:

```jsonc
{
  "schema": "oxdm-health-batch/v1",
  "oxdm_version": "…", "oxvif_version": "…",
  "generated_at": "YYYY-MM-DDTHH:MM:SS",
  "redacted": false,
  "devices": [ { target, display_addr, name, fingerprint, fingerprint_error,
                 report /* HealthReport */, profile_g_probe } ]
}
```

Group export may additionally stamp the group name (P2).

## 11. Privacy / redaction (implemented — recap)

- "Redact IP / serial" toggle. On export: `serial_number` / `hardware_id`
  blanked, and a whole-JSON IPv4 scrub rewrites dotted quads to `x.x.x.x`
  (version strings, ms fields, timestamps survive — they aren't A.B.C.D).

## 12. Implementation — file changes

| File | Change |
|---|---|
| `state.rs` | `HealthGroup`, `HealthDeviceRef`; `Ctx.health_groups`; `group_credentials_for(group, device)` cascade helper |
| `persist.rs` | `read/write_health_groups` (`healthcheck.toml`); extend keychain blob with `groups`; group-cred read/write |
| `components/device_list.rs` | context-menu "Add to HealthCheck list" + picker dialog |
| `components/` (new or reuse) | group picker dialog; per-device-in-group creds mini dialog |
| `views/health_overview.rs` | left lists column + right panel; group rendering, offline rows, group/device cred entries + source badges; keep All-devices dynamic list with §9.1 behavior |
| `i18n/{en,zh_tw,ru}.rs` | new keys (×3 in lockstep) |
| `assets/main.css` | lists column + group/cred UI |

## 13. Phasing

- **P1 (core)** — group data model + `healthcheck.toml` persistence + right-click
  add (picker dialog) + left-lists/right-panel layout + per-group Run/Export +
  offline gray rows + **group & per-device-in-group credentials** (cascade,
  storage, source badges) + All-devices dynamic list per §9.1.
- **P2 (polish)** — rename/delete group, remove/move devices between groups,
  in-group filtering/selection reuse, group name in export.

(Per-group credentials were promoted from optional to **core** at the user's
request; both credential tiers ship in P1.)

## 14. Decisions log

1. Batch view = left lists column + right panel.
2. "All devices" dynamic list **and** saved groups **coexist**.
3. All-devices default filter = **logged-in only**; Run = **visible + selected**;
   selection sticky; render-key = index.
4. Groups persist to `healthcheck.toml`; identity = endpoint‖addr; offline rows shown.
5. Right-click add → **picker dialog** (groups + New group).
6. Per-group credentials = **core**; plus **per-device-in-group** override.
7. Credential cascade: device-in-group → group → app default; **auto-fallback**
   to app default when unset.
8. Group creds stored in the single keychain blob under `groups.<id>`; scoped to
   Health runs only.
9. Profile G actively probed; export bundle + IP/serial redaction (already built).
