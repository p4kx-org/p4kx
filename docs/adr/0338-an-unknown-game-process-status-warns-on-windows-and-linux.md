# An unknown game process status warns on Windows and Linux

Decided for #1030. This aligns the process warning policy of ADR-0331 with the independent
installation write guard of ADR-0036.

## Contract

The guard exposes three outcomes through
`p4kx_guard::game_process_status() -> std::io::Result<bool>`:

- `Ok(true)`: a matching process was observed.
- `Ok(false)`: the visible process list was read successfully without a matching process.
- `Err(error)`: absence or presence cannot be determined; the error retains the reason.

The compatible `game_is_running() -> bool` adapter returns true for an observed game or
an unknown result, and false only for a known absence. Both Windows and Linux use this policy.
The CLI and both GUI languages say the game **may** be running, so a failed check is not presented
as confirmed process presence. Reading stays available and the installation write guard does
not consult this result.

## Windows

A successful qprocess query is parsed for process rows. Any failed or unavailable query falls
back to Windows PowerShell, including qprocess's non-zero result for no matching process. The
localized qprocess message cannot distinguish that result reliably from a query failure.

The fallback enumerates process names with terminating error handling and prints their matching
count. A successful zero count establishes absence; a successful positive count establishes
presence. Non-zero exits, I/O failures, malformed/negative/overflowing counts and invalid UTF-8
are unknown. When both probes fail, their reasons are retained in the returned error. Commands
use System32 paths, no interactive window and no PowerShell profile. No game memory is read.

## Linux

Only bounded comm and first-cmdline-argument metadata in numeric, non-symlink process folders is
read. A positive match is conclusive even if another entry was unreadable. Without a match,
directory enumeration or metadata/file read errors make the result unknown. A process disappearing
with NotFound during the scan is skipped. Non-files and symbolic links are not opened.

PID namespaces and hidepid visibility remain limits: a successfully read visible list cannot
prove that no game exists outside that list. Unsupported platforms without this process backend
report no applicable game process.

## Verification

Synthetic command replies exercise successful qprocess without fallback, missing/failing qprocess,
PowerShell missing/failing/I/O errors, valid zero/positive counts and malformed counts. Injected
proc reads cover partial enumeration, unreadable metadata, partial reads, disappearing processes
and a conclusive positive match after other errors. Existing fake-tree, CLI read-only and GUI
warning tests cover the public consumers and keep ordinary work available.
