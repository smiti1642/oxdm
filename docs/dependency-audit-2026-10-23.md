# Remaining dependency advisory review — 2026-10-23

Baseline: `03ccba8`, after the TLS/QUIC security updates. OxDM is a desktop app
for Linux, macOS and Windows; Android is not a supported build. The review uses
the checked-in lockfile, target-specific Cargo trees and the selected dependency
sources. Informational warnings remain visible; no advisory ignores were added.

## Results and action

| Finding | Selected path / platform | Disposition |
| --- | --- | --- |
| clipboard-win 5.4.1, yanked | OxDM -> arboard 3.6.1 -> clipboard-win, Windows | Update the lockfile to non-yanked 5.4.3. The upstream source replaces unsafe HTML fragment copying with checked slicing and extension, and adjusts clipboard-open retries. OxDM currently calls `set_text`, not the affected HTML reader, but there is a compatible maintained update, so retain neither the yanked version nor the defective reader. |
| fxhash 0.2.1, [RUSTSEC-2025-0057](https://rustsec.org/advisories/RUSTSEC-2025-0057.html) | Wry's Android-only `kuchiki` alias for kuchikiki 0.8.8-speedreader -> selectors 0.24 -> fxhash | Not selected for any of the three desktop targets. No unrelated Wry/HTML-parser migration. |
| rand 0.7.3, [RUSTSEC-2026-0097](https://rustsec.org/advisories/RUSTSEC-2026-0097.html) | Same Android-only parser -> selectors build dependency phf_codegen 0.8 -> phf_generator 0.8 -> rand 0.7 | Not selected for desktop builds. Additionally, phf_generator uses a fixed-seed `SmallRng`, not the reentrant `ThreadRng` implicated by the advisory. |
| paste 1.0.15, [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) | Dioxus desktop -> image -> ravif -> rav1e -> paste | Compiled procedural macro; the advisory reports discontinued maintenance, with no specific vulnerability. No camera-supplied tokens are compiled as macros. Keep visible pending an upstream codec/macro migration. |
| proc-macro-error 1.0.4, [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) | Linux GTK3/glib procedural macros | Build-time diagnostic macro with an unmaintained advisory, not runtime parsing of camera data. Keep visible pending the upstream GTK binding migration. |
| glib 0.18.5, [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) | Dioxus -> Wry/tao/GTK3/WebKitGTK -> glib, Linux only | A real upstream undefined-behavior defect, but no call of the affected API was found in the current application or selected dependency Rust sources. Retain the warning and explicit review limits below. |

The clipboard update does not establish a previously exploitable OxDM HTML
clipboard path. Its rationale is the Windows dependency's yanked status, the
upstream bounds fix, and the availability of a compatible replacement. Native
Windows and Linux CI now run for `develop` as well as `main`, with `--locked`
compilation/tests so they validate the reviewed dependency version.

The reviewed registry release maps to upstream commit
[`ed27008`](https://github.com/DoumanAsh/clipboard-win/blob/ed27008ae5d1f10cfbb266de039f0fb35fd05a56/src/raw.rs)
through its `.cargo_vcs_info.json`; `raw::get_html` is the repaired reader.

## glib applicability and limits

The [upstream fix](https://github.com/gtk-rs/gtk-rs-core/pull/1343) changes an
immutable out-pointer to a mutable one in `VariantStrIter::impl_get`. Calling the
iterator can produce undefined behavior and optimized-build crashes. The public
entry point is `Variant::array_iter_str`; its iterator constructor is crate-private.

The Linux selected dependency sources were searched for `array_iter_str` and
`VariantStrIter`. There were no references outside glib itself. Within glib,
the references define/export this API and exercise it in its own tests. OxDM
also contains no direct glib/Variant use. macOS and Windows do not select glib.

This is static source evidence, **not a formal reachability proof or a Linux
GUI runtime test**. It does not cover a later dependency change or arbitrary
macro-generated calls. Reassess if new Variant code, a new Linux integration, or
dependency updates introduce this API. The GTK3 binding family requires glib
0.18; the patched >=0.20 API family cannot simply replace it through a lockfile
update. The registry still has no 0.18 patch release beyond 0.18.5. Do not force
an incompatible glib version or hide the advisory to claim a clean audit.

## Reproduction

```sh
cargo audit
cargo tree --locked --target x86_64-pc-windows-msvc -i clipboard-win
cargo tree --locked --target x86_64-unknown-linux-gnu -i glib
cargo tree --locked --target all -i kuchikiki -e features
cargo tree --locked --target x86_64-unknown-linux-gnu -i kuchikiki
cargo tree --locked --target x86_64-pc-windows-msvc -i kuchikiki
cargo tree --locked --target aarch64-apple-darwin -i kuchikiki
cargo metadata --locked --format-version 1 \
  --filter-platform x86_64-unknown-linux-gnu > linux-metadata.json
```

The all-target parser tree identifies the optional-platform path; each desktop
parser tree prints `nothing to print`. Inspect `wry 0.53.5/Cargo.toml`:
`target.'cfg(target_os = "android")'.dependencies.kuchiki` pins
`package = "kuchikiki"` to `=0.8.8-speedreader`. Filter `linux-metadata.json`
through its resolve nodes before scanning package `src/**/*.rs`, including
`build.rs`, for the affected glib API. Metadata alone can include inactive
optional resolution entries; use Cargo trees for the selected feature path.

After the clipboard update, ordinary audit reports zero vulnerability entries,
three unmaintained warnings and two unsound warnings; the yanked warning is
gone. The remaining warnings are not suppressed. Re-run the source/path review
when changing the desktop framework, its image codecs, or native integration.
