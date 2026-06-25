# OxDM <version>

<!--
  Release-notes template. Copy this into the GitHub draft release body and:
  - replace <version> and fill in "Changes"
  - replace #<issue> with the Fedora/Flatpak tracking issue number
  - delete any platform row that wasn't built for this release
-->

## Changes
-

## Downloads

| Platform | Asset |
|----------|-------|
| macOS (Apple Silicon) | `oxdm-<version>-macos-aarch64.dmg` |
| Windows (x86_64) | `oxdm-<version>-windows-x86_64.msi` |
| Linux — Ubuntu/Debian (x86_64) | `oxdm-<version>-ubuntu-x86_64.AppImage` |

## Linux (pre-release)

The Linux build is an **AppImage tested on Ubuntu 22.04+ / Debian-based**
distributions only.

- Make it executable first: `chmod +x oxdm-*.AppImage`
- Requires FUSE2: `sudo apt install libfuse2`
  (or run with `./oxdm-*.AppImage --appimage-extract-and-run`)
- **Fedora / RHEL-based distros are not yet supported.** Their WebKitGTK
  helper (`WebKitNetworkProcess`) lives at a different path, so an
  Ubuntu-built AppImage crashes on launch. Native Fedora/RHEL support is
  planned via Flatpak — tracking: #<issue>.
