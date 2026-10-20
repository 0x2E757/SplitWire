# SplitWire

A WireGuard client for Windows, built on wireguard-nt, that holds many profiles at once. Each profile is connected
or not on its own, and several run in parallel. A profile sends **only** its listed domains (each name and every
subdomain, resolved through that profile's DNS) and its listed IPv4 addresses and networks through its own tunnel.
Everything else (the LAN, the internet, RDP into the machine) stays on the normal connection.

- One wireguard-nt adapter per profile, `SplitWire-<name>`, with a /32 address and no DNS servers; SplitWire adds
  its own routes. No kill switch. IPv4 only.
- One DNS proxy on `127.0.0.53:53` for all profiles, reached through NRPT rules written in the registry.
- A Windows service (`SplitWire.exe`, also the command line) and a tray icon with a window
  (`SplitWire.Window.exe`).
- Rust, the standard library and the Windows API through a hand-written ABI: no crates. The official signed
  `wireguard.dll` is vendored and pinned by SHA-256.

## Status

Version 0.1.0, in daily use with one profile: import, connect and disconnect from the window
and the tray, routing by name through the profile's DNS, NRPT in the registry, the service and its recovery after a
crash. Not done yet: live tests of several profiles at once.

## Installing

Download `SplitWire-Setup-<version>.exe` from [Releases](https://github.com/0x2E757/SplitWire/releases), check it
against `SHA256SUMS.txt` and run it: it asks for administrator rights once, installs into `%ProgramFiles%\SplitWire`,
registers and starts the service and starts the tray. Run a newer one to update: the profiles that were connected
connect again, also over a copy installed by hand. Uninstall from Settings, Apps; the profiles and settings stay
unless you choose to delete them ([installer](docs/building/installer.md)).

## Building

You need Rust with the `x86_64-pc-windows-msvc` target, the MSVC linker and the Windows SDK.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build.ps1
```

The package lands in `target\package` ([building](docs/building.md)).

## Documentation

| Page | About |
|---|---|
| [Architecture](docs/architecture.md) | Layers, modules, boundaries |
| [Profiles](docs/profiles.md) | Names, importing a `.conf`, colours and order, keys |
| [Lists](docs/lists.md) | The list file format, its problems, all lists together |
| [Configuration](docs/configuration.md) | `splitwire.conf`, `profile.conf`, the service-owned files |
| [Files](docs/files.md) | The data folder, atomic writes, its ACL |
| [Tunnel](docs/tunnel.md) | Loading wireguard-nt, the adapter's lifecycle, IP Helper, upstream DNS |
| [Engine](docs/engine.md) | Connect and its rollback, Disconnect, start, restore and stop |
| [Routing and DNS](docs/routing-and-dns.md) | Which profile gets a name or an address, the proxy, re-homing |
| [Warm-up](docs/warm-up.md) | The first seconds after Connect or idle: the first handshake, DNS retries, the cache flush |
| [NRPT](docs/nrpt.md) | The registry rules, their sync, Group Policy |
| [Alerts](docs/alerts.md) | Alert kinds, dismiss and restore, the tray state |
| [IPC](docs/ipc.md) | The pipe's frames, their binary codec and the messages |
| [Service and pipe](docs/service.md) | The host, the pipe and its DACL, the dispatcher, the SCM |
| [Window](docs/window.md) | The window's pages, its structure, the demo and the page check |
| [Tray](docs/tray.md) | The tray icon, its tooltip and menu, one instance, the sign-in start |
| [Safety](docs/safety.md) | The invariants the code holds, and who runs live tests |
| [Building and running](docs/building.md) | Package, checks, the command line and the dry run |
| [Contributing](CONTRIBUTING.md) | Code rules, commit types and scopes |

## License

MIT, see [LICENSE](LICENSE). The vendored `wireguard.dll` is WireGuard LLC's, under its own licence; see
[vendor/wireguard-nt/README.md](vendor/wireguard-nt/README.md).
