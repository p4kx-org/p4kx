# Channel and launcher discovery

P4KX discovers installed Channels and public patch labels from one shared launcher-source implementation. CLI, GUI, MCP and installation protection use the same ordered source snapshots. Discovery reads directory metadata and the specific configuration/log files below; it does not read launcher credentials, registry dumps, game memory or network data.

## Inputs and priority

`WineEnvironment::new(home, wine_prefix)` and the five public fields of `SearchPaths` remain available. Owned `WineEnvironment` builders add compatibility-data, XDG, Steam installation/library and Bottles roots. `SearchRecipe::standard()` captures platform/environment inputs once and refreshes filesystem topology on every request. `SearchRecipe::explicit(paths)` and the explicit `SearchPaths` snapshot API never look up the host process environment. `snapshot_with_manual` borrows current manual settings without cloning configured candidate vectors before admission.

On Unix, standard candidates include, in this order:

1. The absolute nonempty `WINEPREFIX`, `~/Games/star-citizen`, and `STEAM_COMPAT_DATA_PATH/pfx`.
2. Configured Steam installations, `$XDG_DATA_HOME/Steam` (default `~/.local/share/Steam`), `~/.steam/steam`, `~/.steam/root`, and both known Flatpak Steam data aliases below `~/.var/app/com.valvesoftware.Steam/` (`data/Steam` and `.local/share/Steam`). Only `steamapps/libraryfolders.vdf` and `config/libraryfolders.vdf` are parsed, in that order. Both the numeric-path and numeric-object `path` layouts are accepted. Every configured/external library is examined at `steamapps/compatdata/<numeric-appid>/pfx`.
3. Explicit Steam libraries, then explicit Bottles roots, `$XDG_DATA_HOME/bottles/bottles`, the Flatpak `~/.var/app/com.usebottles.bottles/data/bottles/bottles` root, and immediate `~/Games/*` candidates for Lutris/custom installations.

No recursive home search takes place. Empty/relative environment inputs are ignored. A directory component first requires its exact spelling. If absent, only a unique ASCII-case match is accepted; ambiguous siblings produce a typed diagnostic. Returned host paths preserve the actual directory spelling. Symlink aliases of a discovered prefix are deduplicated by resolved identity, retaining the first priority.

The native launcher comes first, then prefixes in discovery order and users in sorted directory order. Only `log.log` and `log.old.log` are opened below each resolved `drive_c/users/<user>/AppData/Roaming/rsilauncher/logs`. Wine drive-letter libraries are interpreted in their original prefix, through `drive_c` or `dosdevices/<letter>:`. Drive-relative paths such as `C:Games` and paths containing `..` are refused; a bare `C:` retains the drive-root representation. UNC/device paths and unknown non-C drive mappings are not interpreted. Known non-C bases must remain absolute in their observed prefix namespace or have an explicitly observed absolute resolution; a Windows drive-prefix join cannot escape into a drive-relative host path.

The default-library and native-launcher overrides replace only their corresponding slots; they do not remove Wine sources. Manual folders and Archives remain available when automatic discovery is incomplete. `P4KX_LAUNCHER_LOGS` remains an exclusive log-folder override **only** for `PublicPatches::standard()` and `standard_report()`. Channel detection, MCP and the guard do not adopt that override.

## One physical parser, ordered interpretation contexts

A snapshot opens regular log files and retains their actual file identities and handles. A physical file is parsed once per scan, even if native and Wine paths alias it. Its facts are then replayed in the original ordered source occurrences, preserving contextual Wine translation and patch precedence. With occurrences A, C, A, the final A still wins. Removing its later context changes the stamp and restores C as winner.

Public patches use the last 16 MiB of each file and lines no longer than 64 KiB. Library discovery reads the whole file with a 1 MiB line limit. Overlong lines are skipped. A tail starts by discarding a line only if its first byte falls inside that line. Old-log patches are applied before current-log patches; later source occurrences win. The patch map retains the newest 4096 distinct change numbers. Parsing and replay both have aggregate fact admissions.

## Bounds and partial reports

One `DiscoveryLimits` value covers prefix discovery, configuration parsing, source opening and scan admission. Production defaults are:

| Resource | Limit |
| --- | ---: |
| Filesystem/source operations | 16,384 |
| Aggregate directory entries | 16,384 |
| One directory snapshot | 4,096 |
| Candidate roots | 256 |
| Prefixes | 1,024 |
| Windows users | 2,048 |
| Source interpretation contexts | 1,024 |
| Configuration files | 32 |
| Aggregate configuration bytes | 4 MiB |
| One configuration file | 1 MiB |
| Configuration object depth | 16 |
| Aggregate configuration tokens | 65,536 |
| Parsed launcher facts | 1,048,576 |
| Aggregate parsed fact payload | 32 MiB |
| Ordered fact replays | 1,048,576 |
| Aggregate replayed payload | 32 MiB |

