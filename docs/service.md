# Service and pipe

`SplitWire.exe service` is the Windows service; `run` is the same engine in a console. Both run through
`application::EngineHost`, which also serves the pipe the window, the tray and the command line use ([IPC](ipc.md)).

## Host

1. `Global\SplitWire.Engine` is taken (a live run), then the pipe's first instance: a second engine stops here.
2. The engine starts ([engine](engine.md)); warnings and errors also go to `logs\splitwire.log` ([files](files.md)).
3. The pipe is served until Ctrl+C, the run's own `--for` time limit or the SCM stops the run.
4. The pipe closes first, so no request reaches a stopping engine; then the engine stops.

A failed start is written to `logs\splitwire.error.log`.

## Pipe

`\\.\pipe\SplitWire` in byte mode, created with `FILE_FLAG_FIRST_PIPE_INSTANCE` and `PIPE_REJECT_REMOTE_CLIENTS`,
with the DACL `D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x12008b;;;IU)`: interactive users read and write but
cannot create an instance, so none can serve it. A dry run serves `\\.\pipe\SplitWire.DryRun`; `--pipe` picks
another name.

`PipeServer` accepts on its own thread and gives each client a thread, at most 8. A client may send many requests
over one connection. Request frames are zeroed once answered, because `PreviewImport` and `Import` carry a `.conf`.
Stopping cancels the clients' pending reads.

`PipeClient` keeps its connection and opens a new one when a send fails (the service restarted); a request that was
sent is never sent twice. Before anything goes to `\\.\pipe\SplitWire`, it checks that the pipe's owner is SYSTEM
or the Administrators, so a user's process squatting the name never receives a `.conf`.

## Control

`application::Control` decodes a request, answers it from the coordinator or a report, and encodes the answer;
anything undecodable gets `Error{bad_request}`. `StatusReporter`, `ProfileReport` and the list and settings views
never take the coordinator's lock, so `Status` keeps answering while a Connect runs.

## SCM

`platform::service::Dispatcher` hands the main thread to the SCM; `ControlHandler` reports the states and accepts
stop, shutdown (both end the run) and power events (a resume runs the periodic jobs at once). `Installer` creates
the service: LocalSystem, automatic start, an unrestricted service SID, restarts after 5 s, 5 s and 30 s.
`uninstall` stops and deletes it and removes the driver when no adapter is left; the data folder stays for a later
install, `--purge` deletes it. Install, uninstall and cleanup are run by a person, never by an agent
([safety](safety.md)).
