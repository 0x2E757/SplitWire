# Icon and description

Part of [Building and running](../building.md).

`build/main.rs` draws the icons with `src/presentation/tray/raster.rs` and links them with a version block as a
`.res` file per executable (`build/resource_file.rs`), without `rc.exe`. Both kinds of icon are `SplitArrow`, its
branch at 65 %, snapped to whole pixels per size; a test compares them with the approved renders.

- `AppArt`, the app icon: icon group 1 of both executables, the lowest, so Explorer and the taskbar show it, and
  the window's class icon. The arrow in white on a tile from #FFA22B (top left) to #E5246F (bottom right), in 16,
  20, 24, 30, 32, 36, 40, 48, 64, 96, 128 and 256 px; both executables get every size, since Explorer shows both.
- `TrayArt`, the [tray](../tray.md) icons, in `SplitWire.Window.exe`, in 16, 20, 24 and 32 px. Per taskbar (light,
  dark), connected is ink (#1B1B1B, #FFFFFF), warning amber (#C27C0E, #FCE100), error red (#C42B1C, #FF6B5E),
  off grey (#8A8A8A).

## Large sizes as PNG

Sizes up to 64 px are embedded as bitmaps, 96, 128 and 256 px as PNG files from `assets/icon`
(`build/png_icons.rs`). The build leaves the pixels it draws as `<size>.bgra` in its `OUT_DIR` and takes the PNGs
only while the fingerprint of those pixels matches `assets/icon/pixels.fnv` and the three files together stay within
16 KB. Otherwise it stops and prints the command that encodes them again (System.Drawing) and writes the
fingerprint:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\encode-icon.ps1 -Pixels "<OUT_DIR>\icon"
```

A change to `AppArt`, `SplitArrow` or `raster.rs` that moves a pixel of those sizes needs it; commit `assets/icon`
with the change, so a clean checkout builds.

## Description

Windows shows the description (FileDescription) in Task Manager and the file's properties: "SplitWire" for
`SplitWire.Window.exe`, "SplitWire service" for `SplitWire.exe`. The publisher (CompanyName) is "SplitWire" for
both: without it, Settings → Apps → Startup shows the file name instead of the description. The version is
`Cargo.toml`'s.
