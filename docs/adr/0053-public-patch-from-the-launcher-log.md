# The public patch version comes from the RSI Launcher log, matched by change number

The build manifest names a branch such as `sc-alpha-4.7.0` that stays the same across the patches
4.7.0, 4.7.1 and 4.7.2, and a four-part version (`4.10.197.2878`) that is no public number. The
Archive and the files next to `Data.p4k` do not carry the public patch: `Game.log` repeats the
manifest's version and `launcherData.json` holds only the launcher version. The RSI Launcher log
(`logs\log.log` and `logs\log.old.log`, the two files ADR-0005 already reads) names it in the
installer lines, e.g. `Delta update applied (SC LIVE 4.7.2-live.11674325) in …\LIVE`, as
`SC <CHANNEL> <version>-<tag>.<change>`.

`Build::public_patch` is the `<version>` of a log line whose `<change>` equals the Build's change
number exactly; the newest such line wins and the version is kept as logged. When no line has that
change number, for instance because the log rotated away or the launcher never installed that Build,
the public patch is absent and exported as `null`: it is never taken from the nearest older or newer
line. The `P4KX_LAUNCHER_LOGS` environment variable names another log folder, as a test aid.

Only the two log files are read, as in ADR-0005; the settings store, `loginData.json` and every other
launcher file stay unopened. The catalog gains `source.build.publicPatch` while schema version 1 is
still open (`docs/catalog.md`).

## Considered Options

- Derive the patch from the branch or the version: wrong for every patch after `.0` and for the
  four-part versions, so it would be a guess.
- Match the nearest log line by channel and date: also a guess when the log lacks the Build.
- Read `Game.log` or `launcherData.json`: neither carries the public patch.
