**These rules always apply to any agent action:**

- All code is written strictly following SOLID & KISS. All application logic must be built through composition of SOLID classes and their methods.
- Every commit subject is `<type>(<scope>): <description>`: a type and a scope from [CONTRIBUTING.md](CONTRIBUTING.md), a description of at most 70 characters. Read that page before committing.
- No crates: the standard library plus the Windows API through the hand-written ABI in `src/platform/` (`abi.rs`, the TaskbarMetrics approach). Unsafe ABI calls stay in `src/platform/`.
- Traits only at real boundaries (OS, network, clock, transport), so the logic is unit-tested with fakes. Small files, one concept each; no logic in `main`.
- Everything is in English: code, comments, docs and messages. Comments are sparse and say why.
- Nothing accessibility-related is implemented.
- Every change keeps the [checks](docs/building/checks.md) green and updates the docs it affects.

## Safety rules for agents

Agents work on a developer's machine that may be reached remotely; a wrong route, DNS rule or adapter can cut
it off.

- Never elevate: no RunAs and no administrator console, even where UAC would not prompt.
- Never change the machine's routes, DNS or NRPT settings, firewall, services, network adapters or drivers.
- Never create, open or configure a WireGuard adapter. `wireguard.dll` may only be loaded from a test folder, with
  no function called.
- Never run a live engine or the service, and never install: use fakes, `run --dry-run`, high loopback ports, scratch
  folders and the `HKCU\Software\SplitWire-test` key.
- Never read real WireGuard `.conf` files or keys; fixtures use made-up keys.
- Live tests are run by a person, never by an agent: see [docs/safety.md](docs/safety.md).
