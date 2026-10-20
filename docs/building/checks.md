# Checks

Part of [Building and running](../building.md). Every change keeps them green.

```powershell
cargo fmt --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File tools\verify-abi.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File tools\verify-markdown.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File tools\verify-vendor.ps1
target\package\SplitWire.Window.exe --verify-pages
```

- The tests are unit tests with fakes for every domain component, plus real but harmless Windows calls, nothing
  that changes the machine: a kernel event, a DPAPI round trip on made-up data, atomic writes in the temp folder,
  read-only IP Helper queries (loopback, the on-link networks, the default route's DNS), a bound UDP exchange with
  a local responder on loopback, NRPT rules and the Group Policy probe under a key of their own in
  `HKCU\Software\SplitWire-test` (deleted afterwards, also for the Run value), named kernel objects, tray menus
  built and never shown, and `wireguard.dll` copied to a temporary folder, loaded
  and its symbols resolved with no function called.
- The [integration tests](integration-tests.md) run dry-run engines and drive them as the window and the CLI do.
- `verify-abi.ps1` compiles `tools\abi-probe.c` with `cl.exe` against the installed SDK and the vendored
  `wireguard.h`, and compares every size and offset it prints with the Rust layout tests. A layout test names its C
  counterpart in a trailing comment, `assert_eq!(size_of::<Guid>(), 16); // GUID`, and the probe prints the same
  name; a number declared on one side only is an error too. It also compares every WinRT and COM GUID and
  vtable slot the window declares with the SDK headers ([window](../window/demo-and-checks.md)).
- `--verify-pages` loads every page of every demo scenario, both themes and both width classes, through
  `XamlReader` without showing a window, checks that every bound element exists, lays the pages out in a hidden
  island to check their [alignment](../window/alignment.md), and types into every field to check that no region
  around it reloads.
- `verify-markdown.ps1`: 5000 characters for `AGENTS.md`, 10 000 for `README.md`, 3000 for any other.
- `verify-vendor.ps1` checks the SHA-256 of every file listed in `vendor/wireguard-nt/README.md`, that no unlisted
  file lies there, and that `wireguard.dll` has a valid Authenticode signature by WireGuard LLC.
