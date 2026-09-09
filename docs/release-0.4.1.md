# OxDM 0.4.1 — publication preparation

Prepared: 2026-09-09. Publication remains a manual maintainer action.

| Item | Value |
| --- | --- |
| crates.io package | `oxvif-device-manager` |
| Application / executable | OxDM / `oxdm` |
| Version prepared | 0.4.1 |
| ONVIF library | oxvif 0.16.0 from crates.io |
| Registry version checked before preparation | 0.4.0 |

## Release notes

OxDM 0.4.1 updates to oxvif 0.16.0 and improves device discovery:

- Returned discovery I/O errors now appear in the existing error toast instead
  of being reported as an empty scan. The existing three-round scan remains.
- Rediscovery retains a device's current address while it is still advertised,
  even when merged discovery responses reorder the available addresses.
- The About dialog and new Quirks baselines identify oxvif 0.16.0. Baselines
  from 0.15 still load and show the existing version-mismatch warning.
- Compatible dependency updates resolve four vulnerability entries reported
  by the initial audit and remove several other dependency warnings.
- The About dialog and README link to Buy me a coffee. The About button is
  localized in English, Traditional Chinese and Russian.
- Window-icon RGB decoding uses fixed-size array chunks to satisfy Rust 1.98's
  new Clippy lint while preserving its output.

This release does not add a network-interface picker or private-CA settings.
Some upstream discovery send/receive errors are still ignored.

## Validation

The final 0.4.1 implementation passed formatting, Clippy with warnings denied,
a Windows build, and all 174 tests on Rust 1.97.0. The seven localization tests
also passed after adding the support label to the required-key list.
The complete desktop application was not tested on oxvif's minimum Rust 1.88.
The [main CI run](https://github.com/smiti1642/oxdm/actions/runs/34332149169)
also passed formatting, Clippy, build and tests on Linux. Real-camera multicast,
macOS runtime behavior and installer acceptance were not validated locally.

`cargo outdated` was run before the upgrade, and `cargo outdated --depth 1`
after it. `cargo audit --json` reports **0 vulnerabilities and 5 warnings**;
the warnings remain open upstream work (fxhash, paste, proc-macro-error, glib
0.18 and rand 0.7). No audit ignore rules were added. See the
[upgrade plan](oxvif-0.16-upgrade-plan.md),
[audit report](oxvif-0.16-audit.json), and
[outdated report](oxvif-0.16-outdated.txt).

`cargo publish --dry-run --locked --allow-dirty` passed for 0.4.1 on Windows
with Rust 1.97.0. Cargo packaged 89 files (1.4 MiB uncompressed, 428.4 KiB
compressed), compiled the extracted crate, and aborted the upload as requested
by dry-run mode. The package includes README, CHANGELOG, Cargo.lock, runtime
CSS/icon assets and the new discovery regression tests. The `docs/` directory
is intentionally excluded from the published crate.

The audit was repeated after the version bump with the same result. The
preparation used `--allow-dirty` because these changes were not yet committed;
the manual publication commands below assume they have been reviewed and
committed. The dry-run did not upload to the registry or create a release tag.

## CI failure investigation

The [main CI run for the FUNDING.yml commit](https://github.com/smiti1642/oxdm/actions/runs/34331335486)
failed in Clippy, after formatting and system dependency installation passed.
Rust 1.98 reported `clippy::chunks_exact_to_as_chunks` at `src/main.rs:45`;
`-D warnings` made the new lint fatal. FUNDING.yml was not the failing input.
The fix uses `buf.as_chunks::<3>().0`, preserving the previous behavior of
processing complete RGB triples and ignoring any remainder. Local Rust 1.97
did not report that new lint. The subsequent main CI run linked above passed
all checks on hosted stable Rust.

## GitHub bundles

The release workflow now installs dioxus-cli 0.7.9 to match the exact Dioxus
dependency in Cargo.toml; the previous workflow still installed 0.7.5.
Pushing the `v0.4.1` tag builds a macOS ARM64 DMG, a Windows x86-64 MSI and
portable ZIP, and an Ubuntu x86-64 DEB. Jobs attach artifacts to a draft GitHub
Release for maintainer review. crates.io publication remains a separate manual
action.

## Manual publication

Run from the OxDM repository. Review and commit the intended release changes
first so Cargo can package a clean checkout, including the new regression tests.

```sh
cargo publish --dry-run --locked
cargo publish --locked
```

Cargo must already be authenticated to crates.io. The package name is
`oxvif-device-manager`; there is no need to publish the separate oxvif library
again. This repository uses its published 0.16.0 dependency.

After publication, users can install this version with:

```sh
cargo install oxvif-device-manager --version 0.4.1 --locked
```

Publishing to crates.io uploads the source crate. GitHub releases and platform
installer bundles are separate work; do not list new downloads until those
artifacts have been built and checked.
