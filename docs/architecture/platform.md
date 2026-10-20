# Application and platform modules

Part of [Architecture](../architecture.md).

## Application

| Component | Responsibility |
|---|---|
| `Engine`, `EngineHost`, `StopSignal` | the composition root; start cleanup and restore; a run serving the pipe until Ctrl+C, `--for` or the SCM ([engine](../engine.md), [service](../service.md)) |
| `Control`, `StatusReporter`, `ProfileReport`, `ListsViewBuilder`, `SettingsViewBuilder`, `RouteGroups`, `WireAlerts`, `WireAttempts` | the pipe's dispatcher and its answers, read without the coordinator's lock |
| `Coordinator`, `ConnectRun`, `Teardown` | the one lock; the ten Connect steps with their undo; Disconnect, best effort |
| `EngineParts`, `Inventory`, `ActiveProfiles`, `ProfileStates` | what the engine acts on; the files as read; the connected profiles; every profile's state |
| `GuardSource`, `DnsService`, `DnsProxy`, `UdpServer`, `TcpServer` | never-routes and fallback; an answer routed before it leaves; the proxy on std threads ([routing and DNS](../routing-and-dns.md)) |
| `Scheduler`, `TunnelWatch`, `Maintenance`, `FileWatcher`, `TrafficSampler` | the periodic jobs on one thread |
| `AlertService`, `BootRestore`, `SettingsReload`, `ListService`, `ListsReload`, `ProfileOps` | alerts; what start restores; settings applied live or by reconnect; list edits with `base_rev` and their pickup; import, remove, preview |
| `DataFolder`, `FolderAccess`, `ProfileSet`, `SettingsFiles` | paths and atomic writes; the folder's ACL; profiles and settings on disk ([files](../files.md)) |
| `WireGuardTunnel` | the `Tunnel` kind over `WireGuardDriver` ([tunnel](../tunnel.md)) |

## Platform

Thin wrappers over the hand-written ABI, without policy. Unsafe code stays here.

| Component | Responsibility |
|---|---|
| `composition::PlatformSet` | the only place naming the concrete types: `live()` and `dry_run()` |
| `dry_run` | logging fakes: driver, addressing, routes, NRPT, flush, loopback upstream, folder ACL |
| `abi`, `registry`, `console`, `engine_mutex`, `elevation`, `local_clock` | declarations, handles, strings; registry keys; Ctrl+C; one live engine; admin or not; the time zone |
| `pipe`, `service`, `security_descriptor` | the pipe's server, client and DACL; the SCM entry, control handler and installer; SDDL |
| `atomic_file`, `dpapi`, `folder_security` | atomic writes; machine-scope DPAPI; the data folder's ACL |
| `library`, `sha256`, `wireguard` | `wireguard.dll` from the exe folder with its hash checked; the driver |
| `ip_helper`, `inet` | the adapter's address and MTU, SplitWire's routes, the read-only `IpHelperProbe` |
| `dns_exchange`, `dns_flush`, `nrpt` | UDP queries pinned to an adapter; the Windows DNS cache flush; NRPT rules and the Group Policy probe |
| `executables` | the package names of the executables |
| `com`, `window` | COM, HSTRING, the STA; the XAML island, the pages' markup, the renderer, the frame loop ([window](../window.md)), the [tray](../tray.md) |
