# Integration tests

Part of [Checks](checks.md). Each runs a dry-run engine in the test process, on its own loopback port and pipe,
with made-up profiles and keys. Nothing on the machine changes and nothing is shown.

- `tests/engine_dry_run.rs` runs the engine on `PlatformSet::dry_run()` with the proxy on `127.0.0.1:15353`, two
  made-up profiles and a fake upstream on loopback: names route on the right adapter, a shared address is held,
  a disconnect lets names fall through, a failing step rolls back, nothing is left.
- `tests/pipe_dry_run.rs` runs a dry-run host serving `SplitWire.Test.<pid>` on `127.0.0.1:15354` and drives it
  over the pipe like the window and the CLI: import, settings, lists, connect, reports, alerts, remove. No answer
  holds a key. The pipe's own tests (`SplitWire.Test.<pid>.<n>`) check the first instance, the DACL and a bad frame.
  The ACL test changes a folder's DACL, so it runs only when `SPLITWIRE_ACL_SCRATCH` names a scratch folder; it
  works in a subfolder it deletes afterwards and keeps the current owner's full control.
- `tests/window_dry_run.rs` runs a dry-run host serving `SplitWire.Test.<pid>.Window` on `127.0.0.1:15355`, imports
  two made-up profiles, connects one and blocks the other by a bad line in its `profile.conf`, then drives the
  window's `Session` over a real `PipeClient`, as `SplitWire.Window.exe --pipe` does: the navigation, the Status
  page, the bell, both profile pages and the tray show what the engine says, and a Disconnect reaches it.
