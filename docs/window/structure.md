# Window structure

Part of [Window](../window.md).

| Layer | Holds | Calls Windows |
|---|---|---|
| `presentation::pages`, `nav`, `alerts`, … | one view model per page and component, built from IPC values | never |
| `presentation::Session` | the page, typed text, pending settings, the import; commands in, requests and effects out | never |
| `presentation::transport` | `EngineTransport` (`PipeClient` implements it); the demo service of the `--demo` scenarios | never |
| `presentation::tray` | the tray's icon state, tooltip and menu; `TrayArt` and `AppArt`, the icons `build/` embeds | never |
| `platform::window::markup` | XAML text from the view models, `$token$` colours, regions and their binds | never |
| `platform::window` | the island, the renderer, events, the frame loop, the picker, the clipboard | yes |

## A frame

Every 16 ms: the answers of the worker thread go to the session, handler events and changed fields become
`Command`s, the session's requests go to the worker, its effects run (clipboard, Explorer, the file picker,
reading the log file), and when the session's revision moved or a second passed the view is rebuilt and
rendered. The worker thread owns the transport: the frame never waits on the pipe, except a right click on the
tray icon, which waits up to 300 ms for a fresh status. The tray's clicks and menu items become commands of the
same session, and its icon and tooltip follow the session's `Status` ([tray](../tray.md)).

## Regions

A page is a list of regions, each a piece of markup for one named host. A region is reloaded only when its
markup changed; its elements get their handlers (`Click`, `Toggled`, menu items, Enter, the drop zone, dragging
a profile item), its text fields are read every frame. Field texts and commands are not part of the markup's
identity: a handler fires the latest frame's command (`BoundCommands`), so a kept Copy copies today's text.
What typing changes lives in live parts, so typing reloads no region around a field ([fields and
focus](fields-and-focus.md)).
A Connect switch carries a revision that moves when a Connect or Disconnect is refused, so a switch the user
flipped shows the service's state again. What changes on its own (a Status card's numbers and chart, uptimes,
the routes' ages, new log lines) is a child region of its own, so the fields, switches and open menus beside
it are not rebuilt.

The markup is pure text: palette tokens (`$card$`, `$pin3a$`) are filled per theme, icons are paths from the kit's
table, the charts are stretched Paths in star rows and columns, so nothing needs a pixel width. A traffic chart
draws Receive in TaskbarMetrics' chart blue `$total$`, Send in the profile's colour. Flyouts carry
their own styles, because the page's implicit styles do not reach them.

## Width classes and theme

The frame skeleton is rebuilt when the width class (1100 DIP) or the theme changes; the regions are reloaded
into it.
