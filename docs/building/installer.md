# Installer

Part of [Building and running](../building.md).

```powershell
winget install JRSoftware.InnoSetup --scope user
powershell -NoProfile -ExecutionPolicy Bypass -File tools\release.ps1
```

`release.ps1` runs [`build.ps1`](../building.md) into `target\dist\package`, so a copy running from
`target\package` keeps running, and writes `SplitWire-Setup-<version>.exe` (from `installer\splitwire.iss`) and
`SHA256SUMS.txt` to `target\dist`. The version is the one in `Cargo.toml`. On GitHub,
`.github/workflows/release.yml` does the same for a pushed tag `v<version>` that matches `Cargo.toml` and attaches
both files to a draft release.

## Setup

Asks for administrator rights once; x64 Windows 10 1903 or later, in English or Russian.

1. A running window is asked to end through `Local\SplitWire.Window.Exit` ([tray](../tray.md)). For an update,
   `net stop SplitWire` stops the service: every profile disconnects and stays in `connected=`
   ([engine](../engine.md)).
2. The package goes into `Program Files\SplitWire`, the only folder `install` accepts, with a Start menu shortcut
   to `SplitWire.Window.exe`.
3. `SplitWire.exe install` registers the service, or points it at this copy again, and starts it. An update runs
   `install --restore`, which connects again every profile that was connected, whatever its `connect_at_boot`.
4. "Start SplitWire at sign-in", offered on a fresh install and on by default, writes the window's Run value with
   `reg.exe` as the user who started Setup. An update leaves the value as the window's switch last set it.
5. `SplitWire.Window.exe --tray` starts as that user, unelevated; "Open SplitWire" on the last page shows the
   window.

An update is told by the `SplitWire` service, so a copy put into `Program Files\SplitWire` by hand and registered
with `SplitWire.exe install` is updated the same way. The data folder ([files](../files.md)) stays as it is.

## Uninstall

The window is asked to end, then the uninstaller asks whether to delete the settings, profiles and logs too (No
by default) and runs `SplitWire.exe uninstall`, with `--purge` for Yes: the service stops, which disconnects every
profile with its NRPT rules, routes and adapter, and is deleted; the driver goes when no other adapter uses it.
`SplitWire.exe cleanup` then removes any NRPT rule a crashed service left. The Run value goes when it starts this
copy, in the hive of the user who uninstalls.

Without code signing, SmartScreen warns about Setup.
