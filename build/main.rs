//! Embeds the window's application manifest (XAML Islands need `maxversiontested`, without it
//! `WindowsXamlManager` refuses to start, and the window is per-monitor DPI aware) and the tray
//! icons: every state for a light and a dark taskbar, drawn by the same code the menus use.

#[path = "../src/presentation/tray/raster.rs"]
#[allow(dead_code)]
mod raster;
mod resource_file;
#[path = "../src/presentation/tray/split_arrow.rs"]
mod split_arrow;
#[path = "../src/presentation/tray/tray_art.rs"]
#[allow(dead_code)]
mod tray_art;

use resource_file::ResourceFile;
use std::path::PathBuf;
use tray_art::{Taskbar, TrayArt};

/// Cargo's name of each executable that gets a manifest, and the manifest.
const MANIFESTS: [(&str, &str); 1] = [("splitwire-window", "build/window.manifest")];
/// The executable that shows the tray icon.
const TRAY: &str = "splitwire-window";

fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    for (bin, manifest) in MANIFESTS {
        let path = root.join(manifest);
        println!("cargo:rerun-if-changed={}", path.display());
        println!("cargo:rustc-link-arg-bin={bin}=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin={bin}=/MANIFESTINPUT:{}",
            path.display()
        );
    }
    for source in [
        "build/resource_file.rs",
        "src/presentation/tray/raster.rs",
        "src/presentation/tray/split_arrow.rs",
        "src/presentation/tray/tray_art.rs",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(source).display());
    }
    let mut resources = ResourceFile::new();
    for state in TrayArt::STATES {
        for taskbar in Taskbar::BOTH {
            let images: Vec<(u32, Vec<u32>)> = TrayArt::SIZES
                .iter()
                .map(|&size| (size, TrayArt::pixels(state, taskbar, size)))
                .collect();
            resources = resources.icon_group(TrayArt::resource_id(state, taskbar), &images);
        }
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("set by Cargo"));
    let path = out.join("tray-icons.res");
    std::fs::write(&path, resources.bytes()).expect("the resource file is written");
    println!("cargo:rustc-link-arg-bin={TRAY}={}", path.display());
}
