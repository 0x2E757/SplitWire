# Lists

Each profile has one list, `profiles\<name>\routes.txt`: one entry per line, UTF-8, compared lower-cased. `#` starts
a comment, also after an entry; blank lines are allowed. An entry is one of:

- a **domain** with at least 2 labels: the name and every subdomain go through the profile's tunnel;
- an **IPv4 address**: a /32 static route;
- an **IPv4 network** with a prefix of at least /8, written with its host bits clear.

A line that is none of these is skipped and reported with its line number. The wording is the same in the service,
the window and the command line, because all of them use `list::RoutesList`:

| Line | Problem |
|---|---|
| `intranet` | a domain needs at least 2 labels |
| `2001:db8::1` | IPv6 is not supported |
| `10.0.0.0/7` | prefix shorter than /8 |
| `10.0.0.1/8` | host bits are set; the network is 10.0.0.0/8 |
| `bad name.com`, `*.example.com`, `300.1.2.3` | not a domain, IPv4 address or IPv4 network |
| an entry already on line 3 (`1.2.3.4` and `1.2.3.4/32` are the same) | already line 3 |

A name goes to the profile whose listed domain matches it most specifically: `a.corp.com` beats `corp.com`.

## All lists together

A name belongs to one profile: an entry exactly equal to another profile's (`1.2.3.4` equals `1.2.3.4/32`) is
refused when added or saved, "also in AcmeCorp line 1: a name belongs to one profile". In hand-edited files, the
later profile in the user's order loses the line, as a problem of its list. Other relations are notes only:

| Note | When |
|---|---|
| covers AcmeCorp's endpoint / tunnel | a network over any profile's endpoint address or another profile's tunnel subnet: its route is refused while connected |
| overrides AcmeCorp line 3 while both are connected | more specific than another profile's entry |
| AcmeCorp line 3 (a.corp.com) overrides it while both are connected | less specific than another profile's entry |
| inside corp.com (line 1) | inside an entry of its own list |

## Combined text

All lists as one text, per profile in the user's order: `[profile <name>]`, then the file's lines verbatim
(trailing blank lines trimmed), one blank line between sections, so section line *k* is file line *k*. Splitting
it back drops the single blank line before a header. The save is refused whole for an unknown or repeated header, a
missing section, a line before the first header or an exact duplicate across sections; a line that is not an entry
is only skipped. Problems carry both numbers: "text line 14 (AcmeCorp line 4): …". Only changed files are written.

## Editing

One entry added or removed, one list or the combined text saved (`ListService`): each change is checked against
every list and carries the revision of the lists it was made on; when a file changed in between (a hand edit), it is
refused with "the lists changed on disk; reloaded". Connected profiles pick a change up at once: statics, the
matcher (learned routes re-homed) and NRPT. Hand edits are picked up within a second.
