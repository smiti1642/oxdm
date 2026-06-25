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
| Windows (x86_64) — installer | `oxdm-<version>-windows-x86_64.msi` |
| Windows (x86_64) — portable | `oxdm-<version>-windows-x86_64-portable.zip` |
| Linux — Ubuntu/Debian (x86_64) | `oxdm-<version>-ubuntu-x86_64.deb` |

## Windows

- **Installer** (`.msi`) — standard install with a Start-menu shortcut.
- **Portable** (`-portable.zip`) — unzip and run `oxdm.exe`; no install, no
  registry writes.
- Both use the **WebView2 runtime**, which is preinstalled on Windows 10/11.
  If the window stays blank on an old/stripped system, install the WebView2
  runtime from Microsoft.

## Linux (pre-release)

The Linux build is a **`.deb` for Ubuntu 24.04+ / Debian-based** distros
(built on Ubuntu 24.04). It depends on the system WebKitGTK, so install it
with a tool that resolves dependencies:

```bash
sudo apt install ./oxdm-<version>-ubuntu-x86_64.deb
```

- **Fedora / RHEL-based distros are not yet supported** (different WebKitGTK
  layout, no `.deb`). Native Fedora/RHEL support is planned via Flatpak —
  tracking: #<issue>.
