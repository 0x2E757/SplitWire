# Importing a profile

Part of [Profiles](../profiles.md).

## What is read from a `.conf`

Keys are case-insensitive; `#` starts a comment. The import review lists every line with its outcome. A shown line
never holds a key, a script or an unknown value: `PostUp = …`, `DNS = …, corp.example.com`.

| Outcome | Lines |
|---|---|
| read | `PrivateKey`, `Address` (one IPv4, its prefix gives the tunnel subnet, /8 or longer), `DNS` (up to 4 IPv4), `MTU` (1280–1500), `PublicKey`, `PresharedKey`, `Endpoint` (IPv4 or name, with port), `PersistentKeepalive` |
| ignored, with the reason | `AllowedIPs`, `ListenPort`, `Table`, `PreUp`/`PostUp`/`PreDown`/`PostDown`, `SaveConfig`, `FwMark`, IPv6 addresses and DNS servers, search domains in `DNS`, unknown keys |
| problem (blocks the import) | a bad key (base64, 32 bytes), a missing `PrivateKey`, `Address`, `PublicKey` or `Endpoint`, a second `[Peer]` or `[Interface]`, a repeated key, an IPv6 endpoint, a bad value |

A line with both kinds of items (`Address = 10.0.0.2/32, fd00::2/64`) is reported twice, read and ignored.

## Preview and import

`ImportPreview` stores nothing. It gives the name problem (the rules, a clash ignoring case, the limit), the
values of a new profile or the changes against a stored profile of the same name (keys only as `changed: yes` or
`no`), the line outcomes, the proposed colour, and checks against the other profiles:

| Check | Level |
|---|---|
| the tunnel subnet overlaps another profile's | bad: blocks |
| the endpoint is inside another profile's listed network or recent answers | warn |
| the endpoint name is under another profile's listed domain | info |
| a DNS server another profile uses too | info |
| the replaced profile is connected | warn: "Will reconnect" |

`ProfileSet::import` repeats the validation. A new profile gets an empty `routes.txt`, `profile.conf` with
`connect_at_boot=false`, its blob and a line at the end of `index.conf`; a failure leaves no folder. A replace swaps
the blob only and keeps the list, settings, colour, position and adapter GUID.
