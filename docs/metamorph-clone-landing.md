# Landing metamorph "clone my camera" in oxdm

> Serve a recorded clone of a real camera from an **in-app bound-port mock
> server** so oxdm drives it like any device, and surface a **structural quirk
> diff** of the clone vs oxvif's synthetic baseline. Companion to oxvif's
> `docs/active/metamorph-container-and-quirk-diff.md`.

> **Status — shipped.** Both phases below are implemented and on `main`:
> Phase A (foundation) and Phase B (the full UI slice — clone action, virtual
> mock device, saved-mocks reopen, and the Quirks tab with a side-by-side,
> word-level diff and JSON export). The honest-limits section still holds and is
> surfaced in the UI. Later work: URL rewriting so replayed media/PTZ addresses
> point at the container instead of the real camera.

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

## Phase B — UI slice (done)

1. **`DeviceEntry` marker** (`src/state.rs`): `clone_of: Option<String>`
   (`Some(original_addr)` = a served clone). Clone entries are **excluded from
   `devices.toml` persistence** (`persist.rs::write_devices_file` filters on
   `manual && clone_of.is_none()`) and from scan-offline marking — they are
   ephemeral loopback devices that live only while served.
2. **"Clone this camera"** — a `CtxMenuItem` in `DeviceCard`'s context menu
   (`src/components/device_list.rs`), shown only for real devices (`!is_clone`):
   `spawn` → `api::record_clone(addr, creds, label)` → best-effort
   `store.save(persist::clone_dir(label))` → `mock_servers::serve(store)` → push
   the virtual device and select it. Toasts report progress and the recorded-op
   count.
3. **Virtual mock device** — `mock_servers::serve(store)` starts a bound
   `MockServer` replay server (held in an `OnceLock` pool, keyed by served URL)
   and returns its device URL. A virtual `DeviceEntry { addr: url, clone_of:
   Some(addr), manual: true, credentials: None, .. }` is pushed into
   `ctx.devices`, labeled **"mock"** (`clone_suffix`), and every existing view
   drives it unchanged.
4. **Saved-mocks reopen** — the `SavedMocks` list in the Manual tab
   (`device_list.rs`) shows one row per `~/.oxdm/clones/<name>/` that isn't
   already being served (`mock_servers::active_labels` filters the running ones
   out). Clicking a row loads its fixtures, serves a replay server, and adds it
   as a mock device.
5. **Quirks tab** — `SettingsTab::Quirks` (`src/views/settings/quirks.rs`),
   present only for a served clone (gated on `clone_of.is_some()` in
   `main_content.rs`). It renders `mock_servers::quirks(url)` (a `QuirkReport`)
   as a list of operations whose response shape drifts from oxvif's reference
   response; each row expands into a **git-style left/right line diff** of the
   two SOAP responses with **word-level (intra-line) highlighting**, and checked
   rows **export to timestamped JSON**. An honest scope note sits above the list.
6. **Teardown** — `mock_servers::stop(url)` runs when a clone device is removed,
   dropping its `MockServer` (Drop shuts the bound port down).

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
