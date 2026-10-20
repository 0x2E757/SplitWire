# Design notes

Part of [Window](../window.md). On purpose, Routes is view-only, fixes wait for Apply, About shows the data
folder as text and the combined text follows the profiles' order. The rest follows from building the window in
system XAML without data binding:

- **Tables** are rows of Grids in StackPanels, as TaskbarMetrics' tables, not ListViews over a grouped
  CollectionViewSource: binding needs runtime objects of our own. Grouping, chevrons and per-line remove buttons
  are the same. The live log shows its newest 200 lines; Copy takes every line that matches.
- **Dialogs** (Remove…, Edit as text) are drawn over the window with a scrim, the look of a ContentDialog, which
  in a XAML island needs a `XamlRoot` and an asynchronous call.
- **Menus**: the profile menu is a MenuFlyout whose items have templates of their own: the kit's 32 px rows
  with the window's stroked icons, the status line on top, thin separators. The flyout closes on a click,
  Escape or a click outside.
- **The combined text** is a monospace TextBox with a gutter that marks section headers and refused lines; the
  highlight is the gutter's colour, not an overlay.
- **Log pausing** is the Follow switch; scrolling down does not pause by itself.
- **Drop zone**: files dropped on the window outside the zone are taken too (`WM_DROPFILES`).
- **The tray menu** is a Win32 popup menu, not a MenuFlyout in a popup window: its items carry the state dots,
  a severity dot per alert and the tray icon in the header as bitmaps, not the kit's other icons.
- **The window is created hidden** at start and hidden again on close, so the tray and the window share one
  session and the window opens at once.

## Status cards

A connected profile's card has its header, then one row of Handshake, Received, Sent
and Routes (DNS), each a label over a value and a caption, in equal columns across the card in both width classes,
then the chart at the card's full width. The other profiles keep their one-line card.
