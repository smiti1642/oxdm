# Landing metamorph "clone my camera" in oxdm

> Serve a recorded clone of a real camera from an **in-app bound-port mock
> server** so oxdm drives it like any device, and surface a **structural quirk
> diff** of the clone vs oxvif's synthetic baseline. Companion to oxvif's
> `docs/active/metamorph-container-and-quirk-diff.md`.

## Design keystone

oxdm is both the **server host** and the **client**: a recorded clone is served
from an in-process `oxvif::mock::MockServer` (held alive in a pool), and oxdm's
existing "connect to a device by URL" path drives it. So the clone appears as a
normal device — **no per-view UI changes**; only a way to create/serve it and a
place to show quirks.

## Phase A — foundation (done)

Feature-gated on oxvif's `metamorph-server`; all additive, all tested.

| Piece | Where | What |
|-------|-------|------|
| oxvif dep | `Cargo.toml` | `oxvif` main dep now enables `metamorph-server` (pulls `metamorph` + `mock-server` → axum in the release binary). |
| Clone-server pool | `src/mock_servers.rs` (new) | `serve(label, store) -> url`, `stop(label)`, `url_for`, `running`. An `OnceLock` map holds each `MockServer` alive (Drop = shutdown), mirroring `sessions.rs`. |
| App wrappers | `src/api.rs` | `record_clone(addr, creds, label) -> Result<FixtureStore>` (dedicated recording session; creds/URL scrubbed by oxvif); `quirk_diff(&store) -> QuirkReport`. |
| Persistence | `src/persist.rs` | `clone_dir(label)` → `~/.oxdm/clones/<label>/`, `list_clones()` — mirror the baseline helpers. Save/load via `FixtureStore::{save,load}`. |
| Test | `mock_servers.rs` `#[cfg(test)]` | `record_serve_drive_diff_and_stop` — full loop through the app wrappers against a bound mock camera. `cargo test --bin oxdm`: 81 pass. |

## Phase B — UI slice (remaining)

1. **`DeviceEntry` marker** (`src/state.rs`): add `clone_of: Option<String>`
   (`Some(original_addr)` = a served clone). A clone entry must be **excluded
   from `devices.toml` persistence** (`persist.rs::write_devices_file` filters on
   `manual`) and from scan-offline marking (`device_list.rs::do_scan`). Touches
   every `DeviceEntry { … }` literal — add the field as the last one.
2. **"Clone this camera"** — a `CtxMenuItem` in `DeviceCard`'s context menu
   (`src/components/device_list.rs`, near "Add to group"): `spawn` →
   `api::record_clone(addr, creds, label)` with a progress signal →
   `store.save(persist::clone_dir(label)?)` → success/quirk toast.
3. **"Serve / open clone"** — load the store, `mock_servers::serve(label, store)`,
   then push a virtual `DeviceEntry { addr: url, clone_of: Some(addr), manual:
   true, credentials: None, .. }` into `ctx.devices`. Existing views drive it.
4. **Quirk view** — render `QuirkReport` (both types are `Serialize`) reusing the
   `health.rs` row styling (`status_class`/`row_message` are `pub(crate)`); a new
   `SettingsTab::Quirks` next to `Health`, or a `View::QuirkDiff`. Export JSON like
   `health_overview.rs`.
5. **Teardown** — `mock_servers::stop(label)` when the clone entry is removed.

## Honest limits to show in the UI (no-silent-caps)

- Clone covers the **standard read surface**, not the whole device — label it a
  "standard-surface snapshot," not a 100% clone.
- Recorded `GetServices` / stream URIs embed the **real camera's addresses**
  (URL pass-through), so some media/PTZ calls on the clone may route to the real
  device. Rewriting response URLs to the container is later work.
- Quirk diff is **structural only** (element-path presence), not values.

## How this validates "is it useful?"

The end-to-end loop is: clone a real camera → serve it → drive it in oxdm's
normal UI with the camera unplugged. If operating the clone ≈ operating the real
device (fidelity) **and** the quirk list flags real deviations, the feature earns
its place. Phase A proves the loop headlessly; Phase B lets a human try it.
