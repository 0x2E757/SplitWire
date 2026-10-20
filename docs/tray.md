# Tray

`SplitWire.Window.exe` shows a notification icon beside its window, in one process with one session and one
pipe client. The view models are `presentation::tray`; `platform::window::tray` shows them.

## Icon

The icon is the `TrayState` of the last `Status` ([alerts](alerts.md)); while no pipe answers it is the error's.
Only its colour tells the state. `TrayArt` draws it per taskbar theme ([icons](building.md#icons)), embedded as icon
groups 101–104 (light taskbar) and 111–114 (dark). The taskbar follows the Windows mode (`SystemUsesLightTheme`),
not the app theme.

The tooltip is "SplitWire · N of M connected", a line per failed, blocked or connecting profile, and
"N alerts", within the 127 characters of `szTip`; profiles that do not fit become "+N more".

## Menu

A left click opens or raises the window. A right click asks for the status (waiting at most
300 ms) and opens a popup menu: a header with the state's icon and "N of M connected", one item per profile,
then "Alerts · N" (each alert opens where it is fixed; Dismiss all) or "No alerts", Open SplitWire (bold) and
Exit. Without the service: "Service not running", Open SplitWire, Exit.

A profile's item is its state dot, its name and its state on the right; one click switches it: Connect when off,
Disconnect when on. No submenu. Short words (On, Off) leave room for the name.

| Profile | Its item |
|---|---|
| connected, connecting | "On", "Connecting": a click disconnects |
| off, failed | "Off", "Failed": a click connects (tries again) |
| disconnecting | disabled |
| blocked | disabled, "Can’t connect · profile.conf is invalid" |
| any, `splitwire.conf` invalid | disabled, "Off · splitwire.conf is invalid" |

Items carry the window's state dots as 32-bit bitmaps. Menus are dark or light with the app theme, through
uxtheme's `SetPreferredAppMode` and `FlushMenuThemes` (ordinals 135 and 136).

## Process

- One instance per session: the mutex `Local\SplitWire.Window`. A second start sets `Local\SplitWire.Window.Show`
  and ends; the first shows its window. A second `--tray` start just ends. `…Window.Exit` asks it to end (Setup).
  A demo uses `SplitWire.Window.Demo`, another pipe `SplitWire.Window.<pipe>`.
- `--tray` shows the icon only, and the window when `open_window_at_sign_in = true`. Closing the window hides it;
  Exit ends the process, and the service keeps running.
- "Start SplitWire at sign-in" writes or deletes the HKCU Run value `SplitWire` = `"<exe>" --tray`. A demo
  writes nothing.

## Trying it

```powershell
SplitWire.Window.exe --demo normal            # window and tray; --tray for the tray only
SplitWire.exe run --dry-run --for 30m --dir C:\Temp\sw --listen 127.0.0.1:15353
SplitWire.Window.exe --pipe SplitWire.DryRun  # in a second console, against the dry run
```

`--verify-pages` loads all 32 icons and the app icon's 8 sizes, and builds every scenario's menu without showing
them: one plain item per profile, as many top-level items as the view has.
