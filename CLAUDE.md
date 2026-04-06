# OxDM — Development Guidelines

## Project overview

`oxdm` is a Dioxus desktop app for managing ONVIF IP cameras, built on top of `oxvif`.
Single crate, no workspace. Desktop-first (Dioxus 0.7.4 + Tauri backend).

## Before every commit

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo build
```

All three must pass cleanly before committing.

## Running locally

```
dx serve --platform desktop
```

Requires `dioxus-cli` (`cargo install dioxus-cli`).

## Rebuilding Tailwind CSS

```
npx @tailwindcss/cli -i tailwind.css -o assets/main.css --watch
```

`assets/main.css` is in `.gitignore` — always rebuild before committing UI changes.

## Architecture

```
src/
  main.rs         Entry point, Router definition, App component
  api.rs          Async wrappers around oxvif (discovery, device info, etc.)
  pages/
    mod.rs        Re-exports all pages
    home.rs       Home page — WS-Discovery scan + device list
    camera_detail.rs  Camera detail page — device info table
assets/
  main.css        Tailwind output (git-ignored, must build locally)
tailwind.css      Tailwind input (source of truth)
```

## Coding rules

- Components must be `fn Foo() -> Element` (PascalCase, `#[component]` attribute).
- All `oxvif` calls go in `src/api.rs` — pages only call `api::*` functions.
- Use `use_resource` for async data fetching; `use_signal` for local state.
- No `unwrap()` in component code — handle `None`/`Err` gracefully in the UI.
- Dark mode is a CSS class variant (`.dark`), not `prefers-color-scheme`.

## Adding a new page

1. Create `src/pages/<name>.rs` with a `#[component] pub fn <Name>() -> Element`.
2. Add `mod <name>; pub use <name>::<Name>;` to `src/pages/mod.rs`.
3. Add a `#[route("/<path>")]` variant to the `Route` enum in `src/main.rs`.
4. Add a link to the new page from an existing page.

## oxvif version

Pinned to `oxvif = "0.8.4"` (crates.io). To upgrade, update `Cargo.toml` and
re-verify that all `api.rs` call sites still compile.
