# Contributing

- Code follows SOLID and KISS: logic is built by composing small structs ([AGENTS.md](AGENTS.md)).
- Everything is in English.
- Before committing, run the [checks](docs/building/checks.md).

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <description>`.

- `type`: what kind of change it is, from the first table.
- `scope`: which part of the program, from the second table. Leave it out, as in
  `<type>: <description>`, when the change has no single area.
- `description`: at most 70 characters, imperative mood ("add", not "added"), lowercase
  unless it starts with a name (`NRPT`, `README`), no period at the end.
- `!` before the colon marks a change that breaks existing files, settings or arguments.

An optional body follows a blank line, says why rather than what and wraps at 72;
trailers such as `Co-Authored-By:` go last. One commit, one type: split unrelated changes.

| Type | Change |
|---|---|
| `feat` | New or changed behavior or look the user can see |
| `fix` | Wrong behavior or look corrected |
| `perf` | Less CPU, memory or I/O, same behavior |
| `refactor` | Code restructured, same behavior |
| `test` | Tests only |
| `docs` | Documentation only |
| `build` | `Cargo.toml`, `build/`, manifests, icons, packaging, `vendor/` |
| `chore` | Other tooling and upkeep: `tools/` checks, repository files |
| `revert` | Reverts an earlier commit |

| Scope | Area |
|---|---|
| `tunnel` | `src/tunnel/`, wireguard-nt, `vendor/wireguard-nt/` |
| `dns` | `src/dns/`, the proxy, upstream exchanges |
| `routes` | `src/routes/`, `src/net/`, IP Helper |
| `nrpt` | NRPT rules in the registry, the Group Policy probe |
| `lists` | `src/list/`: list files, matching, the combined text |
| `profiles` | `src/profile/`: import, keys, storage |
| `alerts` | Alerts and the tray state |
| `service` | The engine, the SCM entry, `src/application/` |
| `cli` | `SplitWire.exe` commands and exit codes |
| `ipc` | The pipe and its protocol |
| `window` | `SplitWire.Window.exe`'s window and view models |
| `tray` | The tray icon and menu |
| `config` | Settings files and the data folder |
| `installer` | Setup, `tools/release.ps1` |

Other files take the scope of the feature the change serves. Documentation uses the scope
of the part it describes. `src/platform/abi.rs` and the logs take the scope of the change
that needs them.

```
feat(routes): refuse a network covering another profile's tunnel
fix(dns): cap cached TTLs at max_ttl
build: vendor wireguard-nt 1.1
```