Candidate vectors are admitted before copying into the resolved snapshot; borrowed seed iteration stops at the aggregate cut. Directory enumeration is sorted only after a complete, bounded snapshot. An oversized or failing directory is discarded as a whole, so an arbitrary filesystem iteration order cannot choose a partial prefix sample. Earlier admitted paths and manual inputs survive in the report. Configuration roots are published only after their entire bounded configuration parses successfully.

`DiscoveryReport<T>` retains usable partial data and typed `DiscoveryIssue`s, including original paths and I/O causes. Parsed and replayed meaningful payloads each have a separate 32 MiB aggregate admission, limiting retained path/version/change payloads even for many long library paths or aliases. Library replay bytes count distinct retained interpreted paths; the separate replay-fact count bounds every occurrence, including duplicate output paths. Patch replay bytes count each applied version/change payload. Missing probe targets are normal absence; a confirmed configuration/log disappearing when opened, or a read failure after opening, is incomplete. Limits and ambiguous paths are explicit failures. A later request retries incomplete discovery and scanning.

CLI warnings go to stderr before the relevant result/output operation. Existing installation-protection startup/archive boundaries preserve their warning-only behavior; only secondary warning delivery is best effort. The GUI retains current typed discovery issues and the latest background-open protection report, shows the existing localized warning alert and offers manual addition. Its tooltip shows the first technical reason. Newly accepted manual settings are registered and protected immediately using the existing manual classification path. Automatic Steam/Wine/log discovery refreshes on a worker: one scan runs and only the newest pending request is kept; stale completions cannot replace a newer manual selection. The initial standard scan remains before the native window starts. Existing manual protection/classification can still perform synchronous directory work; the worker boundary concerns automatic discovery. Worker spawn/disconnect failures use the same warning surface and do not replace primary Archive/comparison errors. MCP adds bounded textual discovery notices to its existing notes surface, including an omitted-occurrence count when needed; tool payloads and JSON schemas do not gain diagnostic fields.

## Cache identity and protection

`LauncherStamp` covers current roots, resolved default-library targets, every ordered source occurrence, Wine drive targets and both known files in every source. Aliased users in the same prefix retain their original paths and ordering; only actual physical file capture is deduplicated. A file state distinguishes missing, nonregular, unavailable and regular; regular states carry actual opened-file identity, length and modification time. Source creation/removal and alias/context changes therefore invalidate an old cache even when surviving log metadata is unchanged. Replacement by a new file with identical size/time also invalidates it.

MCP reuses a public-patch cache only after complete discovery and a complete scan with the same stamp. An incomplete read cannot be cached as a healthy empty source set. Guard refresh uses the same source snapshot and stamp. Logged library values remain logical paths, so their downstream symlink/junction bindings are not all represented in this stamp. On a complete unchanged-stamp hit, the guard reuses its bounded measured path list and resolves each path again through the existing installation resolver; it does not reparse the logs. A binding from A to B therefore adds B while preserving A. Default-library targets are already stamped. The pass is nonrecursive over the existing count-/payload-bounded result, and installations already protected remain protected after a source disappears, changes or fails. Direct archive opening also protects that archive's installation.

Residual metadata limitation: rewriting the same file identity while preserving both its length and modification timestamp cannot be detected reliably by this stamp. Fresh scans detect size/time changes during their read; complete-cache reuse does not hash whole log contents. This bound avoids unbounded extra content passes.

## Validation and platforms

Synthetic tests own their directories, logs/configurations and injected filesystem. They cover candidate families, multi-library VDF, exact/unique/ambiguous case, duplicate prefixes and physical logs, A/C/A precedence, Wine interpretation, work cuts, real read errors, cache creation/removal/replacement/retry and monotone protection. Frontend tests exercise the same measured reports used by the product. They never require installed game data or credentials.

The implementation uses portable Rust and the existing native target matrix: Windows and Linux on AMD64 and ARM64. Linux packages retain Ubuntu 24.04 / glibc 2.39 as minimum; Flatpak additionally requires Documents API 5. Those platform contracts do not change through discovery. Source-only preparation is not evidence of native execution; release acceptance still requires the repository's native four-target checks and applicable device tests.

## Format references

Steam configuration follows Valve's [KeyValues declaration](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/tier1/KeyValues.h). Bottles locations follow its [native-to-Flatpak migration documentation](https://docs.usebottles.com/flatpak/migrate-bottles-to-flatpak). Steam Flatpak data aliases are described in the [Flathub Steam package](https://github.com/flathub/com.valvesoftware.Steam). Physical identity uses [same-file Handle](https://docs.rs/same-file/latest/same_file/struct.Handle.html), also used elsewhere in this workspace.
