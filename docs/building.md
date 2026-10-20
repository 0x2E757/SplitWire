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
(`-Destination` picks another folder). `tools\release.ps1` builds Setup from it
([installer](building/installer.md)).

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
`maxversiontested`, and the window is per-monitor DPI aware. It also embeds the
[icon and description](building/icon-and-description.md) of both executables and the tray icons; the large icon
sizes are PNG files in `assets/icon`, encoded again by `tools\encode-icon.ps1` when the art changes.
