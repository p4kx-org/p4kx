# Linux packages and native verification

The v0.5.0 release contract includes AMD64 and ARM64 versions of the GUI, CLI and MCP server
in tar.gz, AppImage, deb and Flatpak distributions. Target rows and asset suffixes come from
[the platform contract](../.config/platforms.json); [Linux package settings](../.config/linux-packaging.json)
define the application identity, baseline and sandbox capabilities.

The minimum host baseline for tar.gz, AppImage and deb is **Ubuntu 24.04 / glibc 2.39**.
Flatpak uses **Freedesktop Platform 25.08**, with its separate runtime ABI. These are release
requirements. Native results are established by the source-bound runtime artifacts of the
corresponding GitHub run, rather than by a successful local Windows build.

## Install and start

Select `x86_64-unknown-linux-gnu` for AMD64 or `aarch64-unknown-linux-gnu` for ARM64.
Every package name starts with `P4KX-<version>-<target>`.

| Suffix | Contents and start command |
| --- | --- |
| `.tar.gz` | Extract anywhere. Run `bin/p4kx-gui`, `bin/p4kx` or `bin/p4kx-mcp` inside the extracted directory. |
| `.AppImage` | Make executable and run it for the GUI; add `--cli` or `--mcp` before application arguments for the CLI or MCP server. |
| `.deb` | Install with `sudo apt install ./P4KX-<version>-<target>.deb`; run `p4kx-gui`, `p4kx` or `p4kx-mcp`. |
| `.flatpak` | Install with `flatpak install --user ./P4KX-<version>-<target>.flatpak`; run `flatpak run io.github.p4kx_org.p4kx`. |

For AppImage systems without usable FUSE, the same real runtime supports
`./P4KX-<version>-<target>.AppImage --appimage-extract-and-run`. Add `--cli` or `--mcp`
after that runtime option. An installed AppImage is not required for this extraction mode.

The AppImage retains linuxdeploy's relative root desktop and icon links and adds the matching
`.DirIcon` link. Packaging checks their installed targets and original bytes, preserves the exact
three-command `AppRun`, and tests link resolution and dispatch after relocating a synthetic AppDir.

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
export destinations require write access to the selected destination. CLI callers can use
`flatpak document-export --app=io.github.p4kx_org.p4kx --forbid-write /absolute/Data.p4k`
and use the returned portal path. Grant a separate output directory with `--allow-write`.
The installation write guard still applies to granted destinations.

The sandbox process namespace does not establish whether a host game is running. An observed
positive game-process match is conclusive; a negative sandbox scan produces the existing unknown
status and warning. It never starts a host process to obtain a broader view.

Audio previews use ALSA. A native desktop needs a configured default ALSA device or the
PipeWire/PulseAudio ALSA bridge. Flatpak uses its runtime and PulseAudio socket. Physical playback,
clipboard exchange, chooser interactions and hardware GPU behavior have separate evidence states;
the bounded software-rendered GUI check does not claim those tests passed.

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
included in release checksums and provenance. Release preflight requires all eight Linux packages
alongside the four Windows packages.

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
belong to the single `fuse.portal` mount with source `portal` at `$XDG_RUNTIME_DIR/doc`.
Generic FUSE filesystem labels from `stat` do not establish portal identity. The native logs
retain the actual mount metadata and Flatpak/findmnt versions before testing portal operations.

[`release-linux.yml`](../.github/workflows/release-linux.yml) builds on the matching Ubuntu 24.04
native CPU and verifies packages on a fresh matching runner. The consumer validates the source
SHA, pinned compiler/nextest, baseline, CPU and package hashes before execution. It then runs:

- Real tar executables, the AppImage runtime and AppRun dispatch, installed deb commands and
  installed `flatpak run` commands for all three applications.
- CLI version/help, synthetic Archive listing, byte-exact extraction and CryXml conversion;
  MCP help, initialization, tool listing and an exact Unicode read response.
- Three actual GUI frames under Xvfb with software graphics, followed by normal exit.
- Flatpak application and runtime OSTree identity, effective permission metadata, actual FUSE
  Documents-portal read-only input/write-output grants and installation-alias write refusal.
- deb/Flatpak uninstall and profile-sentinel preservation.

The synthetic Archive comes from `p4kx-testkit`'s `native_fixture` example. No game data or real
Archive key enters the jobs. An unavailable required capability fails the package job; no skipped
operation is written as a successful runtime proof. Portal-origin behavior remains subject to
the actual FUSE-grant check, and the release job produces no passing proof if it exposes an
installation-alias bypass.

For a branch-only native Linux validation, dispatch the existing `release.yml` workflow with
`linux-only=true`. It runs preparation and both Linux producer/consumer rows, skips Windows
packages and external scans, and cannot assemble or publish a Release. Do not create a tag:

```powershell
gh workflow run release.yml --repo p4kx-org/p4kx --ref codex/implement-v0.5.0 -f linux-only=true
```

The normal pull-request trigger also validates Linux packages, together with the complete
Windows matrix. Publication remains restricted to version tags and successful full-matrix gates.
