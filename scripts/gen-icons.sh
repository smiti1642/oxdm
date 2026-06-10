#!/usr/bin/env bash
# Regenerate the app-icon PNG from the master SVG.
#
#   assets/icons/oxdm.svg  -->  assets/icons/icon.png  (1024x1024, alpha preserved)
#
# Dioxus.toml points `[bundle] icon` at this single PNG; `dx bundle` generates
# the platform formats (.icns / .ico) from it at bundle time, so this is the
# only icon file we keep in the repo. Re-run only when the SVG changes.
#
# IMPORTANT: rasterize with sharp (preserves transparency). Do NOT use macOS
# `qlmanage` here — its thumbnail renderer flattens transparent areas to white,
# which gave the rounded corners a white edge. Needs Node (npx fetches sharp-cli).
set -euo pipefail

cd "$(dirname "$0")/.."
SVG=assets/icons/oxdm.svg
OUT=assets/icons/icon.png

npx --yes sharp-cli --input "$SVG" --output "$OUT" resize 1024 1024

echo "generated: $OUT ($(sips -g pixelWidth "$OUT" 2>/dev/null | tail -1 | tr -d ' '))"
