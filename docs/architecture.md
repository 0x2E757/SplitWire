# Architecture

One package, `splitwire`: a library with the logic and two thin executables. All logic is small structs composed
through constructors (SOLID and KISS, see [AGENTS.md](../AGENTS.md)). Traits exist only at the boundaries of the
OS, the network, the clock and the transport, so every component is unit-tested with fakes.

| Layer | Holds | Calls Windows |
|---|---|---|
| domain: `list`, `net`, `routes`, `dns`, `alerts`, `logs`, `stats`, `profile`, `config`, `tunnel`, `clock`, `cli` | the rules, pure and tested | never |
| `application` | the engine composed from a platform set: the coordinator, the Connect steps, the proxy and the periodic jobs; the data folder, the profiles, settings and lists on disk | through the boundary traits and the file helpers of `platform` |
| `ipc` | the pipe's messages and their binary codec ([IPC](ipc.md)) | never |
| `platform` | thin wrappers over the hand-written ABI, without policy | yes; unsafe code stays here |
| `presentation` | the window's and the tray's view models, their session and the engine transport ([window](window.md), [tray](tray.md)) | never |

| Page | About |
|---|---|
| [Domain modules](architecture/domain.md) | what each domain module in place holds |
| [Application and platform modules](architecture/platform.md) | the composition side and the Windows wrappers in place |
| [Boundary traits](architecture/boundaries.md) | the traits the logic is tested through, and why each exists |

Concrete platform types are named in one place only, the platform set the engine is built from.
