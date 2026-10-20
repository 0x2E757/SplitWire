# Building and running

You need Rust with the `x86_64-pc-windows-msvc` target, the MSVC linker and the Windows SDK. Nothing else: the
package has no crates. `.cargo/config.toml` links the C runtime statically, so the package does not need the Visual
C++ Redistributable (`VCRUNTIME140.dll`). Any other target stops the build with `compile_error!`.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build.ps1
```

The engine runs in the foreground with `SplitWire.exe run`, a dry run with `--dry-run`; the other commands talk to
it over the pipe ([command line](building/command-line.md)).

`build.ps1` runs `cargo fmt --check`, `cargo test` and `cargo clippy` (skip them with `-SkipTests`; the other
[checks](building/checks.md) are run by hand), builds release and puts the package into `target\package`
(`-Destination` picks another folder).

## Package

Cargo names the files its own way (`splitwire.exe`, `splitwire-window.exe`: dots are not allowed in its names). The
files get their package names when copied according to the list in `tools\package-names.ps1`; the same list in
code is `src/platform/executables.rs`, and a test checks that they match.

| File | Purpose |
|---|---|
| `SplitWire.exe` | The service and the command line |
| `SplitWire.Window.exe` | The tray icon and the window |
| `wireguard.dll` | The official wireguard-nt library, unmodified ([vendor](../vendor/wireguard-nt/README.md)) |
| `wireguard-nt-LICENSE.txt` | Its licence |
| `README.md` | The project's README |

The programs must sit side by side: the service loads `wireguard.dll` only from its own folder.

`build/main.rs` embeds `build/window.manifest` into `SplitWire.Window.exe`: XAML Islands need its
`maxversiontested`, and the window is per-monitor DPI aware.

## Icons

`build/main.rs` draws the icons with `src/presentation/tray/raster.rs` and links them as a `.res` file
(`build/resource_file.rs`).

- `TrayArt`, the [tray](tray.md) icons, in `SplitWire.Window.exe`, in 16, 20, 24 and 32 px: `SplitArrow`, its
  branch at 65 %, snapped to whole pixels per size; a test compares them with the approved renders. Per taskbar
  (light, dark), connected is ink (#1B1B1B, #FFFFFF), warning amber (#C27C0E, #FCE100), error red
  (#C42B1C, #FF6B5E), off grey (#8A8A8A).
