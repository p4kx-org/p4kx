# Linux packages and native verification

The v0.5.0 release contract includes AMD64 and ARM64 versions of the GUI, CLI and MCP server
in tar.gz, AppImage, deb and Flatpak distributions. Target rows and asset suffixes come from
[the platform contract](../.config/platforms.json); [Linux package settings](../.config/linux-packaging.json)
define the application identity, baseline and sandbox capabilities.

The minimum host baseline for tar.gz, AppImage and deb is **Ubuntu 24.04 / glibc 2.39**.
Flatpak uses **Freedesktop Platform 25.08**, with its separate runtime ABI, and additionally
requires **host Documents-portal API 5 or later with `GetHostPaths`**. A host can meet the native
Ubuntu/glibc baseline while still lacking this separate Flatpak prerequisite. These are release
requirements. Native results are established by the source-bound runtime artifacts of the
corresponding GitHub run, rather than by a successful local Windows build.

## Linux-Hilfe auf Deutsch

### Paket wählen

Prüfe die CPU mit `uname -m`. Wähle für `x86_64` (AMD64, Debian `amd64`) das Ziel
`x86_64-unknown-linux-gnu` und für `aarch64` (ARM64, Debian `arm64`) das Ziel
`aarch64-unknown-linux-gnu`. Der Release-Vertrag sieht auf beiden CPUs GUI, CLI und MCP
als tar.gz, AppImage, deb und Flatpak vor. Die Installation und die Aufrufe für alle drei
Programme stehen in der gemeinsamen Tabelle. [Installieren und starten](#install-and-start)

Die Basis für tar.gz, AppImage und deb ist Ubuntu 24.04 mit glibc 2.39. Flatpak nutzt
Freedesktop Platform 25.08 und benötigt auf dem Host ein Documents-Portal ab API 5
mit funktionierender, authentifizierter Hostpfadabfrage. P4KX kann ein vorhandenes `Data.p4k`
auf beiden CPUs mit dem jeweils nativen Paket öffnen. Für Star Citizen und Wine gelten
deren eigene Plattformanforderungen.

### Archiv und Wine-Präfix auswählen

Wähle **Archiv hinzufügen …** für eine `Data.p4k` oder **Ordner hinzufügen …** für den
Kanalordner mit der `Data.p4k` oder für die Star-Citizen-Bibliothek. Nutze den
tatsächlichen Linux-Dateipfad, etwa unter `drive_c` im Wine-Präfix.

Für ein eigenes Präfix setze `WINEPREFIX` vor dem Start auf dessen absoluten Pfad
oder ergänze den Pfad im aktiven Profil:

```toml
[wine]
prefixes = ["/pfad/zum/StarCitizen-Praefix"]
```

Die Suche beginnt mit `WINEPREFIX` und den konfigurierten Präfixen in ihrer Reihenfolge.
Danach prüft P4KX `~/Games/star-citizen` und `STEAM_COMPAT_DATA_PATH/pfx`. Steam wird
unter `$XDG_DATA_HOME/Steam`, `~/.steam/steam`, `~/.steam/root`, `~/.local/share/Steam`
sowie unter `data/Steam` und `.local/share/Steam` im Profil `~/.var/app/com.valvesoftware.Steam/`
gesucht. Aus `steamapps/libraryfolders.vdf` und
`config/libraryfolders.vdf` ermittelte Bibliotheken tragen ihre Proton-Präfixe unter
`steamapps/compatdata/<numerische App-ID>/pfx` bei.

P4KX sucht Bottles-Präfixe direkt unter `$XDG_DATA_HOME/bottles/bottles` (Standard
`~/.local/share/bottles/bottles`) oder unter
`~/.var/app/com.usebottles.bottles/data/bottles/bottles`. Ausdrücklich konfigurierte
CrossOver-Wurzeln werden zwischen diesen beiden Orten geprüft. Für Lutris oder eigene
Installationen prüft P4KX anschließend direkte Unterordner von `~/Games`. P4KX prüft die
exakte Schreibweise jedes Pfadbestandteils zuerst und akzeptiert andernfalls nur einen
eindeutigen Treffer ohne Beachtung der ASCII-Groß- und Kleinschreibung. Eine mehrdeutige
oder unvollständige Suche wird gemeldet. [Suchpfade und Prioritäten](channel-discovery.md#inputs-and-priority)

### Profil und Flatpak-Dateizugriff

| Inhalt | XDG-Pfad bei nativen Paketen | Standard |
| --- | --- | --- |
| Einstellungen und gespeicherter Archivschlüssel | `$XDG_CONFIG_HOME/P4KX` | `~/.config/P4KX` |
| Cache | `$XDG_CACHE_HOME/P4KX` | `~/.cache/P4KX` |
| Anwendungsdaten | `$XDG_DATA_HOME/P4KX` | `~/.local/share/P4KX` |

Ist eine XDG-Variable leer, ungesetzt oder relativ, gilt der Standardpfad. Einstellungen
stehen in `settings.toml`, der gespeicherte Schlüssel in `archive.key`. Unter Flatpak
liegen diese Dateien in `~/.var/app/io.github.p4kx_org.p4kx/config/P4KX`. Cache und
Anwendungsdaten liegen im selben Flatpak-Profil unter `cache/P4KX` und `data/P4KX`.
[Profilordner und expliziter Profilpfad](profile-folders.md)

P4KX öffnet das Spielarchiv lesend. Flatpak hat Lesezugriff auf `~/Games/star-citizen`.
Für andere Präfixe kannst du den konkreten Pfad mit `:ro` freigeben. Alternativ
erteilst du in der Dateiauswahl eine Lesefreigabe für das Archiv. Wähle den separaten
Exportordner mit Lese- und Schreibzugriff. Dateiexporte verwenden diesen Ordner auch
für temporäre Dateien. Die Installation bleibt durch die Prüfung der Hostpfade
geschützt. [Flatpak-Freigaben und Befehle](#profiles-audio-and-sandbox-access)

Die Paketprüfung protokolliert CLI, MCP und GUI je CPU. Die synthetische GUI-Prüfung
zeichnet Frames mit Xvfb und lavapipe. Die gehosteten Linux-Läufe lassen Hardware-GPU,
hörbare Wiedergabe, interaktive Dateidialoge und Zwischenablage offen. Zur Release-Abnahme
gehört der physische Nachweis auf dem Windows-AMD64-Desktop des Maintainers.
[Native Prüfungen](#required-native-checks)

## Install and start

Use `uname -m` to check the Linux host CPU. Select `x86_64-unknown-linux-gnu` for
`x86_64` (AMD64 or Debian `amd64`) and `aarch64-unknown-linux-gnu` for `aarch64`
(ARM64 or Debian `arm64`). Every package name starts with `P4KX-<version>-<target>`.
P4KX can open an existing `Data.p4k` natively on either CPU. Star Citizen and Wine have
their own platform requirements.

| Suffix | Contents and start command |
| --- | --- |
| `.tar.gz` | Extract with `tar -xzf ./P4KX-<version>-<target>.tar.gz`. Open the extracted `P4KX-<version>-<target>` directory and run `./bin/p4kx-gui`, `./bin/p4kx` or `./bin/p4kx-mcp`. |
| `.AppImage` | Run `chmod +x ./P4KX-<version>-<target>.AppImage`, then `./P4KX-<version>-<target>.AppImage` for the GUI. Put `--cli` or `--mcp` before the application's arguments to start the CLI or MCP server. |
| `.deb` | Install with `sudo apt install ./P4KX-<version>-<target>.deb`; run `p4kx-gui`, `p4kx` or `p4kx-mcp`. |
| `.flatpak` | Install with `flatpak install --user --bundle ./P4KX-<version>-<target>.flatpak`; run `flatpak run io.github.p4kx_org.p4kx`. |

Direct AppImage launch needs a FUSE device accessible to the current user and a compatible
mount helper on `PATH`. On Ubuntu 24.04, the `fuse3` package supplies `fusermount3`.
The runtime already includes libfuse3. If FUSE is unavailable, use
`./P4KX-<version>-<target>.AppImage --appimage-extract-and-run`. Add `--cli` or `--mcp`
after that runtime option.

The AppImage retains linuxdeploy's relative root desktop and icon links and adds the matching
`.DirIcon` link. Packaging checks their installed targets and original bytes, preserves the exact
three-command `AppRun`, and tests link resolution and dispatch after relocating a synthetic AppDir.

The tar.gz GUI uses the host's window and graphics libraries, the desktop file chooser portal,
and ALSA (`libasound2t64` on Ubuntu 24.04). The deb declares its shared-library and portal
dependencies so `apt` can install them. [Ubuntu runner dependencies](../.github/scripts/install-linux-package-dependencies.ps1)

The Flatpak bundle points to Flathub for Freedesktop Platform 25.08. To install that runtime
separately in the same user installation, use the native Flatpak on the target CPU.

```text
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//25.08
```

Flatpak CLI and MCP commands are selected before the application ID:

```text
flatpak run --command=p4kx io.github.p4kx_org.p4kx --help
flatpak run --command=p4kx-mcp io.github.p4kx_org.p4kx --archive /path/visible/in/sandbox/Data.p4k
```

The deb installs real executables and the shared distribution identity in `/usr/lib/p4kx`,
with `/usr/bin` symlinks, desktop metadata, icons, manual pages and legal notices in standard
locations. Its architecture field follows the target, and generated native shared-library
dependencies are combined with the window/audio/portal libraries loaded dynamically.
Dependency analysis uses the three absolute staged executable paths and the staged Debian control directory.
The Debian version adds packaging revision `-1` and maps an upstream prerelease separator to
`~`, retaining Debian's prerelease ordering.
`lintian` warnings and errors fail packaging.
The package-local rule for `initial-upload-closes-no-bugs` follows the
[tag's documented non-Debian-archive exception](https://udd.debian.org/lintian-tag.cgi?tag=initial-upload-closes-no-bugs):
these upstream releases have no Debian ITP bug. It applies only to that package/changelog context.
Native synthetic-package checks require that this rule is active and unrelated errors still fail.

The Linux window, desktop file, icons and Flatpak use the same reverse-DNS identity
`io.github.p4kx_org.p4kx`. The existing theme, typefaces and rendering behavior are retained.

## Find an Archive

In the GUI, use **Add Archive…** to register a `Data.p4k`, or **Add folder…** to register the
Channel folder containing it or the Star Citizen library. Use the Linux host path, such as a path
below `drive_c` in a Wine prefix. A manual entry remains available if automatic discovery cannot
identify the installation. For the native CLI, use `p4kx channels add` with that same host path.

```sh
p4kx channels add '/absolute/path/to/prefix/drive_c/Games/StarCitizen/LIVE/Data.p4k'
p4kx channels
```

For tar.gz, substitute `bin/p4kx` from the extracted package. For AppImage, pass `--cli`
before `channels`. For Flatpak, use `flatpak run --command=p4kx io.github.p4kx_org.p4kx`
with a path visible in the sandbox. [Flatpak file grants](#profiles-audio-and-sandbox-access)

For a custom Wine prefix, set `WINEPREFIX` to its absolute path before starting P4KX, or add
it to the active profile's `settings.toml`.

```toml
[wine]
prefixes = ["/absolute/path/to/prefix"]
```

P4KX checks `WINEPREFIX` and configured prefixes first, followed by `~/Games/star-citizen`
and `STEAM_COMPAT_DATA_PATH/pfx`. It searches Steam compatibility data, including external
libraries listed in `steamapps/libraryfolders.vdf` or `config/libraryfolders.vdf`.
Native Bottles roots, configured CrossOver bottle roots and Flatpak Bottles roots follow.
P4KX then checks immediate `~/Games` children for Lutris or custom installs. Each component
uses an exact spelling first, then a unique ASCII case-insensitive match. Ambiguous or
incomplete discovery is reported.
[Search roots and priority](channel-discovery.md#inputs-and-priority), [Profile folders](profile-folders.md)

## Upgrade and remove installed packages

For a new deb, run `sudo apt install ./P4KX-<version>-<target>.deb` over the existing
`p4kx` package for the same CPU. Remove it with `sudo apt remove p4kx`.

For a newer Flatpak bundle, run `flatpak install --user --bundle ./P4KX-<version>-<target>.flatpak`.
The bundle replaces the deployed commit of the existing stable app ref for that CPU. Remove
the app with `flatpak uninstall --user io.github.p4kx_org.p4kx`. These removal commands
preserve profile data.

For tar.gz and AppImage, launch the newly extracted directory or new image directly.

## Profiles, audio and sandbox access

Native packages use the usual XDG profile folders; Flatpak uses its per-application profile.
Packages own neither settings nor the stored Archive key. Removing deb or Flatpak without an
explicit data-deletion request preserves profile data. Smoke processes use the actual
[`P4KX_FOLDERS_ROOT` resolver](profile-folders.md), and preservation sentinels contain synthetic
text rather than a real key.

The Flatpak grants Wayland, fallback X11, direct-rendering devices, IPC, PulseAudio and network
access for the opt-in anonymous update check. It has read-only access to
`~/Games/star-citizen`. It requests neither unrestricted home/host access nor host process
execution. For another Wine prefix, add a read-only grant for the precise location:

```text
flatpak override --user --filesystem=/absolute/custom/prefix:ro io.github.p4kx_org.p4kx
```

File chooser selections use Documents-portal grants: Archive input requires read access;
export destinations require read/write access. Atomic file exports also need access to the
selected parent directory for sibling temporary files; a file-only grant can be refused. CLI callers can use
`flatpak document-export --app=io.github.p4kx_org.p4kx --forbid-write /absolute/Data.p4k`
and use the returned portal path. Grant a separate output directory with `--allow-write`.
The installation write guard still applies to granted destinations. A file-only Archive grant
can hide its host parent and `Bin64`; authenticated origin lookup alone does not prove that the
source is an ordinary copy. If installation membership or existing-target identity cannot be
established, the write is refused with an uncertainty explanation. A separate write grant does
not override source protection.

The Flatpak window asks for a parent folder and one file or package-folder name for these
exports. The system chooser selects the parent; the displayed full path is the actual output.
For a Blender package, its name creates a child of that selected parent. Selecting a folder
does not bypass the installation or source checks.

All three Flatpak applications check the Documents prerequisite before opening a profile,
loading a stored key or starting normal work. CLI/MCP startup failure uses exit 1 and the
`[p4kx.runtime.documents-api5]` diagnostic; the GUI shows a prerequisite notice. `--help` and
`--version` remain available. API 4 does not provide an operational mode. The release packages
do not bundle or install a replacement host Documents service.

The sandbox process namespace does not establish whether a host game is running. An observed
positive game-process match is conclusive; a negative sandbox scan produces the existing unknown
status and warning. It never starts a host process to obtain a broader view.

Audio previews use ALSA. A native desktop needs a configured default ALSA device or the
PipeWire/PulseAudio ALSA bridge. Flatpak uses its runtime and PulseAudio socket. The hosted
Linux package checks leave audible playback, interactive chooser and clipboard use, and
hardware GPU behaviour unverified on both CPUs. Release acceptance requires physical
desktop evidence on the maintainer's Windows AMD64 desktop.
[Desktop audio verification](supported-platforms.md#desktop-audio-verification)

## Source, licensing and package identity

Each distribution includes `p4kx-distribution.json` beside the real executables. The update
selector checks the compiled target and format before choosing a compatible download.

AppImage bundles the GUI's required native shared libraries. Their installed Ubuntu binary and
source versions, ELF machines, hashes and original copyright notices are inventoried. The
embedded static runtime is built natively from the exact type2-runtime source commit, pinned
libfuse/squashfuse sources and hash-checked signed Alpine packages, including compiler runtime
archives. A native linker map rejects archives outside this recorded source closure.

Each target additionally releases
`P4KX-<version>-<target>-appimage-corresponding-source.tar.gz`. It contains original library
sources and notices, distribution recipes and patches, exact APK inputs, runtime build files
and [relink instructions](../packaging/linux/runtime/RELINK.md). AppImage transformation does
not change the applications' `.text` hashes. Package-specific ELF hashes still change when
RPATH, stripping or packaging changes their surrounding bytes.

The three target-specific application SBOMs include the bundled library/runtime and source
artifact identities. Package, producer metadata, SBOM, source artifact and runtime evidence are
included in release checksums and provenance. Release preflight requires all eight Linux
packages. [Five-target release preflight](supported-platforms.md#release-preflight-and-evidence)

## Required native checks

Contract fixtures, packaging stages, tools and runtime logs use a fresh `RUNNER_TEMP` task keyed
by run, attempt, target and producer/consumer role, outside the cached Cargo build directory.
Consumers require the complete producer matrix to succeed; missing artifacts still fail verification.

Build dependencies explicitly include Ubuntu's `librsvg2-common` GdkPixbuf SVG loader. Before
compiling applications, native checks compose the shipped desktop assets, compose an SVG-only
copy and require a corrupt SVG to fail with a detailed filename/decoder report. This verifies
AppStream image decoding independently of optional packages preinstalled on the runner image.

Flatpak stores the declared `fallback-x11` socket together with its implied `x11` marker. The
effective-metadata check requires both markers and still rejects unrestricted X11 without
`fallback-x11`, extra sockets, filesystem access or bus permissions. The installed metadata is
retained in the native consumer logs before validation.

Documents-portal checks inspect the kernel mount table with `findmnt`: each granted path must
belong to the single `fuse.portal` mount with source `portal`. The authenticated
`GetMountPoint` route may reach its visible sandbox target through a runtime `doc` symlink.
Generic FUSE filesystem labels from `stat` do not establish portal identity. The native logs
retain the actual mount metadata and Flatpak/findmnt versions before testing portal operations.
They separately measure the sandbox runtime directory, canonical document path and mount
inventory; the host runtime-directory spelling does not substitute for those observations.
They also record the measured Documents API version, XML introspection and installed portal
package versions through a bounded probe. The [portal identity core](portal-alias-core.md)
documents the separate host-mapping and write-policy contracts. Native validation uses two
isolated sessions: the stock API-4 session must reject normal Flatpak startup while preserving
help/version, and a source-pinned API-5 session must pass the original packaged runtime checks
and actual grants. Package-smoke launchers bind their temporary writable permission to each
session's actual output directory so the host can hash the extracted bytes. The additional
service binaries are disposable CI inputs, not package
payloads. Compilation, synthetic evidence tests and a recorded interface number do not by
themselves establish real FUSE or package acceptance.

[`release-linux.yml`](../.github/workflows/release-linux.yml) builds on the matching Ubuntu 24.04
native CPU and verifies packages on a fresh matching runner. The consumer validates the source
SHA, pinned compiler/nextest, baseline, CPU and package hashes before execution. It then runs:

- Real tar executables, the AppImage runtime and AppRun dispatch, installed deb commands and
  installed `flatpak run` commands for all three applications. The AppImage checks require
  both direct FUSE launch and explicit extract-and-run.
- CLI version/help, synthetic Archive listing, byte-exact extraction and CryXml conversion;
  MCP help, initialization, tool listing and an exact Unicode read response.
- Three actual GUI frames under Xvfb with software graphics, followed by normal exit.
- Flatpak application and runtime OSTree identity, effective permission metadata, actual FUSE
  Documents-portal read-only input/write-output grants and installation-alias write refusal.
- In-place upgrades from an older native deb package version and a distinct Flatpak commit
  under the same application ref, with exact replacement metadata and executable bytes,
  candidate launch, uninstall and profile-sentinel preservation. Clean install and launch
  checks run separately. The predecessor packages are workflow-only fixtures built with
  the same native executables as the release candidate and an earlier package version.
  v0.5.0 introduces these Linux release formats.

The synthetic Archive comes from `p4kx-testkit`'s `native_fixture` example. No game data or real
Archive key enters the jobs. An unavailable required capability fails the package job; no skipped
operation is written as a successful runtime proof. Portal-origin behavior remains subject to
the actual FUSE-grant check, and the release job produces no passing proof if it exposes an
installation-alias bypass.

The native Ubuntu CI jobs for PR #1117 head `405e97268` checked merge revision
`5d3d917f424657990fd775b8fae8ac843a3fd528`. All native AMD64 and ARM64 test
shards, doctests and schema checks passed.
[CI run](https://github.com/p4kx-org/p4kx/actions/runs/45315878072)

The branch package run built version `0.4.2` from the same merge revision.
Native AMD64 and ARM64 package consumers passed the four-format application checks,
software-rendered desktop frames, Documents grants and package lifecycle checks. These
AppImage receipts cover extract-and-run.
[AMD64 package job](https://github.com/p4kx-org/p4kx/actions/runs/45315878218/job/138196477374) ·
[ARM64 package job](https://github.com/p4kx-org/p4kx/actions/runs/45315878218/job/138196477207)

Release merge `8b6032a9d` passed the four native package checks on AMD64 and ARM64.
Each architecture's digest-verified package logs include separate AppImage runtime
receipts for direct FUSE launch and extract-and-run. Its `executed-packages.json`
binds both launch modes to the corresponding native target and merge revision.
[AMD64 package job](https://github.com/p4kx-org/p4kx/actions/runs/45338631037/job/138282038446) ·
[ARM64 package job](https://github.com/p4kx-org/p4kx/actions/runs/45338631037/job/138282038436)

The tagged v0.5.0 candidate requires its own hosted Linux package evidence on both CPUs.

For a branch-only native Linux validation, dispatch the existing `release.yml` workflow with
`linux-only=true`. It runs preparation and both Linux producer/consumer rows. Use the branch
dispatch for a validation run and a version tag for Release publication.

```powershell
gh workflow run release.yml --repo p4kx-org/p4kx --ref codex/implement-v0.5.0 -f linux-only=true
```

The Release workflow's pull-request trigger for packaging changes runs the Linux, Windows
and macOS package checks. Publication requires a version tag and successful full-matrix gates.
