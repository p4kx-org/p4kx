# Running-game detection

P4KX warns that Star Citizen may be running when the game is found or its visible process list
cannot be checked. The warning does not stop reading or exporting to an allowed output folder.
The separate installation write guard always refuses game-folder writes.

The Rust service API `p4kx_guard::game_process_status()` returns
`std::io::Result<bool>`: true means a matching process was observed, false means a
successfully read visible list had no match, and an error means unknown with its reason.
`p4kx_guard::game_is_running()` retains the boolean API and conservatively returns true
for unknown as well as confirmed presence.

On Windows the check runs qprocess from System32. An unavailable or unsuccessful query uses
Windows PowerShell without a profile or interactive window to count matching process names.
A command failure or invalid count cannot be mistaken for zero processes. On Linux the check
reads bounded comm and cmdline metadata below /proc, including the Windows image path under
Wine and Proton. Processes disappearing during the scan are skipped; unreadable entries make
an otherwise negative result unknown. Positive evidence still reports a running game.

Only process names are inspected. The check does not attach to the game or read its memory.
A PID namespace or hidepid can hide processes outside the visible list, so absence is scoped
to that list.

The CLI checks once per run; P4KX_GAME_CHECK=off disables its notice. The GUI checks in the
background and polls again every 20 seconds. Notices are available in English and German.
The check does not configure or start Wine, Proton or Star Citizen.

The platform contract and synthetic failure cases are recorded in
[ADR-0338](adr/0338-an-unknown-game-process-status-warns-on-windows-and-linux.md).
