# Demo and checks

Part of [Window](../window.md).

## Demo

`SplitWire.Window.exe --demo <scenario>` runs the window against the demo service instead of the pipe. It serves
its scenarios as IPC message values, every message encoded and decoded through the codec, on one fixed
day (19.10.2026 11:28:55); only the traffic moves on, a sample a second. It reacts like the
service: Connect takes a moment, a list edit is checked against every list, settings are merged and validated by
the `config` rules, an import is previewed by `ImportPreview` from the dropped or chosen `.conf`.

| Scenario | Shows |
|---|---|
| `normal` | two of three profiles connected, four alerts, HomeLab blocked by its `profile.conf` |
| `firstrun` | no profile: the Welcome drop zone |
| `noservice` | the service not running |
| `connectfail` | HomeLab's Connect failed at step 3, with the step table; Retry fails again |
| `settingsinvalid` | `splitwire.conf` line 4 invalid, every profile held off; "Use 300" and Apply fix it |

`--theme light|dark` overrides the theme for comparing screens. A demo saves no window settings.

## `--verify-pages`

Builds every page and state of every scenario (the `Showcase`: pages, dialogs, the review, a bad drop, the nav
overlay, paused logs, open expanders) in both themes and both width classes, loads each region through
`XamlReader` without a window, checks that every bound element and field exists, and mounts the regions with
their handlers. Every mounted field must read back the session's text, and a multi-line editor must keep its
lines after the markup and after resets. Then every field of every state is typed into key by key, as the frame
loop sees it, and must stay the same TextBox with its text and caret; a region reloaded under the focused field
must give the new TextBox its caret and selection ([fields and focus](fields-and-focus.md)). Laid out in a
never-shown island, every marked row must line up ([alignment](alignment.md)), as must every profile menu, laid
out as a card since no flyout opens without a window; a Routes card's toggle must cover its header; a region kept
as it was must raise the latest commands. It prints the totals and exits 1 with the region, the element, the misaligned marks
or XAML's own error text when one fails. The unit tests check the same markup for balanced elements, filled placeholders
and named binds. Then it loads every tray icon from the executable's resources and builds every scenario's tray
menu in both themes, without showing either.

## Interfaces in `verify-abi`

`src/platform/window/winrt.rs` declares every WinRT and COM interface the window calls. A GUID ends with
`// <header> <Interface>` (`class <Name>` for a coclass), a slot with `// <Interface>.<Method>`.
`tools/verify-abi.ps1` reads both, finds each interface in the SDK header (bases first: `IUnknown` 3,
`IInspectable` 6; parameterized interfaces by their C vtable) and fails on a different GUID or slot.
