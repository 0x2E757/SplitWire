# Window

`SplitWire.Window.exe` shows the service's state and changes it. It is built the way TaskbarMetrics'
window is: system XAML in a XAML island through the hand-written WinRT ABI, no third-party libraries, no WebView.
The same process shows the [tray](tray.md).

| Page | About |
|---|---|
| [Structure](window/structure.md) | The layers, the frame loop, regions and how markup becomes elements |
| [Fields and focus](window/fields-and-focus.md) | Typing reloads no region around a field; live parts; focus after a reload |
| [Demo and checks](window/demo-and-checks.md) | `--demo`, `--verify-pages`, the interface checks of `verify-abi` |
| [Alignment](window/alignment.md) | Rows on one line, titles on one baseline, the alignment check, `--render-pages` |
| [Design notes](window/design-notes.md) | Choices that follow from system XAML without data binding |

## Pages

Status, one page per profile (with its traffic chart), Routes (view-only), Lists (rows or the combined text),
Logs (newest on top), Settings (with the apply bar), Add profile (the first run's Welcome), the import review,
the alerts bell on every header, the profile menu on a right click of a profile's navigation item and on "…": the state, Connect or Disconnect, Replace .conf…, Open log
and Remove…. A Routes card opens and closes by a click anywhere on its header but "…". Without the service,
Status says so and every other page is closed.

The traffic charts follow TaskbarMetrics' axes: labels 8 px before the plot, only the outermost with the unit,
1-2-5 scales in KB/s, MB/s or GB/s; a wide label runs into the card's padding.

## Frame

The system title bar `SplitWire vX.Y.Z` in the app's colours, the navigation pane (248 px from 1100 DIP, 56 px
icons with tooltips and ☰ below), the content layer with the page and the apply bar pinned under it. The window
opens centred at 1280 × 1024 or where it was closed, never smaller than 780 × 960 or the work area. The theme
follows the Windows app theme unless Settings chose Light or Dark. Closing the window hides it while the tray runs.

## Arguments

```text
SplitWire.Window.exe [--tray] [--pipe <name>]
SplitWire.Window.exe --demo normal|firstrun|noservice|connectfail|settingsinvalid [--theme light|dark]
SplitWire.Window.exe --verify-pages
SplitWire.Window.exe --render-pages <folder>
```

Without `--demo` the window talks to `\\.\pipe\SplitWire` through `PipeClient`; `--pipe SplitWire.DryRun` talks
to a dry run. `--tray` is the sign-in start: the tray icon, and the window only when chosen ([tray](tray.md)). At
start the process restricts its DLL search to System32.

## Window settings

`%LOCALAPPDATA%\SplitWire\splitwire.window`: `open_window_at_sign_in`, `theme` (`system`, `light`, `dark`) and
`placement = x,y,w,h,maximized` in DIP, written when the window closes. A demo writes nothing.
