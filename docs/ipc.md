# IPC

The window, the tray and the command line talk to the service over the pipe `\\.\pipe\SplitWire`. One request gets
one response. `src/ipc/` holds the protocol: the frames, their codec and every message. The pipe itself, its DACL
and the dispatcher are in [service and pipe](service.md).

## Frames

`u32` little-endian length of what follows (at most 16 MiB), `SW1`, a tag byte, then the fields:

| Field | Encoding |
|---|---|
| integer | unsigned LEB128, at most 10 bytes |
| string | length, then UTF-8 |
| list | count, then the items |
| option | `0`, or `1` then the value |
| enum | the variant's index (a variant with fields: then its fields) |
| time | Unix milliseconds |
| IPv4 address | its 4 bytes; a network adds its prefix, a server its port |

`Frame::read` checks the length before allocating, and `Reader` checks every length and count against the bytes
left, so garbage is an error, never a panic or a large allocation. Bytes after a message are an error too. Request
tags are 1–24 (19, once `CancelTimer`, stays unused), response tags 128–142, so neither is read as the other. Any decoding error is answered with
`Error{bad_request}`.

## Messages

`Request` and `Response` list every message of the protocol (`Status`, `ProfileStatus`, `Routes`, `Traffic`,
`Logs`, the alert, list, settings, connect and import requests). Any request may get `Error{code, message}`, with
`bad_request`, `unknown_profile`, `refused{problems}` (list problems with both line numbers, settings problems
with theirs, or text) and `stale_rev`.

The messages reuse the domain types where they exist: `ImportPreview` with its checks, changes and per-line
outcomes, `SettingProblem`, `SettingChange`, `KeyValueEntry`, `TextProblem`, `QueryCounters`,
`UpstreamHealthState`, `HandshakeHealth`, `DriverVersion`, `Pin`, `TunnelAddress`, `Endpoint`, `Ipv4Net` and the
log `Level`. What the engine reports (states, attempts and their steps, alerts, routes, traffic, list lines) has
small types of its own in `src/ipc/`.

## Keys

No response can carry key material: profiles appear as their facts and public keys only, and a test scans every
encoded response, previews of `.conf` files full of keys included, for the keys' base64 and raw bytes. The `.conf`
of `PreviewImport` and `Import` travels from the client to the service only, in a `ConfText` that is zeroed on
drop and redacted in `Debug`; its frame is built without regrowing, and both ends zero it after use.
