# Safety

The code and the tests hold these invariants. The machine SplitWire runs on may be reached only through the network
it changes, so each rule protects that connection.

1. No route shorter than /8, so never the default route. `RouteGuard` enforces it, and `IpHelperRouteTable::add`
   checks it again.
2. Never a route over: loopback, link-local, multicast, broadcast, `0/8`; the fallback DNS servers; the on-link
   networks of other interfaces; any profile's endpoint; another profile's tunnel subnet.
3. A profile's routes go only on its own adapter's LUID, with metric 7 and protocol NETMGMT. No other route is
   modified or deleted. Never an IPv6 route.
4. A learned /32 belongs to one profile at a time; there is no preemption.
5. A profile's upstream DNS queries leave only through its adapter. The proxy binds loopback only. Endpoints are
   resolved only through the fallback DNS.
6. Adapters get no gateway, no DNS servers and no DNS registration. No kill switch, no firewall rules.
7. NRPT: only `SplitWire-*` keys with `Comment=SplitWire`, exactly the union of the connected profiles' domains, each
   with at least 2 labels, and only while the proxy serves. Never a child process.
8. Connect is per step with rollback and never changes another profile. Every start removes leftover NRPT rules
   first. One live engine; another process's adapter is never taken over.
9. Profile names are validated before any path or adapter name is built. Tunnel subnets never overlap.
10. Keys appear in no log, message, status, preview, error or file but the blob; key buffers are zeroed
    (`SecretKey`, `SecretBytes`). The data folder is admin-only; only `logs\` is readable by interactive users.
11. `wireguard.dll` loads only from the executable's folder, by full path, with its SHA-256 checked.
12. The pipe rejects remote clients, keeps its DACL and is created as the first instance; a client sends nothing
    to `\\.\pipe\SplitWire` unless SYSTEM or the Administrators own it. Request frames are zeroed after use.
13. A dry run never calls the driver, IP Helper writes, the HKLM NRPT, the DNS flush, ACL changes or the SCM: those
    are the logging fakes of `PlatformSet::dry_run()`, and it runs unelevated.

## Who runs what

Agents and automated tests stay within fakes, dry runs and real but harmless calls ([AGENTS.md](../AGENTS.md)).
Live runs, `install`, `uninstall`, `cleanup`, the service and loading the driver are a person's, from an
elevated console on the machine; agents never run them.
