# Configuration

The service's settings are strict `key=value` files: `#` starts a comment, blank lines are allowed, keys and values
are trimmed. A missing key takes its default; an empty list value is the empty list. A line without `=`, a repeated
key, an unknown key and a bad value are problems `{file, line, key, value, reason}`; a problem may carry a fix
("Use 1420"), which the window fills in and Apply writes.

## `splitwire.conf`

An invalid file keeps every profile off.

| Key | Default | Valid | Applies |
|---|---|---|---|
| `listen` | `127.0.0.53:53` | a loopback IPv4 address; port 53 unless a dry run | next start |
| `fallback` | empty: the DNS of the default-route interface | 0–4 IPv4 addresses | live |
| `bypass` | empty | comma-separated names with at least 2 labels | live |
| `max_ttl` | `300` | 30–86400 s | live |
| `idle_expiry` | `86400` | 600–604800 s | live |

## `profiles\<name>\profile.conf`

An invalid file blocks only its profile.

| Key | Default | Valid | Applies |
|---|---|---|---|
| `connect_at_boot` | `false` | `true` or `false` | next start |
| `upstream` | empty: the profile's `DNS=` | 1–4 IPv4 addresses, each with an optional `:port` | reconnects if connected |
| `mtu` | empty: the profile's MTU, else 1420 | 1280–1500 | reconnects if connected |

## Saving

`SettingsMerge` puts each change into its file's text: a key's first line gets the new value, keeping its spacing
and comment; later repeats of the key and an unset key's line are deleted; a new key is appended; the line endings
stay. `SettingsFiles::save` validates every affected file and writes all of them or none, each atomically. A save
carries the revision it was made on; when the files changed since, it is refused as stale. The engine then applies
the live keys at once and reconnects the connected profiles whose `upstream` or `mtu` changed. Hand edits are
picked up within a second the same way; an invalid `splitwire.conf` keeps the last good values in force.

## Service-owned files

Not settings: the service writes them and reads them leniently.

| File | Holds |
|---|---|
| `splitwire.state` | `connected=<name>,<name>`: the profiles to restore at start |
| `profiles\index.conf` | one `<name> = <pin 1–6>` line per profile in the user's order; a folder without a line is appended with the first unused pin, a line without a folder is dropped |
