## Platform notes

The bundles are **not code-signed**, so each OS shows a first-run warning. This
is expected — here's what to do per platform.

### macOS (Apple Silicon `.dmg`)

macOS reports **"oxdm is damaged and can't be opened"** (「oxdm」已損毀,無法打開).
The app is **not** damaged — this is Gatekeeper blocking an unsigned, un-notarized
app downloaded from the internet. To run it:

1. Open the `.dmg` and drag **oxdm** to **Applications**.
2. In Terminal, clear the quarantine flag:
   ```sh
   xattr -dr com.apple.quarantine /Applications/oxdm.app
   ```
3. Launch oxdm normally.

Apple Silicon (`aarch64`) only — no Intel build. On Intel, build from source.

### Windows (`.msi` / portable `.zip`)

SmartScreen shows **"Windows protected your PC"**. Click **More info** →
**Run anyway**. The bundles rely on the **WebView2 runtime** (preinstalled on
Windows 10/11); if the window stays blank, install WebView2 from Microsoft.
`x86_64` only.

### Linux (`.deb`)

Debian / Ubuntu **24.04+**, `x86_64`:
```sh
sudo apt install ./oxdm-<version>-ubuntu-x86_64.deb
```
It uses the system WebKitGTK (not a bundled one). **Fedora / RHEL are not yet
supported** as a prebuilt package (Flatpak planned). On any other distro, install
from crates.io — `cargo install oxvif-device-manager` — with the WebKitGTK / GTK /
xdo development libraries present.
