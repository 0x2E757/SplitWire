# Profiles

A profile is one WireGuard `.conf`, imported under a name. It lives in `profiles\<name>\` ([files](files.md)).

## Names

`[A-Za-z0-9._-]`, 1–32 characters, not ending in a dot, not a Windows device name (`CON`, `PRN`, `AUX`, `NUL`,
`COM0`–`COM9`, `LPT0`–`LPT9`, with or without an extension), unique ignoring case. A name is validated before any
path or adapter name (`SplitWire-<name>`) is built from it. At most 16 profiles; renaming is remove and import.

## Import

What is read from a `.conf`, what is ignored, and what the import review checks: [import](profiles/import.md).

## Colours and order

Six pin colours, 1–6, repeating beyond six profiles. Import proposes the first unused one (when all are used, the
least used). `index.conf` holds the user's order; every list of profiles follows it.

## Keys

The blob is `CryptProtectData` with `CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN` and the entropy
`SplitWire profile v1`, over the plaintext `version=1`, `guid`, `private_key`, `address`, `mtu`, `dns`,
`peer_public_key`, `preshared_key`, `endpoint`, `keepalive`, `imported` (Unix seconds). Keys are decrypted to read
the public facts at load, for a replace's diff and to connect, and zeroed on drop. No error, preview or file but
the blob holds them; a test scans previews and files for them.
