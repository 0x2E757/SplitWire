# Alerts

Alerts live in the service (`alerts::AlertStore`, `application::AlertService`). Conditions raise and clear them;
the user dismisses and restores them. Their wording is in `alerts::AlertTexts`.

## Identity

An alert's id is `kind:profile:subject`, stable for its condition (`address_held:AcmeCorp:192.0.2.93`). Raising it
again updates it. A dismissed alert stays hidden until its fingerprint (the condition's details) changes. A blocking
alert keeps a profile, or all of them, off and cannot be dismissed. A cleared alert moves to the history; the
history (`AlertHistory`) keeps the alerts cleared or dismissed since local midnight, at most 200.

## Kinds

| Kind | Scope | Severity | Raised / cleared |
|---|---|---|---|
| `settings_invalid` | global | error, blocking | `splitwire.conf` has a problem / it is valid |
| `profile_settings_invalid` | profile | error, blocking | its `profile.conf` has a problem / it is valid |
| `connect_failed` | profile | error | a Connect failed / the next attempt starts |
| `recover_failed` | profile | error | `TunnelWatch` could not restore the adapter / it did, or the profile disconnected |
| `upstream_unreachable` | profile | error | 3 failures or 30 s without an answer ([warm-up](warm-up.md)) / an answer |
| `nrpt_write_failed` | global | error | a registry write failed / the next sync succeeds |
| `handshake_stale` | profile | warning | the handshake is stale, or none came within 10 s of Connect / a handshake |
| `address_held` | profile | warning | another profile holds an answered address / the claim is released |
| `list_problem` | profile | warning | lines skipped on load / the file is fixed |
| `gp_nrpt` | global | warning | Group Policy defines NRPT rules / it no longer does |
| `route_refused` | profile | info | the guard refused a route / it is no longer refused |

The conditions of a connected profile (`recover_failed`, `upstream_unreachable`, `handshake_stale`,
`address_held`, `route_refused`) end when it disconnects. A new alert is also logged.

## Tray state

`TrayState::of` reads the alerts not dismissed and the profiles' states; the first rule that matches wins:

| State | When |
|---|---|
| `error` | an error alert of a connected profile, `connect_failed`, `recover_failed`, or `settings_invalid` |
| `warning` | any other warning or error alert, or a profile connecting |
| `connected` | a profile is connected |
| `off` | otherwise |

`service` (no pipe) is the client's own state; its tray icon is the error's.
