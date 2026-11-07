# Windows packages

The release target is `x86_64-pc-windows-msvc` for AMD64 and `aarch64-pc-windows-msvc` for
ARM64. For version `<version>` each target produces `P4KX-<version>-<target>.msi` and
`P4KX-<version>-<target>-portable.zip`. The full Rust target disambiguates both names. Older
AMD64 releases use their historical `-x86_64` names, supported by the shared update selector.

Choose ARM64 on a Windows ARM64 device and AMD64 on an AMD64 device. Both packages contain the
GUI, CLI and MCP server, licence files, third-party notices, a distribution identity sidecar,
`windows-runtime.json` and `MICROSOFT-REDISTRIBUTABLE-NOTICE.txt`. Any app-local Microsoft runtime
DLL has its target PE machine, SHA-256 and Visual Studio redistributable source recorded.
Windows system DLLs and Universal CRT are operating-system prerequisites, not copied files.

Runtime selection reads the newest stable three-component version below the selected Visual
Studio installation's `VC/Redist/MSVC`, then the exact target `x64` or `arm64` directory. That
directory must contain exactly one regular `Microsoft.VC<digits>.CRT` directory. Debug CRT,
`onecore`, foreign CPUs, missing targets and ambiguous CRT families are not fallbacks. The host
import analyzer follows the applications' actual dependency graph; copied DLLs retain their PE
machine and SHA-256 checks. Inventory source paths record the selected version and actual CRT
family, including VC143 and VC145. The notice links the applicable Microsoft terms and official
[2022](https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution) and
[2026](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution) lists.

Before native compilation, each Windows producer runs `test-windows-runtime.ps1`. Synthetic PE
headers and controlled import results exercise both CPU choices and CRT families through the real
copy/inventory validation path, including rejection cases. This establishes packaging logic, not
native runtime execution; the packaged application checks below remain required.

## Installation and replacement

The MSI installs for the current user without administrator rights in
`%LOCALAPPDATA%\Programs\P4KX`, with one Start menu shortcut and one user PATH entry.
There is one installed product across CPU choices. Newer versions upgrade it; lower versions
are rejected. Choosing another CPU package explicitly at the same version replaces the current
product on Windows ARM64, including when replacing an emulated AMD64 installation with ARM64.
The same-version replacement is intentional; it does not create a second PATH installation.

Settings and `%APPDATA%\P4KX\archive.key` are user data, never MSI-owned files. Installation,
upgrade, rollback, CPU replacement and uninstall preserve them. Portable packages run from an
extracted folder and use the same default profile. An isolated profile is available through
`P4KX_FOLDERS_ROOT`, described in [profile folders](profile-folders.md).

Update checks are opt-in and link-only. The common selector reads the target/package sidecar
next to the running EXE and selects its CPU/package asset; unavailable or inconsistent identity
falls back to the Release page. It never downloads, installs or switches CPU automatically.

## Native verification and supported Windows evidence

The native matrix selects `windows-2025` (Windows Server 2025, AMD64) and `windows-11-arm`
(Windows 11, ARM64). Release runtime JSON records the actual OS version/build, source SHA and
native tool identities. These are the intended CI verification systems; workflow definitions
and synthetic MSI construction alone do not establish a successfully executed release.
No untested Windows 10 minimum or ARM64 device result is claimed by this document.

Each release run must execute the packaged release EXEs from both the portable ZIP and the MSI:

- CLI lists the synthetic Archive, extracts Unicode text byte-for-byte and converts compressed CryXml.
- MCP negotiates the protocol, lists tools and reads that synthetic text using a read-only tool.
- GUI draws three frames and exits within 120 seconds with an isolated process profile.
- The native renderer names a required software adapter. Device checks remain explicit.
- MSI lifecycle verifies machine/template/component metadata, install, upgrade, blocked downgrade,
  transactional rollback with restored product/file hashes/PATH, profile/key preservation and uninstall.
- ARM64 verifies deliberate same-version CPU replacement in both directions. Its AMD64 compatibility
  payload is not counted as native execution. AMD64 verifies rejection of the ARM64 MSI.
- Historical upgrade uses only `P4K-Extractor-0.2.0-x86_64.msi` and checks its MSI identity; ARM64
  does not use this AMD64-only predecessor.

Installer lifecycle scripts refuse non-disposable local execution and pre-existing P4KX ownership.
Tests seed synthetic `settings.toml`/`archive.key` markers only on a clean hosted runner. Runtime
processes use `P4KX_FOLDERS_ROOT`, so no test GUI writes into that marker profile or the user's data.
The local `test-windows-package-contract.ps1` builds synthetic, non-executable MSI payloads for
both CPUs and checks database/rollback-probe authoring without installing either MSI.

