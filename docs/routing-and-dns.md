# Routing and DNS

How a name and an address find their profile. The guards are in [safety](safety.md), the rules in [nrpt](nrpt.md).

## Who gets a name

`ProfileMatcher` holds the connected profiles' domains only. The longest matching domain wins: `a.corp.com` in A
beats `corp.com` in B. When A disconnects, its names fall through to B; a name listed only by disconnected profiles
has no NRPT rule and resolves normally.

## The proxy

One proxy on `listen` (loopback only), UDP and TCP, running while a profile is connected. `QueryHandler` sends:

- bypass names and every profile's endpoint host to the fallback DNS;
- a listed name to its profile's `ProfileResolver`: its own cache, then its upstream through its bound exchange; AAAA
  is an empty NOERROR;
- every other name to the fallback.

TTLs are capped at `max_ttl`. The addresses of a listed answer are routed on the profile's adapter **before** the
answer is sent (`DnsService`), so the first connection already takes the tunnel.

## Who gets an address

`HostClaims` gives each learned /32 to one profile: the first to learn it. When B's answer holds an address A holds,
B does not route it and gets `address_held`; the address leaves B's answer when others remain, otherwise the answer
is kept and the traffic goes through A. The claim ends when A's route expires, A disconnects or A's list drops the
name. Networks need no claim: the longest prefix wins.

## Re-homing

When a connect, a disconnect or a list change gives a name to another profile, the learned routes of that name are
dropped and the Windows cache is flushed; the next answer learns on the right adapter.

## Upstream

Each profile asks its upstream (`upstream`, else the `.conf`'s `DNS=`) through a socket bound to its tunnel address
with `IP_UNICAST_IF` on its adapter, and gets a /32 to each upstream server on its adapter. The exchange is rebuilt
when the adapter's index changes. After 3 failures in a row, or 30 s without an answer, `upstream_unreachable` is
raised until an answer comes. Right after a Connect a failed exchange is retried for up to 5 s before SERVFAIL, and
the first answer since the Connect or a failure flushes the Windows cache ([warm-up](warm-up.md)).

## Learned routes

A learned /32 expires `idle_expiry` after the last answer that named it (checked every 30 s). Every 30 s the guard
is refreshed (fallback servers, on-link networks) and each profile's routes are reconciled with it.
