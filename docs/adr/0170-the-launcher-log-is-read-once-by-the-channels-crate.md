# The launcher log is read once, by the Channels crate, within bounds

ADR-0053 takes the public patch from the RSI Launcher log; ADR-0005 limits the launcher files read
to `log.log` and `log.old.log`. Two crates had each their own copy of that rule, and every open of
an Archive, every Channel and the detection read the logs again.

`p4kx-channels` is the only crate that reads the launcher logs. It holds the file list, the folder
lookup and one scan that collects the library folders and the public patches in a single pass.
`p4kx-guard` and the frontends use its refreshed ordered source snapshots and typed reports.
The [discovery contract](../channel-discovery.md) defines prefix roots, limits and cache identity.

- `Build` is pure: `Build::with_public_patch(Option<String>)` sets the value, nothing is read, and
  `Archive::open` reads no launcher file. A test opens an Archive in a child process whose
  `P4KX_LAUNCHER_LOGS` names a log with the Build's change number and finds no public patch.
- `PublicPatches` is the change number to version map of one scan. `detect` and
  `detect_with_patches` read the logs once for all Channels; a front-end that needs the patch of
  an Archive's Build (`p4kx info`, the catalog and ship base `source`, the comparison and schema
  drift report, the Blender package, the MCP `archive_info`) gets the map and passes it on:
  the reporting standard map in the CLI, the detection of the Channel list in the GUI, and the
  same source-aware snapshot/cache in MCP. Library code takes the map in its options and never
  the environment.
- `P4KX_LAUNCHER_LOGS` is an exclusive log-folder override only for `PublicPatches::standard()`
  and `standard_report()`. Detection, MCP and guard use native and Wine source occurrences from
  their recipe. Explicit `SearchPaths` and explicit recipes never read the process environment.
- A physical log is read and parsed once per snapshot scan. Ordered native/Wine occurrences are
  interpreted separately, preserving prefix-specific libraries and later-source patch precedence
  even when a file occurs again after another source. Source stamps retain actual opened-file
  identity and ordered context topology; partial discovery or reads cannot admit a healthy cache.
- A line is read as bytes with its break stripped and never held beyond its limit. The tail and the
  64 KiB bound apply to the public patches only: only the last 16 MiB of a file count (the launcher
  rotates at 5 MiB) and a line over 64 KiB is skipped, so the newest line wins in an oversized log.
  The first line of a tail is dropped only when the tail starts mid-line.
- Library folders, on which the guard depends, are read from the whole file, with lines up to 1 MiB;
  only a longer line is skipped.
- A file that is not a regular file is not opened. A non-interruption read error ends the file,
  retains earlier facts and its typed cause, and requires a retry on the next request. Only an
  initial missing-file probe counts as ordinary absence; errors opening confirmed files remain
  incomplete. Guard refresh never removes protection learned from an earlier source.
- A label counts when its version is non-empty numeric parts separated by dots and its change is
  1 to 20 digits. The map keeps the newest 4096 changes; the old log is folded in before the
  current one, so the current log wins.

The comparison report and the schema drift report state `public_patch` in `source.<side>.build`, the
MCP `BuildShape` carries it; both are additive.