The target runtime artifact includes both package hashes and executed binary hashes. Final
assembly rejects missing, skipped or mismatched package evidence and requires the complete
[four-target release matrix](supported-platforms.md). Native hardware GPU, dialogs, clipboard and
audio evidence remains part of the final integration verification; hosted frame success does not
assert those capabilities. Native CI/device run URLs must be retained with the final release audit.

## Release integration

Contract fixtures, packaging stages, tools and runtime logs use a fresh `RUNNER_TEMP` task keyed
by run, attempt, target and producer/consumer role, outside the cached Cargo build directory.
Consumers require the complete producer matrix to succeed; missing artifacts still fail verification.

MSI inputs resolve to native absolute paths before metadata and lifecycle checks. Related products
are enumerated without a synthetic empty product, and a cleanup failure preserves any original
verification exception. The invocation regression uses synthetic paths and mocked COM enumeration;
it never installs software on the development machine.

Installer logs retain `original-path.json`, one `<transaction>.log.path.json` per completed MSI
operation and `final-path.json` after cleanup. These contain the user PATH's raw registry value
and kind plus the .NET user-scope result, distinguishing separator changes, variable expansion
and registry-type changes. Uninstall requires the exact original value and kind; comparison
never drops empty segments or normalises case, trailing separators or variable references.
`test-msi-path.ps1` exercises these distinctions without writing the registry or installing a
package. Actual preservation remains a native hosted-runner gate.

The separate prepend hypothesis uses `build-msi.ps1 -UserPathPart first`. Normal packages retain
the default `last` operation. Probe MSIs and their `.probe.json` bindings belong only to validation
artifacts; each binding requires the exact source revision, target, version, operation and MSI
SHA-256. The probe cannot satisfy release runtime evidence or change the released operation.

`check-msi-path-probe.ps1` runs the complete modern lifecycle against the host's exact original
PATH, plus eight install/uninstall cases: plain, leading, trailing and doubled separators in
`ExpandString`; a plain `String`; both kinds of explicit empty string; and an absent value. Each
case retains its own raw baseline and result. Failures do not skip the remaining cases. Ownership
cleanup must succeed before any separate fixture reset, and reset records do not count as installer
preservation. The writer is limited to the hosted runner's user `Environment\Path` value.

AMD64 first calibrates the exact historical 0.2.0 MSI with install/uninstall and records its observed
cleanup rest. The historical rename-upgrade case compares against that rest. Any difference between
the pre-legacy original and the calibration rest is separately reported as a legacy prior mutation;
it is never relabelled as preservation by the current MSI. The modern lifecycle still compares with
its own exact original baseline. Restoring a closed fixture between scenarios is a separately logged
test action, after all current and historical ownership has gone.
If calibration is unavailable or fails, its original failure remains in the report and final
aggregate; only the dependent historical upgrade is skipped. The modern lifecycle and all eight
raw-value cases still run after successful ownership cleanup and fixture reset. Unresolved
ownership aborts further mutation.

The invocation contract executes the run-all and cleanup paths with mocked registry/installer
operations. The packaging contract compiles both append and prepend Environment rows for both
CPUs, reads the MSI databases and rejects mismatched probe bindings without installing software.
These checks establish probe correctness; only the native hosted-runner results can confirm or
disprove the prepend hypothesis. Leading separators, empty values and registry kinds remain exact
requirements even when they disprove it.

`release-windows.yml` consumes the Windows rows of `.config/platforms.json`. Each native producer
builds three release applications and a next-version MSI, generates target-filtered CycloneDX
SBOMs including any Microsoft runtime files, and uploads `windows-build-<target>`. A separate
native consumer validates source/CPU/tool/digest metadata before executing portable/installed
payloads and emits `release-<target>` only after successful lifecycle verification.

Each release artifact contains two packages, three `P4KX-<version>-<target>-<binary>.cdx.json`
SBOMs, `P4KX-<version>-<target>-runtime.json` and a host-tool-role inventory. Host CycloneDX is
pinned 0.5.9 AMD64 and explicitly analyzes `--target <target>`, including on an ARM64 runner;
its compatibility execution is dependency analysis, never native app evidence. WiX is pinned
7.0.0. Native compiler and nextest pins come from the common setup.

The shared workflow scans every architecture-specific package, attests each target's packages
and metadata and assembles SHA-256/SHA3-512 sums. The Linux owner consumes `linux-matrix` from
the same prepare job and emits the same `release-<target>` interface. Publishing requires all
twelve packages, twelve SBOMs and four executed native proofs; a Windows-only run cannot publish
a partial matrix. PR runs prepare artifacts without publication.
