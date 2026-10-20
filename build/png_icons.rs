//! The app icon's large sizes as PNG files kept in `assets/icon` (TaskbarMetrics' approach). They
//! are encoded by `tools\encode-icon.ps1` from the pixels this build draws, and taken only while
//! they still match those pixels and stay small.

use std::path::{Path, PathBuf};

pub struct PngIcons {
    /// `assets/icon`: `<size>.png` and the fingerprint of the pixels they encode.
    assets: PathBuf,
    /// Where this build leaves the pixels for the script.
    pixels: PathBuf,
}

impl PngIcons {
    pub const SIZES: [u32; 3] = [96, 128, 256];
    /// All the PNG files together; the gradient tile takes about 14 KB.
    const LIMIT: u64 = 16 * 1024;
    const FINGERPRINT: &'static str = "pixels.fnv";

    pub fn new(root: &Path, out: &Path) -> Self {
        Self {
            assets: root.join("assets").join("icon"),
            pixels: out.join("icon"),
        }
    }

    pub fn assets(&self) -> &Path {
        &self.assets
    }

    /// The PNG of every size in `SIZES`, or what to do to get valid ones.
    pub fn load(&self, pixels: impl Fn(u32) -> Vec<u32>) -> Result<Vec<(u32, Vec<u8>)>, String> {
        let fingerprint = self.leave_pixels(pixels)?;
        let rerun = format!(
            "run: powershell -NoProfile -ExecutionPolicy Bypass -File tools\\encode-icon.ps1 -Pixels \"{}\"",
            self.pixels.display()
        );
        let saved =
            std::fs::read_to_string(self.assets.join(Self::FINGERPRINT)).unwrap_or_default();
        if saved.trim() != fingerprint {
            return Err(format!(
                "the PNG icons do not match the app icon in src/presentation/tray/app_art.rs; {rerun}"
            ));
        }
        let mut images = Vec::new();
        for size in Self::SIZES {
            let path = self.assets.join(format!("{size}.png"));
            let png = std::fs::read(&path)
                .map_err(|error| format!("{}: {error}; {rerun}", path.display()))?;
            if Self::dimensions(&png) != Some((size, size)) {
                return Err(format!(
                    "{} is not a {size} × {size} PNG; {rerun}",
                    path.display()
                ));
            }
            images.push((size, png));
        }
        let total: u64 = images.iter().map(|(_, png)| png.len() as u64).sum();
        if total > Self::LIMIT {
            return Err(format!(
                "the PNG icons take {total} bytes, more than {}",
                Self::LIMIT
            ));
        }
        Ok(images)
    }

    /// Writes `<size>.bgra` (`0xAARRGGBB`, rows top-down) for the script and returns the
    /// fingerprint of all of them, which the script stores next to the PNGs.
    fn leave_pixels(&self, pixels: impl Fn(u32) -> Vec<u32>) -> Result<String, String> {
        std::fs::create_dir_all(&self.pixels).map_err(|error| error.to_string())?;
        let mut hash = Fnv::new();
        for size in Self::SIZES {
            let bytes: Vec<u8> = pixels(size).iter().flat_map(|p| p.to_le_bytes()).collect();
            hash.write(&bytes);
            std::fs::write(self.pixels.join(format!("{size}.bgra")), &bytes)
                .map_err(|error| error.to_string())?;
        }
        let fingerprint = format!("{:016x}", hash.0);
        std::fs::write(self.pixels.join(Self::FINGERPRINT), &fingerprint)
            .map_err(|error| error.to_string())?;
        Ok(fingerprint)
    }

    /// Width and height from the IHDR chunk right after the PNG signature.
    fn dimensions(png: &[u8]) -> Option<(u32, u32)> {
        const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        if png.len() < 24 || png[..8] != SIGNATURE || &png[12..16] != b"IHDR" {
            return None;
        }
        let number = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
        Some((number(16), number(20)))
    }
}

/// FNV-1a, 64-bit: stable across Rust releases, unlike the std hasher.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ *byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
}
