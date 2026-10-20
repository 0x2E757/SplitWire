# Command line

Part of [Building and running](../building.md). `SplitWire.exe <command>`; `help` prints the usage.

| Command | Who | Does |
|---|---|---|
| `run [--dry-run] [--for <d>] [--dir <p>] [--listen <a>] [--pipe <n>] [--connect <name>]...` | admin (dry run: anyone) | the engine in the foreground, serving the pipe |
| `list` | pipe | the profiles in order, with state and handshake |
| `status [<name>]` | pipe | the engine's status, or one profile with its routes and last attempt |
| `import <file>... [--name <n>] [--preview] [--replace]` | pipe | preview, then import with the proposed colour; the name defaults to the file name |
| `connect <name>`, `disconnect <name> \| --all`, `remove <name>` | pipe | a profile stays connected until it is disconnected |
| `lists [--save]` | pipe | the combined text; `--save` reads a new one from stdin |
| `alerts [--dismiss <id> \| --restore]` | pipe | list, dismiss or restore alerts |
| `install [--restore]`, `uninstall [--purge]`, `cleanup`, `service` | admin, SCM | [service](../service.md) |

The pipe commands talk to `\\.\pipe\SplitWire`; `--pipe SplitWire.DryRun` talks to a dry run.

Exit codes: `0` ok, `1` error, `2` usage, `3` engine not reachable, `4` administrator required, `5` refused (the
problem is printed with its lines), `6` unknown profile.

## Dry run

| Option | Means |
|---|---|
| `--dry-run` | adapters, routes, NRPT, the DNS cache flush and the folder ACL only log; no administrator needed |
| `--for` | stop the whole run after the time (`20s`, `5m`, `1h30m`): the engine's own time limit, a dead man's switch for test runs; it is not a connection's duration. Ctrl+C stops at any time |
| `--dir` | the data folder (default `%ProgramData%\SplitWire`) |
| `--listen` | the proxy's loopback address; a dry run takes any port, a live run only 53 |
| `--pipe` | the pipe it serves (default `SplitWire`, a dry run `SplitWire.DryRun`) |

A dry run is the engine on the logging fakes of `PlatformSet::dry_run`. Real and harmless: DPAPI, the read-only
interface queries, the Group Policy key, DNS and the pipe. A profile's upstream may be a fake DNS server on
loopback (`upstream=127.0.0.1:<port>` in its `profile.conf`). The log goes to the console:

```powershell
SplitWire.exe run --dry-run --for 30m --dir C:\Temp\sw --listen 127.0.0.1:15353
SplitWire.exe import C:\Temp\Made-up.conf --preview --pipe SplitWire.DryRun
```

A live run (no `--dry-run`) needs an elevated console and is run by a person, never by an agent ([safety](../safety.md)).
