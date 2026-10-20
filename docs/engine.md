# Engine

How the engine connects and disconnects profiles, and starts and stops. `application::Engine` builds everything
from a platform set; `Coordinator` holds the one lock every state change takes. The DNS path never takes it.

## Connect

Ten steps in order (`ConnectRun`), each with an undo. A failing step and every step done are undone in reverse;
the profile becomes `failed` with the attempt kept and `connect_failed` raised. Nothing of another profile changes.

| # | Step | Does | Fails when |
|---|---|---|---|
| 1 | `profile` | both settings files valid, keys decrypted, an upstream known | a file is invalid, no `DNS=` and no `upstream` |
| 2 | `proxy` | the first profile binds the proxy (otherwise skipped) | the port is busy |
| 3 | `guard` | base guard refreshed, endpoint resolved through the fallback, this tunnel a never-route of the others | the endpoint does not resolve |
| 4 | `adapter` | adapter created, /32, MTU, no DNS | the driver or IP Helper refuses |
| 5 | `peer` | peer configured, adapter up | the driver refuses |
| 6 | `routes` | upstream /32s and statics on this adapter | its routes cannot be read |
| 7 | `dns` | the bound upstream exchange and the resolver; the first handshake awaited | — |
| 8 | `matcher` | the matcher with this profile; learned routes re-homed | — |
| 9 | `nrpt` | rules synced to the union; Windows cache flushed | a registry write fails |
| 10 | `state` | in `connected=` | `splitwire.state` cannot be written |

`Tunnel::up` does steps 4 and 5 at once; its error says which part failed. Step 7 waits up to 10 s for the first
handshake (probing a peer without a keepalive), so NRPT sends no name to a tunnel that cannot carry it yet;
without one the Connect still succeeds ([warm-up](warm-up.md)).

A connection has no duration: it lasts until Disconnect, Remove or a stop. A Connect of a connected profile
changes nothing.

## Disconnect

Best effort, every part runs: matcher (names fall through), NRPT and flush, resolver, routes and claims, adapter
down and closed, the tunnel subnet released from the other guards, the keys dropped, `connected=`. After the last
profile the proxy stops.

## Start and stop

1. The folder's ACL is set; `splitwire.running` is created. If it was already there, the last run crashed.
2. Every SplitWire NRPT rule is removed and the Windows cache flushed.
3. Settings, profiles and lists are read: `held_off`, `blocked` and their alerts.
4. Restore (`BootRestore`): `connected=` profiles with `connect_at_boot=true`; after a crash every one. An invalid
   `splitwire.conf` restores nothing and leaves `connected=` as is.
5. The scheduler starts: files and traffic samples every second, `TunnelWatch` every 10 s, maintenance
   (guard, expiry, alerts, Group Policy probe, proxy watchdog) every 30 s.

Stop disconnects everything (keeping `connected=`) and deletes the marker.
