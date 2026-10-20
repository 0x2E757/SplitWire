# NRPT

The Name Resolution Policy Table sends the listed domains to the proxy. SplitWire writes it in the registry only:
no PowerShell, no script, no child process (`platform::nrpt::RegistryNrpt`).

## Rules

One key per domain of the union of the connected profiles' lists:

```text
HKLM\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig\SplitWire-<domain>
  Version             DWORD     2
  Name                MULTI_SZ  .<domain>, <domain>
  ConfigOptions       DWORD     8
  GenericDNSServers   SZ        127.0.0.53
  IPSECCARestriction  SZ        (empty)
  Comment             SZ        SplitWire
```

`Comment` is written last. Only keys named `SplitWire-*` with `Comment=SplitWire` are listed or deleted; anything
else under `DnsPolicyConfig` is never touched.

## Sync

`NrptSync` compares the rules present with the union and writes the difference only, trying every change; the first
error fails Connect step 9 and raises `nrpt_write_failed` until a sync succeeds. Every change is followed by
`DnsFlushResolverCache`. Whether the DNS client also needs `SERVICE_CONTROL_PARAMCHANGE` is decided in the live
tests.

## When rules exist

Only while the proxy serves: every start removes all SplitWire rules first, the last disconnect removes them, and
a watchdog removes them if the proxy stops on its own.

## Group Policy

`GroupPolicyNrptProbe` reads `HKLM\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient\DnsPolicyConfig` (read-only)
every 30 s. Subkeys there mean Windows ignores the local rules: alert `gp_nrpt`.

## Tests

`RegistryNrpt` and the probe take their root key and path as parameters. Tests use a key of their own under
`HKCU\Software\SplitWire-test` and delete it afterwards; they never open the real `DnsPolicyConfig`.
