# Native platforms and release verification

The v0.5.0 contract requires native CLI, GUI and MCP releases. The table lists the required build and release targets. The [macOS specification](macos-support.md) defines Apple Silicon acceptance.
[The platform contract](../.config/platforms.json) is authoritative for target identities,
runner labels, package spellings and asset suffixes.

| OS | CPU | Rust target | Native CI runner | Required packages |
| --- | --- | --- | --- | --- |
| Windows | AMD64 | `x86_64-pc-windows-msvc` | `windows-2025` | MSI, portable ZIP |
| Windows | ARM64 | `aarch64-pc-windows-msvc` | `windows-11-arm` | MSI, portable ZIP |
| Linux | AMD64 | `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | tar.gz, AppImage, deb, Flatpak |
| Linux | ARM64 | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | tar.gz, AppImage, deb, Flatpak |
| macOS 26+ | Apple Silicon | `aarch64-apple-darwin` | `macos-26` | DMG |

Runner labels describe verification environments. A successful compile or an emulated executable
does not establish native runtime support. Required application behavior is checked on a matching
OS and CPU. The architecture-neutral Blender add-on ZIP and the availability of third-party bpy
wheels are separate from the native application matrix. [Blender package exchange](media-fixture-selection.md#native-package-exchange-in-ci)
lists the native consumer environments and the Linux ARM64 Blender version boundary.

## System memory

The complete unfiltered Catalog v3 producer must use less than 40 GB of
application memory. Individual SDK queries and the reduced Basetool catalog
producer must use less than 2 GB. Measure on an available host and estimate
the headroom for other processes on 64 GiB and 16 GiB systems, respectively.
Verify both LIVE and current PTU with the packaged full producer. Record
memory and temporary storage separately.
[Catalog memory acceptance](catalog-partitioning.md#system-memory-acceptance)

## Build and test contract

The matrix job validates the five explicit rows and publishes them to lint, doctest, schema,
concurrency and test jobs. Each job selects the exact compiler and nextest pins and an explicit
native host toolchain. Native Windows linker variables include the complete target. Linux mold
selects the running CPU's binary; the pinned nextest installer uses the runner architecture.

Test producers build all three applications and native test binaries. PE, ELF and Mach-O machine fields,
compiler/nextest hosts, source commit, application version and SHA-256 digests identify the
archive. The artifact and cache identities include the complete Rust target. Consumers validate
the sidecar and archive hash before extracting or running tests.

The partitioned synthetic suite retains nextest process isolation. Explicit smoke steps require
non-empty selections for CLI Archive listing, extraction and conversion; MCP initialization,
tool listing and a read-only request; and GUI frames followed by exit. Windows and Linux check a
software adapter. Linux uses native lavapipe for both the X11 window under Xvfb and the Wayland
window under isolated Weston. The Wayland check disables X11 for that process and requires the
viewport to report native 2� scaling. The packaged Linux smoke uses Xvfb. macOS checks native
Metal. Actual step outcomes populate the native runtime artifact only after successful execution.

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

Each package carries `p4kx-distribution.json` to identify its target and format.

```json
{"target":"aarch64-unknown-linux-gnu","package":"app-image"}
```

Package kinds are `msi`, `portable-zip`, `tar-gz`, `app-image`, `deb`, `flatpak` and `dmg`.
All three applications in one distribution use the same identity. Linux packages may keep real
executables in a private application directory and expose symlinks on PATH; the identity belongs
beside the real executables. The relocatable macOS bundle places it in `Contents/Resources`.

The update service bounds this optional metadata to 4 KiB and validates its target against the
running binary's compile-time OS and CPU. Host emulation does not change the selected binary
architecture. Missing, malformed, incompatible or oversized identity selects the Release page.

The compatible filename is `P4KX-{version}-{full-Rust-target}{package-suffix}`. The shared
`asset_filename` API and `Get-P4kxAssetName` helper consume the same contract. Only a unique
matching GitHub asset with its expected download URL is selected. Historical AMD64 Windows
MSI/portable names remain eligible for that target. Ambiguous, missing or malformed asset
metadata selects the Release page. The check remains opt-in, anonymous and link-only.

`Release.page` retains its Release-page meaning. `Release.asset` carries the optional compatible
package, and `Release::link()` returns the selected link or page. The GUI, CLI and MCP use that shared result.

## Release preflight and evidence

A new push to a pull request supersedes its earlier CI, release-validation, MSI probe and
workflow-lint runs. Main-branch, merge-queue, manual and tag-triggered runs retain independent
groups.

Manual Release workflow runs can select `linux-only` or `windows-only` to validate both native
CPUs and package formats for that OS without external scans or publication. The options are
mutually exclusive. These scoped checks do not replace the final full release matrix from one
source commit.

Release validation on a pull request includes the native Windows observation control.
The manual Release option `windows-observation-control-only` runs a small process-lifetime
control on both native Windows CPUs. Set `$releaseCandidateRef` to the candidate branch:

```powershell
gh workflow run release.yml --ref $releaseCandidateRef -f windows-observation-control-only=true
```

The control verifies a source-bound worker across separate workflow steps, uploads its active
phase checkpoint, and requires a closed journal after release or a failed terminal on deadline
expiry. Receipts bind the original request, PID and start time, target, source hashes and
unchanged runner tracking fingerprint. Their scope is process observation and owned cleanup.
MSI package acceptance still requires the original installation and runtime proofs.
The control's checkpoint and completion artifacts retain `native_completion_unproven: true`.

The Windows MSI lifecycle runs its original diagnostic parent in a tracked hidden console.
A separate diagnostic upload captures its original request, launch handle and script inventory
before the first snapshot. It includes the parent-start receipt when that file is already
available. Upload errors are recorded as diagnostics.
Eight checkpoints upload bounded copies of its file journal and source metadata before the
final wait. Checkpoints carry an unproven verdict. Acceptance still requires the original
successful result, its closed journal, `final-cleanup/end`, matching input/source receipts
and the existing installed runtime checks. The ten-minute worker cap, thirty-five-minute
absolute deadline and forty-five-minute job limit remain unchanged. Checkpoint uploads consume
the original observer allowance; a delayed upload can delay the next parent observation.
Expiry can stop only the bound parent and cannot prove installer or descendant cleanup.
Each checkpoint captures and uploads in separate runner steps within a combined three-minute
allowance. Fixed unproven progress files identify request validation, process observation and file
preservation. Each Snapshot runs all synchronous validation, process reads, waits and file
preservation in its own captured child. Its caller reserves bounded cleanup within the original
absolute deadline and caps capture at 100 seconds, leaving the existing two-minute capture and
one-minute upload steps unchanged. Caller timeout stops only that owned Snapshot child; its
receipts retain an unproven verdict and cannot attest original-worker or installer cleanup.
Checkpoint artifacts retain original copies under `checkpoints/NN` and caller progress and logs
under `snapshots/NN`. The original result remains the acceptance authority.

Before checksums and release notes are assembled, `check-release-matrix.ps1` requires:

- Thirteen non-empty native packages, comprising four for Windows, eight for Linux and one DMG for macOS.
- Fifteen CycloneDX SBOMs, one per target/application, with the actual version and explicit
  `p4kx:target` and `p4kx:binary` metadata properties.
- Five native runtime artifacts identifying the same source commit and pinned tools, all three
  native application machine identities, and successful required smoke steps.

The filenames are `P4KX-{version}-{target}-{binary}.cdx.json` and
`P4KX-{version}-{target}-runtime.json`. Runtime artifacts state their `native-synthetic` scope
and preserve device-verification states. A missing target/package/binary fails preflight.
The top-level archive digest must match the actual portable ZIP on Windows, tar.gz on Linux or DMG on macOS.
Each package's execution proof carries its own digest. On macOS it includes the signed executable
hashes, Mach-O deployment target and signing receipt for the final DMG. Tagged release preflight
also requires device evidence for the exact DMG on macOS 26 and the current stable macOS release.
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
exact SHA-256 of each of the thirteen native packages. The assembly job compares the scan receipt
against the package bytes, source revision, version and workflow run. The receipt stays with the
workflow evidence and the release notes link the reports. Pending, missing or failed reports
prevent publication. Pull requests and operating-system-specific dry runs do not contact VirusTotal.
A full manual branch run records scan availability in its notes.

All hosted checks use synthetic data. Real Archive tests stay ignored/local or explicitly
private, and no game assets, keys or confidential values enter CI artifacts or releases.

## Windows and Linux ARM64 desktop verification

Run on a native ARM64 Windows or Linux desktop using the candidate's packaged GUI. Use an
absolute `P4KX_FOLDERS_ROOT` for the application profile and separate fixture and export
directories. Use a disposable desktop session if the current clipboard must be preserved.
Launch the GUI normally so it remains open for interaction. For Flatpak, use the package's
documented profile and portal grants. [Profile folders](profile-folders.md), [Linux packages](linux-packages.md)

Generate the fixtures from the candidate's source revision. Use two new absolute directories.
The examples write synthetic Archives and the text reference.

```text
cargo run --locked -p p4kx-testkit --example native_fixture -- <absolute-smoke-fixture-directory>
cargo run --locked -p p4kx-testkit --example material_reference_fixture -- <absolute-model-fixture-directory>
```

1. **File dialogs.** Open `native-smoke.p4k` through the native Archive picker. Cancel a second
   open and confirm the selected Archive stays open. Select `Data/hello.txt` for extraction,
   cancel the first output-folder selection, and check that the export directory stays empty.
   Complete the selection in a writable directory with a Unicode name. Compare the extracted
   file's SHA-256 with the generated `hello.txt` and record both hashes.
2. **Clipboard.** Copy `Data/hello.txt` using **Copy** in the Text preview and paste it into a
   separate native text editor. Compare it with the generated `hello.txt`, checking Unicode,
   the combining character, line breaks and tab. Record any editor newline conversion. In
   `virtual-root.p4k`, use the multiline Query editor for the reverse direction. Copy from
   the external editor, paste into P4KX and compare the resulting text without running the
   Query. Exercise copying the empty `Data/empty.txt` preview and pasting with an empty
   clipboard into the Query editor. Record the observed result of each operation.
3. **Viewer and GPU.** Open `virtual-root.p4k` and select `Data/Objects/Test/cube.cgf`. Open
   the Model tab and confirm that the cube draws and responds to orbit and zoom. Record
   observed process-attributed GPU activity separately from the viewer outcome. On Windows,
   Task Manager's GPU Engine column can identify the adapter and engine used by `p4kx-gui`.
   On Linux, supported DRM drivers expose per-process engine counters in `/proc/<pid>/fdinfo/`.
   Check the counters before and after moving the model. Record the desktop session, adapter,
   driver and observation. [Windows GPU observation](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/), [Linux DRM counters](https://docs.kernel.org/gpu/drm-usage-stats.html)
4. **Audio.** Run the packaged GUI procedure below on the native default audio output.
   Include the result of `a_missing_device_is_reported_to_the_player_and_does_not_panic`
   from the matching native CI run or a native execution of that test.
   [Desktop audio verification](#desktop-audio-verification)

Record one result per target and package tested and link it from the release audit. Keep the
hosted runtime receipt as its own evidence.

| Field | What to retain |
| --- | --- |
| Candidate | Version, full source revision from the target's runtime receipt, and native package workflow run and attempt. |
| Package and GUI | Target, package filename and SHA-256, and SHA-256 of the executable actually launched. Match these against the corresponding package smoke receipt. |
| Host | OS version and build or kernel, native CPU, desktop session, GPU and driver, and audio backend. |
| Observations | Test date, fixture source revision, and per-check outcome for dialogs, clipboard, viewer, hardware GPU, audio start/pause/seek/stop, and no-device behavior. Retain the observed result, a log or comparison where available, or the reason a check could not run. |

Record `success` only for an observed check. Use `failed` for a broken outcome, `unavailable`
for a missing capability and `not_checked` for a deferred check. The release audit keeps
any required check without successful evidence open. Publish structural results and synthetic
comparisons. Keep personal paths and clipboard contents out of public evidence.

### Validate the recorded ARM64 desktop evidence

Use a JSON array with a record for each tested Windows or Linux ARM64 package. The receipt
formats these existing observations as follows.

| Fields | Expected values |
| --- | --- |
| `schema`, `scope` | Integer `1` and string `native-device`. |
| `target`, `version`, `revision`, `workflow_run`, `workflow_attempt` | The exact native package runtime receipt values. The attempt is a string recorded by the package consumer. |
| `package`, `package_filename`, `archive_sha256`, `gui_sha256` | Package kind from the platform matrix, canonical release filename, and lower-case SHA-256 of the package and launched GUI executable. |
| `fixture_revision`, `tested_on` | Full source revision used to produce the fixture and the test date as `yyyy-MM-dd`. |
| `host` | `os` (`windows` or `linux`), `architecture` (`arm64`), `os_version`, `os_build_or_kernel`, `desktop_session`, `gpu`, `gpu_driver` and `audio_backend`. |
| `checks` | Each check below has an `outcome` and an `observation` describing the result or why it could not be checked. |

- `file_dialogs` records Archive open, cancellation and extraction. Successful evidence includes
  the matching lower-case `expected_sha256` and `observed_sha256` from the fixture and exported file.
- `clipboard` records cross-program text transfer. `viewer` records the model preview and controls.
  `hardware_gpu` reports adapter activity separately, including an unavailable capability.
- `audio_start`, `audio_pause`, `audio_seek` and `audio_stop` record native playback behaviour.
  `no_audio_device` records the native no-device check. Successful evidence names
  `a_missing_device_is_reported_to_the_player_and_does_not_panic` in its `test` field.

The offline release preflight binds these records to the already verified package receipts and
requires successful file dialog, clipboard, viewer, playback and no-device evidence for each
ARM64 target. Supply the original release run ID for the selected package artifacts.

```powershell
.github/scripts/check-release-matrix.ps1 -AssetDirectory $assets -Version $version -Revision $revision -Arm64DeviceEvidencePath $receipt -ReleaseRunId $releaseRunId
```

This command verifies recorded outcomes against the candidate. Device observations still come from
the native desktop checks above.

For a tagged release, use the original tagged Release run's Windows and Linux ARM64
packages. Submit the combined Windows and Linux receipt array to **Validate Windows and Linux
ARM64 device evidence** on that same tag and enter the original Release run ID. The intake
checks the signed runtime receipts from the original build and attests their binding to the
desktop evidence. Keep local paths and clipboard contents out of the submitted receipts.

After both this intake and the [macOS device intake](macos-support.md#submit-the-native-device-receipt)
succeed, re-run only the failed jobs of the original Release workflow. Tagged assembly
authenticates both receipts and rechecks the Windows and Linux observations against the
exact release packages and original producer attempts. A rebuilt package requires matching
device evidence.

## Desktop audio verification

On each matching native CPU, use a usable default audio output on the desktop under test.
The same ignored test opens the production audio output and checks sample consumption after
start and seek, the position while paused, and consumption stopping when the player closes.
It plays a quiet synthetic signal through WASAPI on Windows, ALSA on Linux, or CoreAudio on macOS.

```text
cargo test --locked -p p4kx-gui --lib -- audio::player::native_tests::native_output_consumes_samples_after_start_and_seek_and_stops_when_closed --ignored --exact
```

For the packaged GUI verification, generate the fixture in a disposable absolute directory.

```text
cargo run --locked -p p4kx-testkit --example native_fixture -- <absolute-output-directory>
```

Launch the native candidate with an absolute `P4KX_FOLDERS_ROOT` to isolate its profile. Set
a low preview volume, open `native-desktop-audio.p4k` and play `Data/Sounds/tone.wem` in
the Audio tab. Pause and confirm the sound and playhead stop. Resume, seek to the middle
and confirm playback resumes from there. Select `Data/readme.txt`
and confirm that closing the preview stops playback. Record the OS version, CPU, source
commit, GUI binary and package hashes, audio device or session backend, and observed results.
[Profile folders](profile-folders.md)

An unavailable output is recorded separately. The ordinary synthetic test injects
`NoDevice` and verifies that the player reports the failure and stops.
