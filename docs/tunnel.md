# Tunnel

How a profile's wireguard-nt adapter is loaded, built, watched and removed. The rules it keeps are in
[Safety](safety.md).

## Loading wireguard-nt

`platform::AppLibrary` loads a plain file name from the executable's folder only, read with writes denied, its
SHA-256 (BCrypt) compared with `Api::DLL_SHA256` (a test keeps it equal to the
[vendor README](../vendor/wireguard-nt/README.md)), then `LoadLibraryExW` by full path with
`LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32`. `wireguard::Api` resolves all twelve exports.
`WireGuardNt` (the real `WireGuardDriver`) loads it at the first use and keeps it; `DriverLog` sends the driver's
lines to the log as `wireguard` at their level; the "not found" of step 2, which every Connect expects, is info.
`Status` reports the running driver's version once it is loaded (`Tunnel::driver_version`); asking never loads it.
Every process start restricts the default DLL search to System32.

## Up

`application::WireGuardTunnel::up` for `SplitWire-<name>`:

1. record the endpoint address as a never-route;
2. if `WireGuardOpenAdapter` finds the name, another process holds it: fail;
3. create the adapter (tunnel type `SplitWire`, the profile's GUID), logging on;
4. `AdapterAddressing::apply`: the /32, the MTU, metric 9000 with automatic metric off, no DNS servers or
   registration;
5. the configuration (`ConfigBuffer`): private key, `REPLACE_PEERS`, one peer with its keys, endpoint and
   keepalive, allowed IP `0.0.0.0/0` for cryptokey routing only;
6. state up; the link is the LUID and the interface index.

A failing step drops the new adapter, which removes it. `down` sets it down and closes the handle.

## Refresh

`refresh` (TunnelWatch, every 10 s):

| Finding | Action |
|---|---|
| state unreadable, LUID changed or no index | close, up again; a new link |
| index changed | the link carries it (the bound DNS socket is rebuilt) |
| down, or the /32 or MTU missing | apply again, state up |
| `Handshaking` or `Stale` | re-resolve the endpoint, at most once a minute |

`HandshakePolicy`: `Handshaking` with no handshake 30 s after up; `Stale` when the last one is over 180 s old and
the tunnel keeps sending: its sent bytes grew at this read and the one before, so a keepalive after a handshake or a
handshake under way does not count. `EndpointResolver` asks the fallback servers only and records every address in
`EndpointAddresses` first. A moved endpoint is set with `UPDATE_ONLY`; one that does not resolve keeps the peer.

## IP Helper and upstream DNS

`IpHelperAddressing` and `IpHelperRouteTable` (`platform::ip_helper`). The route table lists and deletes only
SplitWire's routes (static, metric 7, on the adapter) and refuses any wider than /8.

`BoundUdpExchange` binds `<tunnel address>:0` and sets `IP_UNICAST_IF` to the adapter's index, so a profile's
queries leave only through its adapter; one socket per query. `UdpDnsExchange` is the unbound one for the fallback.
