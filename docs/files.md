# Files

```text
%ProgramData%\SplitWire\         SYSTEM + Administrators only
  splitwire.conf                 global settings
  splitwire.state                connected=…                 (service-owned)
  splitwire.running              crash marker                (service-owned)
  profiles\index.conf            order and colours           (service-owned)
  profiles\<name>\profile.dat    the DPAPI blob
  profiles\<name>\profile.conf   per-profile settings
  profiles\<name>\routes.txt     the list
  logs\                          + read for interactive users
    splitwire.log (+ .1)         warnings and errors, 1 MB and one rollover
    splitwire.error.log          the last failed start
```

`application::DataFolder` builds every path, a profile's only from a validated name ([profiles](profiles.md)).
`run --dir` points it at another folder. The settings files are described in [configuration](configuration.md), the
list in [lists](lists.md).

`logs\splitwire.log` holds the log's warnings and errors across restarts (`logs::LogFile`): past 1 MB it becomes
`splitwire.log.1`, replacing the older one. `splitwire.error.log` holds why the last start failed. The window reads
both directly. A line is `2026-10-19 09:14:11.060Z WARN  - nrpt: message`: the UTC time with milliseconds, the
level, the profile or `-`, the source and the message on one line. `logs::LogLineText` writes it (the file and the
console echo) and reads it back (the window), so both sides share one format.

## Writes

Every file is written to `<file>.tmp`, flushed to the disk, then moved over the file with
`MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`, so a reader sees the old or the new file, never a part. Several
files that belong together (a settings save) are written as `.tmp` first and replaced only when all were written.

## Access

At every start the service sets the owner Administrators and the protected DACL `D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`
on the data folder, which also takes back a folder a non-administrator created first. `logs\` adds
`(A;OICI;0x1200a9;;;IU)`: interactive users read the logs, which never hold secrets. `FolderSecurity` applies both;
the blobs are machine-scope DPAPI, so this ACL is what keeps them from other users.
