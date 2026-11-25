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
archive. The artifact and cache identities include the complete Rust target. Test execution
validates the sidecar and archive hash before extracting or running tests. Windows and Linux
download their target's archive on native partition runners. Apple Silicon validates and runs
the complete archive on its native build runner, with one partition.

The synthetic suite retains nextest process isolation on all five targets. Explicit smoke steps require
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
checks retain explicit `not_checked`, `success` or `unavailable` states. The owner's Windows
AMD64 desktop is the only required physical-device verification. The other four targets use the
required hosted native checks for acceptance. Record their unobserved device capabilities in
the release audit.

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

The Windows MSI lifecycle launches its original diagnostic parent in a tracked hidden console.
The consumer records the bound request, launch handle and script inventory locally. An immediate
checkpoint copies the original journal and source metadata. On ARM64 a second checkpoint
observes the Registry capture boundary. A third checkpoint observes the first install
outcome on both architectures. A checkpoint records the phase observed in the original
journal and carries an unproven lifecycle verdict. The original files remain in the local
checkpoint and snapshot directories for diagnostic upload.

A separate Finish supervisor observes the original lifecycle. The WaitFinish step uses a
bounded probe to find its terminal receipt and binds the observed digest to the original
request and supervisor. The ARM64 lifecycle wait retains its probe milestones in the wait
receipt. CollectFinish requires those bound receipts, the original successful result, the
closed journal and final cleanup. The terminal evidence upload must succeed before the
executed release payload can be recorded. The last diagnostic upload collects the early
checkpoints and worker logs if the job reaches that step.

The ten-minute worker cap, thirty-five-minute diagnostic deadline and forty-five-minute
job limit also apply to the native release consumer. The observer step reserves one minute
for owned-process cleanup and its receipt. Deadline cleanup can stop the bound parent.
A stopped parent leaves the installer and descendant cleanup unproven. The separate
harmless native controls measure the observer and runner boundaries.

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
hashes, Mach-O deployment target and signing receipt for the final DMG. Hosted Apple Silicon
consumers verify the exact DMG on macOS 26 and the approved macOS 27 runner.
Tagged assembly compares the observed runner version with the release pin in
[macOS host evidence](../.github/scripts/macos-host-evidence.ps1). The current approval
covers the `xcode-27` guest running macOS 27.0.1. Dry runs record the actual macOS
version and build. Update the pin when GitHub supplies and verifies the newer guest.
For PR merge `4d334e716cbc87e27525e8e095410a19103c51f5`, both native
guest lanes mounted the original DMG read-only and ran the relocated app.
They also verified the exact add-on ZIP in Blender 5.2.4. The minimum
guest reported macOS 26.7.2 (25G332). The current guest reported macOS
27.0.1 (26A434). The native assembly verified both receipts before
publishing the tested PR artifact.
[Minimum guest](https://github.com/p4kx-org/p4kx/actions/runs/45576709479/job/139106444957) ·
[Current guest](https://github.com/p4kx-org/p4kx/actions/runs/45576709479/job/139106444987) ·
[Native assembly](https://github.com/p4kx-org/p4kx/actions/runs/45576709479/job/139107694349)

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
Packaging checks, signatures, scans, provenance, the complete export audit and Windows AMD64 desktop checks remain required
parts of the final release gate. Tag publication requires a completed VirusTotal report for the
exact SHA-256 of each of the thirteen native packages. The assembly job compares the scan receipt
against the package bytes, source revision, version and workflow run. The receipt stays with the
workflow evidence and the release notes link the reports. Pending, missing or failed reports
prevent publication. Tagged assembly also requires the exact native and add-on asset inventory.
It verifies each build provenance and SBOM bundle against the expected signing workflow
and run. The verified subject names and SHA-256 values must match the assembled files.
Each CycloneDX predicate must match its published SBOM. Both checksum files cover the
assembled assets and are excluded from their own input lists.
Pull requests and operating-system-specific dry runs do not contact VirusTotal.
A full manual branch run records scan availability in its notes.

All hosted checks use synthetic data. Real Archive tests stay ignored/local or explicitly
private, and no game assets, keys or confidential values enter CI artifacts or releases.

## Windows AMD64 desktop verification

Run the physical checks on the owner's Windows AMD64 desktop with the candidate's packaged GUI.
Use an absolute `P4KX_FOLDERS_ROOT` for the application profile and separate fixture and export
directories. Use a disposable desktop session if the current clipboard must be preserved.
Launch the GUI normally so it remains open for interaction. [Profile folders](profile-folders.md)

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
   Record activity before and after moving the model, the desktop session, adapter and driver.
   [Windows GPU observation](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/)
4. **Audio.** Run the packaged GUI procedure below on the native default audio output.
   Include the result of `a_missing_device_is_reported_to_the_player_and_does_not_panic`
   from the matching native CI run or a native execution of that test.
   [Desktop audio verification](#desktop-audio-verification)

Record the exact candidate package and observed results in the release audit. Keep the
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

### Submit the Windows desktop receipt

Use a JSON array of observations with schema 1, scope native-device, the
x86_64-pc-windows-msvc target and host OS windows with architecture amd64. Match
the version, revision, workflow run and attempt, package SHA-256 and GUI SHA-256
to the runtime evidence. Use the same revision for the fixtures and record
each outcome and observation in the checks object. The
[receipt validator](../.github/scripts/windows-desktop-device-evidence.ps1)
defines the exact fields and required checks.

Run **Validate Windows AMD64 desktop device evidence** on the same release tag.
Supply the original Release run ID and the receipt JSON array. After intake
succeeds, re-run the failed jobs of the original Release run. The assembly job
validates the signed receipt against the candidate runtime record and package.

## Desktop audio verification

Use the default WASAPI output on the owner's Windows AMD64 desktop for the required physical
playback check. The ignored test opens the production audio output and checks sample consumption
after start and seek, the position while paused, and consumption stopping when the player closes.
The same fixture can supply optional playback observations with ALSA or CoreAudio.

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
