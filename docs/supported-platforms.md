# Native platforms and release verification

The v0.5.0 contract requires native CLI, GUI and MCP releases. The table shows the targets in the current build and release matrix. The [macOS specification](macos-support.md) defines the additional Apple Silicon release requirement.
[The platform contract](../.config/platforms.json) is authoritative for target identities,
runner labels, package spellings and asset suffixes.

| OS | CPU | Rust target | Native CI runner | Required packages |
| --- | --- | --- | --- | --- |
| Windows | AMD64 | `x86_64-pc-windows-msvc` | `windows-2025` | MSI, portable ZIP |
| Windows | ARM64 | `aarch64-pc-windows-msvc` | `windows-11-arm` | MSI, portable ZIP |
| Linux | AMD64 | `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | tar.gz, AppImage, deb, Flatpak |
| Linux | ARM64 | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | tar.gz, AppImage, deb, Flatpak |

Runner labels describe verification environments. A successful compile or an emulated executable
does not establish native runtime support. Required application behavior is checked on a matching
OS and CPU. The architecture-neutral Blender add-on ZIP and the availability of third-party bpy
wheels are separate from the native application matrix.

## Build and test contract

The matrix job validates the four explicit rows and publishes them to lint, doctest, schema,
concurrency and test jobs. Each job selects the exact compiler and nextest pins and an explicit
native host toolchain. Native Windows linker variables include the complete target. Linux mold
selects the running CPU's binary; the pinned nextest installer uses the runner architecture.

Test producers build all three applications and native test binaries. PE/ELF machine fields,
compiler/nextest hosts, source commit, application version and SHA-256 digests identify the
archive. The artifact and cache identities include the complete Rust target. Consumers validate
the sidecar and archive hash before extracting or running tests.

The partitioned synthetic suite retains nextest process isolation. Explicit smoke steps require
non-empty selections for CLI Archive listing, extraction and conversion; MCP initialization,
tool listing and a read-only request; GUI frames followed by exit; and software-adapter evidence.
Linux uses native lavapipe under Xvfb. Actual step outcomes populate the native runtime artifact
only after successful execution.

GUI and package smoke processes set an absolute `P4KX_FOLDERS_ROOT` to isolate settings, keys,
cache and application data through the production folder resolver. Merely replacing `APPDATA`
does not isolate Windows known folders. [Profile folders](profile-folders.md) defines the override.

The workspace checks exercise native dependency paths including AES/CBC, CRC, SHA-2, flate2,
ruzstd, ring, zstd-sys/cc, tiny-skia, wgpu, rodio/cpal, rfd and Windows resource compilation.
Missing setup tools, graphics capabilities or native libraries fail their required checks rather
than remove an architecture from the matrix. Physical audio, clipboard, dialogs and hardware GPU
checks retain explicit `not_checked`, `success` or `unavailable` states. The final integration
gate reconciles required device evidence; a synthetic runtime artifact alone is not that gate.

## Package identity and update links

Each package includes `p4kx-distribution.json` beside the real application executables:

```json
{"target":"aarch64-unknown-linux-gnu","package":"app-image"}
```

Package kinds are `msi`, `portable-zip`, `tar-gz`, `app-image`, `deb` and `flatpak`.
All three applications in one distribution use the same identity. Linux packages may keep real
executables in a private application directory and expose symlinks on PATH; the identity belongs
beside the real executables.

The update service bounds this optional metadata to 4 KiB and validates its target against the
running binary's compile-time OS and CPU. Host emulation does not change the selected binary
architecture. Missing, malformed, incompatible or oversized identity selects the Release page.

The compatible filename is `P4KX-{version}-{full-Rust-target}{package-suffix}`. The shared
`asset_filename` API and `Get-P4kxAssetName` helper consume the same contract. Only a unique
matching GitHub asset with its expected download URL is selected. Historical AMD64 Windows
MSI/portable names remain eligible for that target. Ambiguous, missing or malformed asset
metadata selects the Release page. The check remains opt-in, anonymous and link-only.

`Release.page` retains its Release-page meaning. `Release.asset` carries the optional compatible
package, and `Release::link()` returns the selected link or page. The GUI uses that shared result.

## Release preflight and evidence

Manual Release workflow runs can select `linux-only` or `windows-only` to validate both native
CPUs and package formats for that OS without external scans or publication. The options are
mutually exclusive. These scoped checks do not replace the final full release matrix from one
source commit.

Before checksums and release notes are assembled, `check-release-matrix.ps1` requires:

- Twelve non-empty native package assets: four Windows packages and eight Linux packages.
- Twelve CycloneDX SBOMs, one per target/application, with the actual version and explicit
  `p4kx:target` and `p4kx:binary` metadata properties.
- Four native runtime artifacts identifying the same source commit and pinned tools, all three
  native application machine identities, and successful required smoke steps.

The filenames are `P4KX-{version}-{target}-{binary}.cdx.json` and
`P4KX-{version}-{target}-runtime.json`. Runtime artifacts state their `native-synthetic` scope
and preserve device-verification states. A missing target/package/binary fails preflight.
The top-level archive digest must match the actual portable ZIP on Windows or tar.gz on Linux.
Each package's execution proof carries its own digest.
Windows runtime evidence additionally covers both the portable ZIP and installed MSI payload,
with package hashes and the exact release EXE hashes. Missing, skipped or mismatched packaged
runtime evidence fails preflight. [Windows packages](windows-packages.md) describes lifecycle,
rollback and CPU replacement verification.
Linux runtime evidence covers all four actual package launchers and retains post-transformation
ELF hashes with unchanged source `.text` hashes, plus Flatpak runtime/app OSTree identity and
Documents-portal grants. The two native Linux runs also require deb and Flatpak in-place
upgrades from separately installed workflow fixtures, launch of the candidate version,
removal and profile preservation. [Linux packages](linux-packages.md) defines the Ubuntu 24.04 / glibc 2.39
host baseline, separate Freedesktop 25.08 runtime, source/relink artifacts and permission policy.
Flatpak additionally requires host Documents API 5 with `GetHostPaths`; an older Documents
service is a startup failure rather than a successful or skipped runtime check.
Packaging checks, signatures, scans, provenance and the full export/device audit remain required
parts of the final release gate. Tag publication requires a completed VirusTotal report for the
exact SHA-256 of each of the twelve native packages. The assembly job compares the scan receipt
against the package bytes, source revision, version and workflow run. The receipt stays with the
workflow evidence and the release notes link the reports. Pending, missing or failed reports
prevent publication. Pull requests and operating-system-specific dry runs do not contact VirusTotal.
A full manual branch run records scan availability in its notes.

All hosted checks use synthetic data. Real Archive tests stay ignored/local or explicitly
private, and no game assets, keys or confidential values enter CI artifacts or releases.
