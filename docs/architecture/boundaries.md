# Boundary traits

Part of [Architecture](../architecture.md). A trait exists only where the logic meets the OS, the network, the
clock or the transport; each one has a fake for the tests and, where it writes, a logging fake for the dry run.

| Trait | Real implementation | Why |
|---|---|---|
| `Tunnel` | `WireGuardTunnel` | the tunnel kind: `up(spec)` gives a `TunnelLink{luid, if_index}`; `down`, `health`, `refresh`. Nothing above it knows it is WireGuard |
| `WireGuardDriver`, `WireGuardAdapter` | `WireGuardNt` | the wireguard-nt driver one level above its ABI; dropping an adapter closes it |
| `AdapterAddressing` | `IpHelperAddressing` | an adapter's /32, MTU, metric and empty DNS settings |
| `RouteTable` | `IpHelperRouteTable` | the routes SplitWire owns, per adapter LUID |
| `NrptRules`, `NrptPolicyProbe` | `RegistryNrpt`, `GroupPolicyNrptProbe` | SplitWire's NRPT rules; whether Group Policy has its own |
| `DnsCacheFlusher` | `DnsCacheFlush` | the Windows DNS cache |
| `NetworkProbe` | `IpHelperProbe` | read-only interface facts: on-link networks, the default route's DNS |
| `DnsExchange`, `DnsExchangeFactory` | `UdpDnsExchange`, `BoundExchangeFactory` | one query to one server; a profile's exchange bound to its adapter |
| `SecretStore` | `DpapiSecretStore` | machine-bound encryption of a profile's plaintext |
| `FolderAccess` | `FolderSecurity` | the data folder's ACL |
| `Clock` | `SystemClock`, `LocalClock` | expiry, staleness, alert times and waits (`sleep`: a manual clock moves on instead), tested without waiting; local midnight for today's alerts |
| `EngineLink` (CLI) | `PipeClient` | the command line's way to the engine, faked in its tests |
| `EngineTransport` (window) | `PipeClient`, `DemoTransport` | the window's requests to the engine: the pipe, or the demo service of the `--demo` scenarios |

Files are not behind a trait: the tests use real scratch folders, which is harmless.
