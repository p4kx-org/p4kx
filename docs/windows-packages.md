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
A private Rust helper preserves that entry's ownership and the original PATH existence,
Registry string type and bytes. A unique pre-existing matching entry is borrowed rather
than removed on uninstall; ambiguous duplicates refuse the change. Foreign PATH edits
are retained. [ADR-0354](adr/0354-msi-path-ownership-and-bounded-environment-notification.md)
defines the transaction, invocation binding and bounded Environment notification.
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
- MSI lifecycle verifies machine/template/component metadata, install, repair, upgrade, blocked downgrade,
  transactional rollback with restored product/file hashes/PATH, profile/key preservation and uninstall.
- ARM64 verifies deliberate same-version CPU replacement in both directions. Its AMD64 compatibility
  payload is not counted as native execution. AMD64 verifies rejection of the ARM64 MSI.
- Historical upgrade uses only `P4K-Extractor-0.2.0-x86_64.msi` and checks its MSI identity; ARM64
  does not use this AMD64-only predecessor.

Installer lifecycle scripts refuse non-disposable local execution and pre-existing P4KX ownership.
Tests seed synthetic `settings.toml`/`archive.key` markers only on a clean hosted runner. Runtime
processes use `P4KX_FOLDERS_ROOT`, so no test GUI writes into that marker profile or the user's data.
The producer's `test-windows-package-contract.ps1` builds synthetic, non-executable MSI payloads
for both CPUs and checks database/rollback-probe authoring without installing either MSI.

The target runtime artifact includes both package hashes and executed binary hashes. Final
assembly rejects missing, skipped or mismatched package evidence and requires the complete
[five-target release matrix](supported-platforms.md). Native hardware GPU, dialogs, clipboard and
audio evidence remains part of the final integration verification; hosted frame success does not
assert those capabilities. Native CI/device run URLs must be retained with the final release audit.

## Release integration

The private [raw Registry fixture](msi-native-fixture.md) is an opt-in verification example,
separate from all application and installer payloads. Its raw observations and fixture resets
cannot satisfy installer-preservation evidence by themselves.

Contract fixtures, packaging stages, tools and runtime logs use a fresh `RUNNER_TEMP` task keyed
by run, attempt, target and producer/consumer role, outside the cached Cargo build directory.
Consumers require the complete producer matrix to succeed; missing artifacts still fail verification.

MSI inputs resolve to native absolute paths before metadata and lifecycle checks. Related products
are enumerated without a synthetic empty product, and a cleanup failure preserves any original
verification exception. The invocation regression uses synthetic paths and mocked COM enumeration;
it never installs software on the development machine.

Installer logs retain `original-path.json`, one `<transaction>.log.path.json` per completed MSI
operation and `final-path.json` after cleanup for managed PATH observations. The raw Registry
fixture separately records exact value existence, Registry type and bytes, including termination,
plus the production-decoded ownership and journal state. Managed strings alone cannot prove raw
preservation. `installer.json` includes a mandatory `path_contract` bound to the source, CPU,
current MSI hash and helper/fixture hashes. Missing or failed evidence blocks release assembly.

`check-msi.ps1` runs the modern lifecycle against the host's exact original PATH, then eight
install/uninstall cases: plain, leading, trailing and doubled separators in `ExpandString`;
a plain `String`; both string kinds explicitly empty; and an absent value. Each case proves
its seeded baseline, active installed prefix and exact post-uninstall value. The prefix includes
the helper's actual trailing backslash. Comparisons do not normalise empty segments, case,
variable references or terminator bytes. Additional raw encodings have pure codec coverage;
they are not claimed as extra executed native cases.

Ownership cleanup must succeed before any separate fixture reset. Reset evidence does not count
as installer preservation. A failure permits later cases only after proved cleanup. Unconfirmed
process completion stops every further helper, seed and restore mutation; killing an MSI client
does not prove that the Windows Installer server has stopped its transaction. A completed nonzero
exit preserves the original failure while permitting the existing ownership-aware cleanup.

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

One source-bound native helper probe also reserves an identity, seals the resolved intent, prepares and applies a real insertion, adds
a controlled foreign prefix, and rolls back with that exact captured binding. The foreign edit
must survive. The MSI lifecycle itself must execute the production notifier. A separate C#
message receiver is optional diagnostic infrastructure; neither individual-window delivery nor
hung-recipient behavior is inferred from lifecycle success.

Pure tests cover invocation uncertainty, raw observations, cleanup, evidence rejection and helper
provenance. The compiled package contract inspects the actual MSI action/search/sequence tables,
real SummaryInformation PackageCode and embedded helper digest. The helper is an embedded Type-2
EXE with a static CRT, independent of installed applications or DLLs. Its private SBOM, target,
imports, compiler, lockfile and notice hashes are retained inside the MSI. It adds no application
to the portable ZIP and no .NET runtime dependency. Native hosted execution remains required.

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

MSI transport keeps its single execute AppSearch before CostInitialize. Early Reserve carries only
ProductCode, actual PackageCode and a fresh nonce; after costing and InstallValidate, Seal binds
one immutable goal and resolved path before InstallInitialize. All deferred and cleanup callbacks
use the same captured token. A genuine maintenance component noop cancels Neutral; disabled rollback
is refused before Seal. A blocked downgrade does not reserve or mutate helper metadata. The native
consumer validates those paths as part of the existing per-CPU lifecycle, not through a separate test framework.

