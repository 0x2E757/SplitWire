# Warm-up

The first seconds after a Connect: the tunnel's first handshake, and patience with its upstream DNS. The steps are
in [engine](engine.md), the upstream in [routing and DNS](routing-and-dns.md).

## The first handshake

Step 7 (`dns`), once the resolver exists, reads the driver's last-handshake time (`Tunnel::health`) every 250 ms
for up to 10 s (`tunnel::HandshakeWait`). The NRPT rules come at step 9, so they exist only once the tunnel can
carry the names; until then listed names resolve as before.

A peer with no `PersistentKeepalive` handshakes only when traffic comes. When the driver reports no handshake yet,
Connect sends it one probe (`ProfileResolver::probe`): an SOA query for the profile's first listed domain, or `.`
without one, to the upstream through the bound exchange, asked once. Its answer counts as the upstream's first;
its failure is not recorded. The probe needs step 6's /32s to the upstream: the bound socket has no other route.
Its time counts toward the 10 s. The step's detail ends with how it went:

| Detail | When |
|---|---|
| `first handshake after 1.3 s` | the driver reported one |
| `probe sent; first handshake after 0.8 s` | the same, after a probe |
| `[probe sent; ]no handshake within 10 s; connecting anyway` | none came: the Connect still succeeds and raises `handshake_stale` ("No handshake yet") until `TunnelWatch` sees a handshake |

## Upstream patience

`dns::UpstreamWarmup`, one per resolver, lasts until the tunnel's first handshake and 15 s after it (after the
Connect, when the handshake came during its wait). Step 7, or `TunnelWatch` every 10 s, passes the driver's
handshake time on.

While it lasts, a failed exchange of a listed name is retried every 0.5 s as long as the next attempt can end
within 5 s of the query; one server with its 2 s timeout gets two attempts in 4.5 s. Only then does the client get
SERVFAIL, and only such a spent budget counts as a failure toward `upstream_unreachable` (3 in a row, as always).
After the warm-up a failure is SERVFAIL at once. A query being retried holds one of the proxy's four UDP workers.

## The cache flush

A profile's first upstream answer since its Connect, or since a failure, flushes the Windows DNS client cache once
(`DnsService`), so the failures it remembers go away. An answered probe was that first answer, before NRPT: no flush.
