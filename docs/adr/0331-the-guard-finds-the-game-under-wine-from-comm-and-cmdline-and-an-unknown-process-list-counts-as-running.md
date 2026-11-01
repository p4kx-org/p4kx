# The guard finds the game under Wine from comm and cmdline, and an unknown process list counts as running

Decided for issue #1017 (part of #827, Linux). It extends the running-game check of ADR-0036 to
Linux; the write guard of that ADR is unchanged.

## Detection

- **Pure function.** `p4kx_guard::game_in_proc_tree(root)` scans a `/proc`-style folder; the
  platform code calls it with `/proc`, the tests with a fake tree built in a temp folder.
- **What identifies the game.** Wine and Proton set the `comm` of a Windows process to its image
  name. `StarCitizen.exe` is exactly 15 bytes, the most the kernel keeps, so `comm` holds it whole;
  `cmdline` carries the Windows image path (`Z:\...\Bin64\StarCitizen.exe`) as its first argument.
  A process counts when its `comm` or the last component of its first `cmdline` argument (split at
  `/` and `\`) is `StarCitizen.exe` or `StarCitizen`, ignoring case. Later arguments are not
  looked at, so `less StarCitizen.exe` is not the game; the launcher and `StarCitizenLauncher.exe`
  are not the game either.
- **Only text files.** Only `comm` and `cmdline` of numeric folders are read, the first 64 and the
  first 4096 bytes. No other file of a process is opened, no handle to it, no memory. A process
  that ends during the scan is skipped. `hidepid` mounts hide other users' processes; the check
  cannot see those.

## Unknown means running

On Linux an unreadable `/proc` makes `game_is_running` return `true`, so the window shows its
warning rather than staying silent. The warning never blocks anything (ADR-0036). Elsewhere without
`/proc` (macOS) the answer stays `false`. The Windows check is unchanged.

## Write guard on Linux

`p4kx-guard` writes nothing; it refuses writes. The canonical comparison resolves every symbolic
link, which covers the `dosdevices/c:` and `dosdevices/z:` links of a Wine prefix; tests build that
layout. Letter case is significant on a Linux file system, as the folding rule of ADR-0036 states;
a case-insensitive mount is still caught by the folder identity (device and inode) check.

## Checks without a Linux equivalent

| Windows check | On Linux |
| --- | --- |
| `qprocess` and the PowerShell process count | replaced by the `/proc` scan above |
| `\\?\` and `\\?\UNC\` prefix handling (`without_verbatim_prefix`, the verbatim test) | does not exist; paths have no such spelling |
| Loopback share `\\localhost\D$\...` (test `a_loopback_share_does_not_hide_the_installation`) | no equivalent; the folder identity check stays and is exercised through the Wine links and the symbolic link test |
| Junction (test `junction_into_the_installation_is_refused`) | a symbolic link, covered by `symlink_into_the_installation_is_refused` and the Wine tests |
| Letter-case folding (test `letter_case_does_not_hide_the_installation`) | none by design; `another_letter_case_is_another_folder_on_a_case_sensitive_file_system` pins that |
| 8.3 short names | do not exist |
