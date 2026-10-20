# Domain modules

Part of [Architecture](../architecture.md). Pure logic: no Windows calls, every component unit-tested.

| Module | Components |
|---|---|
| `list` | `DomainName`, `RoutesList`, `ListEntry`, `ListProblem` (one file, [lists](../lists.md)); `ListSet`, `ListConflicts`, `CombinedListText` (all lists); `DomainMatcher`; `ProfileMatcher` (a name to the most specific connected profile, the NRPT union) |
| `net` | `Ipv4Net` (CIDR math); `NetworkProbe` (read-only interface facts) |
| `routes` | `RouteGuard` (never-routes); `RoutePlanner`, `RouteManager` (one per connected profile on its LUID: wanted against installed, learn before answering, expiry, rebind); `LearnedRoutes`, `RefusedRoutes`; `HostClaims` (the first learner holds a /32) |
| `dns` | `wire`, `ResponseBuilder` (read, patch, rebuild without held addresses); `DnsCache`, `Forwarder`, `ServerSet`; `ProfileResolver`, `ResolverSet` (a profile's cache, upstream and health); `UpstreamWarmup` (retries after Connect or idle, [warm-up](../warm-up.md)); `QueryHandler` (bypass, listed, other; claims); `DnsPolicy` (live bypass and TTL cap); `NrptRules`, `NrptSync` ([nrpt](../nrpt.md)) |
| `alerts` | `Alert`, `AlertKind`, `AlertStore` (raise, clear, dismiss, restore, history), `AlertTexts`, `TrayState` ([alerts](../alerts.md)) |
| `profile` | `ProfileName`, `Pin`, `Endpoint`, `TunnelAddress`, `AdapterGuid`; `WgConfParser`, `ConfLine`; `Profile`, `ProfileFacts`, `ProfileCodec`; `SecretKey`, `SecretBytes`; `ImportPreview`, `ImportChecks`, `ProfileFields`; `ProfileState` ([profiles](../profiles.md)) |
| `config` | `KeyValueFile`, `Settings`, `ProfileSettings`, `ProfileIndex`, `StateFile`, `SettingsMerge` ([configuration](../configuration.md)) |
| `tunnel` | `TunnelSpec`, `TunnelLink`, `TunnelHealth`, `UpStage`; `HandshakePolicy`, `HandshakeWait` (Connect's wait for the first handshake); `EndpointResolver`, `EndpointAddresses` ([tunnel](../tunnel.md)); `ConnectStep`, `ConnectAttempt` (the ten steps and their outcomes, [engine](../engine.md)) |
| `logs`, `stats`, `clock` | `LogBuffer`, `Log`, `LogFile` (warnings and errors on disk); `QueryStats`, `UpstreamHealth`, `TrafficSeries` (300 one-second samples); `Clock`, local midnight |
| `cli` | `Command`, `RunArgs`, `DurationArg`, `ExitCode`, `PipeCommands` and the texts they print, `App` ([command line](../building/command-line.md)) |
