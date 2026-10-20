# wireguard-nt

The files below are the unmodified official WireGuardNT 1.1 files, amd64 build, as published by WireGuard LLC.
Copyright (C) 2018-2026 WireGuard LLC. The DLL is licensed under the Prebuilt Binaries License in
`LICENSE.txt`, which allows distributing it only unmodified, under its own name, alongside software that uses
it only through the API of `wireguard.h`.

Version: 1.1 (the DLL's file and product version; built 2026-05-07)
Archive: https://download.wireguard.com/wireguard-nt/wireguard-nt-1.1.zip, downloaded 2026-10-19,
SHA-256 DCEB30A9BC4BE48CCE0F74160FC88A585A2C2627366E8F846FC6658F9038DACE
Taken from the archive: `bin/amd64/wireguard.dll`, `include/wireguard.h`, `LICENSE.txt`
Authenticode signer: WireGuard LLC (EV certificate by Sectigo Public Code Signing CA EV R36,
thumbprint 1B3AFA5E2A76BB51F00020002DCCADB165689C33)

| File | Purpose | SHA-256 |
|---|---|---|
| wireguard.dll | The driver API library; it carries the signed driver it installs | B1B85E072C45D81358BE29D94C599DC76652F912BE8C0F0A41E2D5D89A6461D3 |
| wireguard.h | The API: function types and the `WIREGUARD_*` structures (GPL-2.0 OR MIT, per its SPDX line) | C770AB9FF979C55D63693099592CD24FDEA4272DE57C9DFB669DAF19725B2E60 |
| LICENSE.txt | The Prebuilt Binaries License | 899D95154986F755D02FEA5DA6781C6054C83F8B1D32BF28EADEB9FBCE0E9003 |

`tools/verify-vendor.ps1` checks every hash above and the DLL's signer. The service loads the DLL only from its
own folder, by full path, after checking its SHA-256 against the value in this table, which the build embeds.
`tools/build.ps1` packages the DLL under its own name and the licence as `wireguard-nt-LICENSE.txt`.
`.gitattributes` keeps these files byte for byte, so the hashes hold in every checkout.

To update: download the new archive from https://download.wireguard.com/wireguard-nt/, check the signer, copy the
same three files, and update the version, the dates and every hash here.
