//! Build-time-only: on Windows targets, generate a multi-resolution
//! `icon.ico` from `assets/icons/icon.png` and embed it into the `.exe`
//! via the Windows resource section. This is what makes Explorer / the
//! Start Menu / pinned-taskbar shortcuts pick up the lens icon for the
//! file itself, independently of any MSI install.
//!
//! No-op on macOS / Linux. The runtime window icon (title bar + taskbar)
//! is set separately in `src/main.rs` via `WindowBuilder::with_window_icon`.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        embed_windows_icon();
    }
    println!("cargo:rerun-if-changed=assets/icons/icon.png");
    println!("cargo:rerun-if-changed=build.rs");
}

fn embed_windows_icon() {
    use image::imageops::FilterType;
    use std::{fs::File, io::BufWriter, path::PathBuf};

    let src = "assets/icons/icon.png";
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
    let ico_path = out_dir.join("icon.ico");

    let img = image::open(src).expect("read assets/icons/icon.png");

    // Standard set of sizes for a multi-resolution Windows ICO. 256 is the
    // largest Explorer ever asks for; 16-48 cover legacy small-icon paths.
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in &[16u32, 24, 32, 48, 64, 128, 256] {
        let resized = img.resize(size, size, FilterType::Lanczos3).to_rgba8();
        let entry =
            ico::IconImage::from_rgba_data(resized.width(), resized.height(), resized.into_raw());
        dir.add_entry(ico::IconDirEntry::encode(&entry).expect("encode ICO entry"));
    }

    let f = BufWriter::new(File::create(&ico_path).expect("create icon.ico in OUT_DIR"));
    dir.write(f).expect("write icon.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(&ico_path.to_string_lossy());
    res.compile().expect("embed Windows resource");
}
