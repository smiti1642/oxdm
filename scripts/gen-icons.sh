#!/usr/bin/env bash
# Regenerate the app-icon PNGs from the master SVG.
#
#   assets/icons/oxdm.svg  -->  assets/icons/icon.png      (1024x1024)
#                          -->  assets/icons/icon-512.png  (512x512)
#                          -->  assets/icons/icon-256.png  (256x256)
#                          -->  assets/icons/icon-128.png  (128x128)
#                          -->  assets/icons/icon-32.png   (32x32)
#                          (all RGBA, alpha preserved)
#
# Dioxus.toml's `[bundle] icon` lists this whole set; `dx bundle` files each
# PNG into the Linux hicolor icon dir by its pixel size (so the desktop-menu /
# taskbar icon resolves across DEs) and generates the platform formats
# (.icns / .ico) from the set. Re-run only when the SVG changes.
#
# IMPORTANT: rasterize with sharp (preserves transparency). Do NOT use macOS
# `qlmanage` here — its thumbnail renderer flattens transparent areas to white,
# which gave the rounded corners a white edge. Needs Node (npx fetches sharp-cli).
set -euo pipefail

cd "$(dirname "$0")/.."
SVG=assets/icons/oxdm.svg

for size in 32 128 256 512 1024; do
    if [ "$size" = "1024" ]; then
        out="assets/icons/icon.png"
    else
        out="assets/icons/icon-$size.png"
    fi
    npx --yes sharp-cli --input "$SVG" --output "$out" resize "$size" "$size"
    echo "generated: $out (${size}x${size})"
done