## Native lifecycle diagnostics

`check-msi.ps1` runs its existing lifecycle in one hidden native PowerShell worker, preserving
both the original PowerShell location and the separate native process directory. The first
owned workflow step establishes a 35-minute deadline inside the 45-minute job. The worker's
allowance is the smaller of ten minutes and the remaining deadline. Its parent uses the existing
direct-process watchdog and five-second termination wait; process startup, synchronous console
or file IO and callbacks are not a hard elapsed-time guarantee. Existing inner installer and
fixture limits, source/CPU guards and uncertainty fences remain required.

An independent runner limit also surrounds the installer step. Its allowance is the smaller of
12 minutes and the remaining whole minutes of the original deadline. Preparation and admission
reject expired, inflated or stale allowances before platform or installer work. The original
worker deadline remains in effect. `installer-check/feedback/step-feedback.json` records the
prepared allowance and source/run/CPU binding separately from the worker's diagnostic directory.
The cancellation-aware diagnostic upload has its own five-minute step limit. Runner or service
failure can still prevent delivery; retained partial evidence leaves native completion unproven.

`installer-check/diagnostics` retains source/input identities, bounded UTC-millisecond phase
rows in `phase-journal.jsonl`, captured worker output and `diagnostic-result.json`. Phase rows
identify actual preamble, lifecycle, runtime, restore and cleanup boundaries. A completed
operation and a failed cleanup have distinct terminal statuses. Diagnostic persistence failures
preserve the original operation failure and do not bypass required profile cleanup.

A timeout or incomplete journal leaves native completion unproven. The parent does not uninstall,
restore Registry values, kill a process tree or infer that the Windows Installer service has
finished. Diagnostics cannot replace the source-bound native runtime and exact-PATH release
proofs. The infrastructure tests execute synthetic PowerShell children and mocked native
adapters; they are not a reproduction of an installer timeout.

The diagnostic-only `MSI feedback runner-fuse probe` workflow runs native AMD64 and ARM64
complete/timeout controls through the same phase writer. Its separate verifier downloads the
actual artifacts and checks the service's failed-step and timeout annotations, source revision,
run/attempt, CPU, original source hashes and phase closure. Only the expected timeout probe may
tolerate failure; the verifier and real installer jobs retain strict failure gating. Successful
probe evidence establishes feedback transport. Installer correctness remains owned by the real
MSI lifecycle. Both capture readers use one bounded byte reader and strict UTF-8 decoding; only
a UTF-8 byte order mark is accepted. The journal keeps its 1-MiB bound and diagnostic JSON its
64-KiB bound. The diagnostic JSON reader requires PowerShell 7.5 or newer and preserves timestamp
strings with `ConvertFrom-Json -DateKind String`; original precision and offsets reach the strict
budget parser unchanged under every culture. This requirement belongs to the CI diagnostic
scripts; the released Rust applications do not require PowerShell. Their contract tests preserve
exact-limit inputs, incomplete journal rows and original writer output while rejecting invalid
UTF-8 and UTF-16 byte order marks.


### Dynamic parent-chain feedback experiment

The separate `MSI dynamic parent-chain probe` workflow exercises the actual preparation output,
`fromJSON` step limit, unchanged admission and hidden parent/Portal path on native AMD64 and ARM64.
It runs no installer or Registry adapter. Its complete, inner-watchdog and outer-runner cases are
separate from the literal runner-fuse probe and from real MSI release acceptance.

Each case prepares a 90-second absolute fixture deadline, yielding one whole minute for the
runner step. The original prepared receipt and exact source hashes upload before the risky step.
Admission still rejects a preparation made stale by upload latency; a refused case cannot count
as a successful timeout experiment. The complete and outer cases retain that original deadline.
Only the inner fixture subcase explicitly uses a new three-second child deadline after admission,
records both deadlines and restores the original environment; the production budget helper and
`check-msi.ps1` have no override. The outer fixture waits longer than the admitted runner step and
requires the parent-start receipt to prove an inner budget greater than one minute.

The independent verifier requires each successful control, the exact one-minute service annotation
for the failed dynamic outer step, original early/late receipt equality, source/run/revision/CPU
bindings, valid admission and original phase closure or active scopes. A job-level timeout, stale
admission, absent active phase or invented start-receipt success is refused. It also rejects targeted
mutations of the actual downloaded artifacts. Any outer interruption leaves native completion
unproven, whether a final parent result exists or not. The fixture can establish cancellation and
feedback behavior; it cannot establish the cause of an MSI timeout or permit external cleanup.

The real consumer preserves the actual prepared allowance, source hashes and already executed
parent-contract artifacts in a one-minute bounded upload after preparation and before installer
work. That upload does not refresh the original deadline or allowance; admission remains the next
installer entry guard. The later cancellation-aware capture also includes the parent controls.
`diagnostic-start.json` is written before worker launch, records the original worker source and
budget, and always declares completion unproven and no mutation authority. It is not evidence that
the child started, exited or left the Windows Installer service idle. All real installer cases,
45-minute job, 35-minute deadline and 600000-ms worker maximum remain required.
