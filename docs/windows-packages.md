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
[five-target release matrix](supported-platforms.md). The release audit records hosted native
execution on both Windows CPUs and physical GPU, dialogs, clipboard and WASAPI playback on the
owner's Windows AMD64 desktop. [Windows AMD64 desktop verification](supported-platforms.md#windows-amd64-desktop-verification)

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

Release preflight verifies thirteen native packages, fifteen target/application SBOMs and five
native runtime records across Windows, Linux and macOS. For a tagged release, the workflow
verifies the scan result for each exact package, attests target packages and metadata, and
assembles SHA-256 and SHA3-512 checksums. Windows-only runs validate both native Windows targets
and upload package evidence. Publication uses the full
[release preflight](supported-platforms.md#release-preflight-and-evidence).

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
allowance is the smaller of ten minutes and the remaining deadline. Its parent drains the worker
output under the direct-process watchdog and five-second termination wait. The owned child
terminates and its captured logs are written before optional replay. The package verifier uses
the persisted journal without a console replay. Native process startup, termination and
filesystem availability can still affect when evidence reaches the runner. Existing inner
installer and fixture limits, source/CPU guards and uncertainty fences remain required.

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

The package consumer records the original Finish supervisor launch and its
exact digest. The hosted Wait shell observes the original supervisor receipt
under the existing collection cutoff and writes the request-bound
`wait-result.json` with the exact launch digest. CollectFinish verifies that
digest, the original receipt chain and the closed journal. The original terminal evidence must upload successfully
before package evidence and payload publication. The detached observer
controls exercise startup and timeout paths with harmless native workers.

On hosted ARM64, a best-effort upload preserves the original phase checkpoints
and caller results for phases 01 through 03 before the Finish supervisor starts.
This artifact contains the compact observation metadata. The upload has a
one-minute limit inside the existing job budget, and release acceptance still
requires the original terminal result and native runtime checks.

After launching the original Finish supervisor, the Windows consumer takes
one read-only checkpoint and uploads a separate immutable `-finish-live`
artifact. The checkpoint binds the original request, exact supervisor launch,
bounded phase journal and supervisor identity. It records supervisor and
original parent liveness, any observed receipt hashes and the allowlisted
runner image and version values when the host supplies them. Sampling and
upload each have a one-minute step limit. Required Wait and CollectFinish
validation retain their original allowance and proceed when optional
evidence is unavailable.

The native AMD64 checkpoint passively follows the current product uninstall
after the historical upgrade for up to 15 seconds within the original observer
allowance. The archived bound journal distinguishes `msiexec/wait` and its
outer `lifecycle/historical-uninstall` phase. Either can show a begin, an end,
an error or no observation. Wait and CollectFinish retain the original
deadline and release acceptance requirements.

The hosted Wait shell emits an entry notice before calling the canonical
receipt check. Its unwind notice reports whether the check returned or raised
an error. Received notices provide shell execution evidence. Runner or service
failure can leave either notice unavailable. Native success requires the
original bound receipts and CollectFinish.

The diagnostic-only `MSI feedback runner-fuse probe` workflow runs native AMD64 and ARM64
complete/timeout controls through the same phase writer. Its separate verifier downloads the
actual artifacts and checks the service's failed-step and timeout annotations, source revision,
run/attempt, CPU, original source hashes and phase closure. Only the expected timeout probe may
tolerate failure; the verifier and real installer jobs retain strict failure gating. The dynamic
parent-chain probe also checks both native CPUs with deliberately blocked replay. Its synthetic
witness requires process exit and captured logs before replay begins. Installer correctness
remains owned by the real MSI lifecycle. Both capture readers use one bounded byte reader and
strict UTF-8 decoding; only
a UTF-8 byte order mark is accepted. The journal keeps its 1-MiB bound and diagnostic JSON its
64-KiB bound. The diagnostic JSON reader requires PowerShell 7.5 or newer and preserves timestamp
strings with `ConvertFrom-Json -DateKind String`; original precision and offsets reach the strict
budget parser unchanged under every culture. This requirement belongs to the CI diagnostic
scripts; the released Rust applications do not require PowerShell. Their contract tests preserve
exact-limit inputs, incomplete journal rows and original writer output while rejecting invalid
UTF-8 and UTF-16 byte order marks.


### MSI package evidence

The AMD64 MSI from producer `0e1da3f4a` in Release run `43813424470` passed the
native read-only MSI metadata and transport assertions. Its SHA-256 is
`61f2ff3f48722db3da1e5cd2003ad1fb103977328a8573310ad2268fe47658ae`.
The embedded helper digest matched the corresponding `windows-build-x86_64-pc-windows-msvc`
payload. The local probe used the same metadata and helper-digest readers as the hosted
installer step. Installation, rollback and profile preservation require the separate hosted
lifecycle result. [Release producer](https://github.com/p4kx-org/p4kx/actions/runs/43813424470/job/133122281746)

The native ARM64 execution job for merge `867e6b9d9` passed the MSI lifecycle,
runtime and preservation checks. Its result is bound to that producer and architecture.
[Native ARM64 execution](https://github.com/p4kx-org/p4kx/actions/runs/43826004006/job/133175698510)

A read-only Windows check opened both exact AMD64 MSI inputs prepared by the
same run. Both SHA-256 hashes matched the diagnostic input receipt. The
v0.2.0 predecessor had template `x64;1033`, one expected per-user Registry
key and the same UpgradeCode as the produced MSI. The hosted historical
install, upgrade and removal remain the lifecycle checks for this pair.
[Earlier AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/43826004006/job/133175698571)

Release merge `bcd39e238` passed native AMD64 verification in run `43837672769`.
The phase journal records the legacy v0.2.0 calibration, rename upgrade, runtime
checks and uninstall. It also records the current install, repair, rollback,
upgrade, downgrade refusal, wrong-CPU refusal, PATH cases and cleanup with the
original state restored. The same job ran the portable CLI, MCP and bounded GUI
with isolated folders. [Native AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/43837672769/job/133219150212)

Release run `44226543671` on `b33c4cbc18fa844e93db3c3630073c9ecab8826d` passed
native Windows AMD64 and ARM64 package execution in the same run. Both original MSI journals
closed successfully. Each architecture produced six successful installed and portable runtime
receipts with matching PE architecture, CLI and MCP execution, and software-rendered GUI frames.
The run also checked repair, rollback, profile preservation and architecture-specific
package handling. [Native AMD64](https://github.com/p4kx-org/p4kx/actions/runs/44226543671/job/134560610488) ·
[Native ARM64](https://github.com/p4kx-org/p4kx/actions/runs/44226543671/job/134560610521)

At PR merge `5d3d917f424657990fd775b8fae8ac843a3fd528`, each native job uploaded
checkpoint 03 and the original Finish supervisor launch. AMD64 had completed the
predecessor installation and entered rename upgrade. ARM64 had completed the
current installation and entered the installed runtime checks. GitHub marked both
jobs cancelled around 55 minutes after they started, with checkpoint 04 still in
progress. The last available receipts establish the original phase boundary and
supervisor launch. The specific stalled operation and final installer result remain
unresolved.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45315878218/job/138200239367) ·
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45315878218/job/138200239372)

At PR merge `3dcb12e6cae223d2594ebac549b601d43a84e7ef`, the native ARM64 package
job passed in 4 minutes 23 seconds. It uploaded the Finish observations and watchdog
receipts for checkpoints 04 and 05, collected the terminal MSI lifecycle receipt and
recorded installed and portable runtime evidence. The AMD64 job verified its own package
digest. A connection reset while requesting the other package's signed download URL stopped
that job before MSI execution.
[Native ARM64](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/job/138241903869) ·
[Checkpoint 04](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/artifacts/14893021939) ·
[Checkpoint 05](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/artifacts/14894095084) ·
[Terminal MSI receipt](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/artifacts/14893786316) ·
[AMD64 download failure](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/job/138241903849)

The targeted AMD64 retry downloaded both inputs and preserved checkpoint 03 and the
Finish supervisor launch. Checkpoint 03 recorded a successful predecessor install and
an open rename-upgrade wait. GitHub still reported checkpoint 04's observation step
in progress after its one-minute step allowance and the original Finish collection
cutoff. The blocking operation remains unknown.
[AMD64 retry](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/job/138250078993) ·
[Checkpoint 03](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/artifacts/14893049565) ·
[Finish launch](https://github.com/p4kx-org/p4kx/actions/runs/45327821298/artifacts/14894530139)

Release merge `8b6032a9d` finished the native AMD64 job in 4 minutes 51 seconds.
The original terminal journal has 200 events, a completed worker with exit 0 and
no active event. The ARM64 job captured and published checkpoint 03 after the
first install. GitHub still reported that upload step as in progress after its
one-minute allowance and the original observer deadline. The next Finish launch
step remained pending.
[Native AMD64](https://github.com/p4kx-org/p4kx/actions/runs/45338631037/job/138277997319) ·
[Terminal AMD64 receipt](https://github.com/p4kx-org/p4kx/actions/runs/45338631037/artifacts/14898410480) ·
[Native ARM64](https://github.com/p4kx-org/p4kx/actions/runs/45338631037/job/138277997254) ·
[ARM64 checkpoint 03](https://github.com/p4kx-org/p4kx/actions/runs/45338631037/artifacts/14897991980)

Release run `45425645716` for PR head `1ea6a27fa` reached `CollectFinish` on native
AMD64 and `WaitFinish` on native ARM64. GitHub reported the 45-minute maximum and recorded
both jobs as cancelled about 55 minutes after their start. The terminal installer results were
unavailable. The operation responsible for the stall remains undetermined.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45425645716/job/138594799181) ·
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45425645716/job/138594799037)

Release run `45447246086` for PR head `e0cf389cf` passed both native Windows jobs.
AMD64 finished in 3 minutes 49 seconds and ARM64 in 4 minutes 42 seconds.
Both verified terminal Lifecycle receipts bind attempt 1 to the merge revision
`5b0f5f8b044f1df7cc1cc35500eb8acfd71c2984`. Both original workers exited with code 0,
with 200 journal events on AMD64 and 174 on ARM64. The native jobs recorded their
installed and portable execution evidence and uploaded both executed release payloads.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45447246086/job/138657917218) ·
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45447246086/job/138657917270) ·
[AMD64 terminal receipt](https://github.com/p4kx-org/p4kx/actions/runs/45447246086/artifacts/14949192760) ·
[ARM64 terminal receipt](https://github.com/p4kx-org/p4kx/actions/runs/45447246086/artifacts/14951115024)

Release run `45530796171` for PR head `ac572e896` bound both target builds to
merge revision `27da392284eed8c67cf58c15ea473089d6c0a20c`. The second ARM64
attempt passed the original installer lifecycle and uploaded the executed package.
AMD64 attempts two and three remained in `WaitFinish` until cancellation. GitHub
reported a 45-minute maximum and recorded both cancellations after 55 minutes.
The third attempt enabled runner diagnostics. Its ordinary job-log endpoint returned
HTTP 404, and collection did not produce a terminal MSI receipt. The blocking operation
remains undetermined.
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45530796171/job/138968074349) ·
[ARM64 terminal evidence](https://github.com/p4kx-org/p4kx/actions/runs/45530796171/artifacts/14990526867) ·
[AMD64 diagnostic job](https://github.com/p4kx-org/p4kx/actions/runs/45530796171/job/138990048543)

Release run `45557170432` for PR head `a8f48d4ea` started the original Finish
supervisor on both native Windows targets. Both jobs were cancelled with
`WaitFinish` still reported in progress. `CollectFinish` and executed-payload
recording remained pending. The ordinary job-log endpoint returned HTTP 404
(`BlobNotFound`) for each job after cancellation. The local synthetic Finish
test covered positive and negative receipts and expiration during probe setup.
The operation that blocked the hosted runs remains undetermined.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45557170432/job/139031674324) ·
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45557170432/job/139031674357)

Release run `45611267510`, attempt 1, checked merge
`7b08ee65fea2e0bd08ea8e244f2bbc14c17f5d7e` for signed PR head `a4723c395`.
The Windows ARM64 job completed `WaitFinish` in 87 seconds and collected the
successful original MSI result. It published the original Finish receipt and
the executed MSI and portable ZIP evidence.
[ARM64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45611267510/job/139217186721) ·
[Original Finish receipt](https://github.com/p4kx-org/p4kx/actions/runs/45611267510/artifacts/15023990683) ·
[Executed ARM64 packages](https://github.com/p4kx-org/p4kx/actions/runs/45611267510/artifacts/15023861004)

The native AMD64 job passed portable execution and launched the Finish
supervisor. GitHub marked `WaitFinish` cancelled after 41 seconds, before its
configured limit. The collection and executed-payload steps were skipped.
The ordinary logs were unavailable and the cancellation cause is undetermined.
[AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45611267510/job/139217186742)

Release run `45633605773`, attempt 1, checked merge
`e59c7410f6411302e8101f180751177dd0d6adfb` for signed PR head `fe054ccf9`.
Both Windows build jobs completed. AMD64 completed the original Finish,
collected its lifecycle receipt and published the executed packages.
[AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45633605773/job/139302848720)

On ARM64, the Build and Finish supervisor start succeeded. At the GitHub API
response time `2026-11-25T15:55:56Z`, the native job had been active for
46 minutes 13 seconds and the Finish wait for 43 minutes 20 seconds. Both
remained in progress after the configured deadlines. The original final
artifact and job log were unavailable at that observation. The underlying
cause remains unresolved.
[ARM64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45633605773/job/139302848822)

Release run `45650794879`, attempt 1, checked merge
`3ac8351b831f99a3b79f9d2b544e3531b42df258` for signed PR head `7ca457080`.
AMD64 completed the original lifecycle and published the executed packages.
On ARM64, the early original checkpoints verified the raw Registry capture,
the first install returning code 0, and entry into the installed runtime
checks. All three snapshot callers exited successfully.
[AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45650794879/job/139353629226)
[ARM64 phase checkpoints](https://github.com/p4kx-org/p4kx/actions/runs/45650794879/artifacts/15043871004)

At the GitHub API response time 2026-11-25 17:24:56 UTC, the original ARM64
job remained in progress after 45 minutes 26 seconds, with Finish waiting
for 41 minutes 58 seconds. GitHub concluded the job `cancelled` at 17:34:30 UTC,
55 minutes after it started. The PR head stayed unchanged and GitHub continued
to show Finish as in progress. Its Check Run annotation records that the job
exceeded its maximum execution time of 45 minutes. The job log returned HTTP 404 after cancellation,
and the final receipt remained absent. Subsequent original lifecycle phases
and the blocking operation remain unresolved.
[ARM64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45650794879/job/139353629186)

Packaged command checks share one monotonic allowance for the child process and
its redirected stdout and stderr. A Windows synthetic test holds an inherited
writer open after the command exits and verifies that collection returns a
timeout while the writer is still alive. It also verifies success output,
the combined UTF-8 log, the original nonzero exit code and process cleanup.
The package GUI check uses the same rule for its 120-second allowance. The
MCP check keeps the JSON-RPC response deadlines and bounds process exit and
stderr capture within ten seconds after closing stdin.

For release acceptance, repeat the native lifecycle checks with the actual release candidate
and bind both its MSI and portable ZIP to the candidate revision and run attempt. Record the
Windows AMD64 desktop and WASAPI checks separately from the hosted package evidence.

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

For PR #1117 head `a61eb3de5`, both independent verifier jobs passed. They confirmed
the native AMD64 and ARM64 runner annotations and original downloaded feedback for
the deliberate timeout cases alongside both complete controls and the chain's
inner-watchdog cases. The detached observer controls also passed on
both architectures in the corresponding Release workflow.
[Runner fuse verification](https://github.com/p4kx-org/p4kx/actions/runs/45576709279/job/139077396882) ·
[Dynamic chain verification](https://github.com/p4kx-org/p4kx/actions/runs/45576709270/job/139077413013) ·
[Detached observer controls](https://github.com/p4kx-org/p4kx/actions/runs/45576709479)

The matching AMD64 original consumer passed native MSI lifecycle and
portable/installed execution on merge `4d334e716cbc87e27525e8e095410a19103c51f5`.
The detached Finish caller returned in 99,910 ms and exited zero under
its supervisor. The original replay worker exited zero and completed all
200 journal events with no active phase. The final observer receipt
records native Lifecycle success. The candidate published its executed
Windows AMD64 payload.
[Native consumer](https://github.com/p4kx-org/p4kx/actions/runs/45576709479/job/139102930100) ·
[Original Finish evidence](https://github.com/p4kx-org/p4kx/actions/runs/45576709479/artifacts/15008347409) ·
[Executed payload](https://github.com/p4kx-org/p4kx/actions/runs/45576709479/artifacts/15008217762)

The matching ARM64 job was cancelled 55 minutes after GitHub started it. The last
reported step was WaitFinish. The native portable checks and Finish supervisor
launch completed. GitHub reported its 45-minute limit. Its job-log endpoint
returned HTTP 404 and the run produced no original terminal MSI receipt or
executed ARM64 payload. The last reported step alone cannot establish the
installer's outcome or the operation that stalled.
[Native ARM64 consumer](https://github.com/p4kx-org/p4kx/actions/runs/45576709479/job/139102930087)

For PR #1117 head fd3160449, both native consumers passed the bounded output tests with
inherited CLI and GUI writers and inherited MCP stderr. Both also completed the original
portable execution and Windows Installer lifecycle. WaitFinish took 75 seconds on AMD64 and
86 seconds on ARM64. The final observer and executed payload uploads succeeded for both.
The operation responsible for the earlier hosted cancellation remains unknown.
[AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45669055539/job/139426076930) ·
[ARM64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45669055539/job/139426076867)

For PR #1117 head `db162ecba`, both native consumers observed the initial
install outcome and uploaded their pre-Finish diagnostics. The last AMD64
checkpoint placed the original worker in the upgrade's `msiexec` wait. The
last ARM64 checkpoint placed it in installed runtime verification. GitHub
recorded cancellation after 55 minutes 1 second on both jobs and cited the
45-minute job limit. The recorded step remained WaitFinish. The job logs
returned HTTP 404, and final lifecycle receipts were absent.
[AMD64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45697517932/artifacts/15064902733) ·
[ARM64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45697517932/artifacts/15065830796) ·
[Release run](https://github.com/p4kx-org/p4kx/actions/runs/45697517932)

For PR #1117 head `36181deeb`, AMD64 completed the exact native lifecycle.
WaitFinish took 92 seconds. The job published the final original MSI receipt
and the executed portable and installer payloads. The ARM64 job retained the
initial install and pre-Finish checkpoint. GitHub cancelled it while the reported
step was WaitFinish. The recorded duration was 55 minutes and the job annotation
cited its 45-minute limit. The final receipt and executed ARM64 payload were absent.
[AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45714089604/job/139572874737) ·
[ARM64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45714089604/job/139572874813) ·
[ARM64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45714089604/artifacts/15072330016)

For PR #1117 head `a839b2adad`, AMD64 completed the original MSI lifecycle and
published its final receipt. The last available ARM64 checkpoint recorded 44 events
and placed its worker in installed runtime verification. GitHub cancelled the job
after 55 minutes and cited the 45-minute job limit. It still reported WaitFinish as
active. The job log and the final original ARM64 receipt were unavailable.
[AMD64 receipt](https://github.com/p4kx-org/p4kx/actions/runs/45747658169/artifacts/15085849753) ·
[ARM64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45747658169/artifacts/15086685851)

On head `f301ec7e8`, the ARM64 runner-fuse and dynamic parent-chain controls reached
their deliberately imposed 60-second step deadlines. Both uploaded diagnostics one
second later. This confirms that ordinary step timeouts can preserve evidence on
the hosted ARM64 runner.
[Runner-fuse job](https://github.com/p4kx-org/p4kx/actions/runs/45758595624/job/139703109461) ·
[Parent-chain job](https://github.com/p4kx-org/p4kx/actions/runs/45758595630/job/139703108446)

On PR #1117 head `194592a2a`, both native Windows controls verified the
WaitFinish entrypoint's refusal of stale and foreign launch receipts. The outer
host accepted a timely original observation and stopped only its owned child
in the separate timeout case.
The real ARM64 consumer also completed the original MSI lifecycle. Its outer host,
supervisor and Finish caller exited zero. Its terminal journal contains 174 events
and closes all active phases. The original terminal receipt and executed package
were published for merge `4e2e192e8733aef7300dd6f048807b92ac180e86`.
[Control and release run](https://github.com/p4kx-org/p4kx/actions/runs/45762183499) ·
[ARM64 terminal receipt](https://github.com/p4kx-org/p4kx/actions/runs/45762183499/artifacts/15092710377) ·
[ARM64 executed package](https://github.com/p4kx-org/p4kx/actions/runs/45762183499/artifacts/15091853493)

The real consumer records the prepared allowance, source hashes, checkpoints and parent controls
on the runner. It refreshes admission under the original deadline before launching the installer.
Before Finish starts, each native target uploads a checkpoint with the six allowed diagnostic
JSON receipts. The checkpoint records the last complete and validated phase row,
including its occurrence, parent, status and timestamp. A partial trailing line
does not replace that observation. The published JSON remains available if the
runner stops reporting during Finish. The workflow uploads the original Finish
launch binding, starts the read-only waiter in a separate bounded step and uploads its
readiness receipt before WaitFinish. The receipt binds the request, launch, collection
deadline and waiter and monitor process identities. It remains available after a later
job timeout if its upload completed.

The waiter polls the supervisor's atomic result using the original UTC cutoff and a
monotonic budget. Its phase markers and private logs help diagnose later failure.
WaitFinish verifies the bound waiter exit and wait receipt. If the terminal result is
absent or late, the wait receipt contains no result digest. Readiness and phase notices
establish progress only. CollectFinish still validates the cutoff, digest and original
native lifecycle evidence before publication.
The diagnostic runner-fuse fixture records whether GitHub retains the wait-start annotation
after a deliberate step timeout. Both observations leave native lifecycle completion unproven.
The hosted fuse run for `e2ec43eef` retained the `predicate-wait` start annotation
on AMD64 and ARM64 after the one-minute timeout. The completed controls contained
no such annotation. The independent verdict checked these cases against GitHub's
Check Run annotations. [Hosted timeout retention](https://github.com/p4kx-org/p4kx/actions/runs/45726436203)
The cancellation-aware diagnostic upload collects those local files after the terminal evidence
and package publication steps. A runner failure before the upload can prevent delivery.
The first install snapshot emits one bounded observation in its existing console stream after
validating the original phase journal. It reports the actual install phase and observed outcome
with a journal event count and terminal marker. The observation carries no lifecycle verdict.
`diagnostic-start.json` is written before worker launch, records the original worker source and
budget, and always declares completion unproven and no mutation authority. It is not evidence that
the child started, exited or left the Windows Installer service idle. All real installer cases,
45-minute job, 35-minute deadline and 600000-ms worker maximum remain required.

On Release run `45825321334`, merge `4c31057f8325235e54cb0305792560f78c7262e4`,
both Windows Builds and the native synthetic controls completed. The actual
AMD64 and ARM64 Installer jobs preserved their original pre-Finish checkpoints
and supervisor launch bindings. The last validated AMD64 phase was an active
`msiexec/wait` during legacy calibration. The ARM64 checkpoint was in
`runtime/installed`. Neither phase establishes a terminal outcome.

GitHub recorded both Windows execution jobs as cancelled at 14:47:02 UTC.
The recorded elapsed time was 55 minutes and the sole job annotation cited
the 45-minute limit. Both step records still showed `WaitFinish` in progress.
The hosted phase notices, final Finish receipts and Windows execution payloads
were absent. Individual job-log downloads returned BlobNotFound. The run-level
log archive also lacked the two Windows job logs. The published checkpoints
establish that the runner reported progress before Finish. The exact operation
where either runner stopped reporting remains unknown.

[Release run](https://github.com/p4kx-org/p4kx/actions/runs/45825321334) ·
[AMD64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45825321334/artifacts/15124615569) ·
[ARM64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45825321334/artifacts/15124860101)

Release run `45843748173` tested merge `2b4434893ea7092853f24eaf04950e8c3fedb9c8`.
The synthetic detached controls reached the supervisor's negative-result
fixture on both CPUs. Its failure injection still replaced the generic file
digest, while the supervisor called the bounded receipt digest. This left the
attempted caller-digest failure unexercised. The later harmless cleanup tried
to read the request from a skipped launch and reported a missing file.

The corrected synthetic checks inject the error at the bounded receipt reader
and verify the bound negative result and refusal to collect it. The harmless
cleanup requires an attempted launch. The original real Installer checks and
release evidence requirements are separate from these synthetic controls.

[AMD64 control log](https://github.com/p4kx-org/p4kx/actions/runs/45843748173/job/139992136364) ·
[ARM64 control log](https://github.com/p4kx-org/p4kx/actions/runs/45843748173/job/139992136557)

On the same run, both real Windows package consumers uploaded the pre-Finish
observations and the supervisor launch binding. At 16:04 UTC both remained in
WaitFinish beyond the collection deadline and the configured step limit. The
last captured AMD64 phase was `previous-install` waiting for `msiexec`. The ARM64
checkpoint had reached `runtime/installed`. The available evidence ends before
the blocking call or terminal installer outcome.

[AMD64 package job](https://github.com/p4kx-org/p4kx/actions/runs/45843748173/job/140018890789) ·
[ARM64 package job](https://github.com/p4kx-org/p4kx/actions/runs/45843748173/job/140018890780)

### Native ARM64 with detached Finish readiness

Windows-only run `45872703215`, attempt 1, used the packages built at
`bb2461a7754a042e331042a4ac78196b0041e5cc`. The native ARM64 consumer on Windows
11 build 26200 completed in 4 minutes 57 seconds. Its waiter published a bound
readiness receipt and exited zero before the original cutoff. CollectFinish
validated the original lifecycle. The journal closed all 174 events and recorded
one completed final cleanup. Install, repair, rollback, and profile preservation
passed. The MSI and portable ZIP both passed native CLI, MCP, and GUI frame
checks. Physical audio, clipboard, file-dialog, and hardware GPU checks remain
unobserved for this hosted ARM64 run.

The terminal evidence archive SHA-256 is
`08677dd39cc0656ce699857e2c35544ae2b4885ef1dd3ddc5c63dcb03d0a7f06`.
The executed package archive SHA-256 is
`74b0e6d8d71da44cdc54b4cc6d0a4323a1ded9e5f84694c64c2099029f0291f1`.
It contains the MSI with SHA-256
`5cb060db3d54ef7c744fc142b4a4ef2c1aa3feb99eec8b83555fda250e126d10`
and the portable ZIP with SHA-256
`a91c2de67eac754d98f04c5e5e2c57a5ddc057159d969baf76d0beaaf8f46d34`.
[ARM64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45872703215/job/140113609908) ·
[ARM64 terminal evidence](https://github.com/p4kx-org/p4kx/actions/runs/45872703215/artifacts/15145488138) ·
[ARM64 executed package](https://github.com/p4kx-org/p4kx/actions/runs/45872703215/artifacts/15146112691)

The separate AMD64 consumer uploaded its request-bound readiness receipt. At
19:06:49 UTC GitHub still reported WaitFinish in progress after the collection
cutoff at 19:06:01 UTC. The last validated original checkpoint had 63 events and
placed the worker in `previous-install` waiting for `msiexec`. The readiness
receipt establishes that the waiter bound to the original supervisor. The
public snapshot contains no waiter exit or terminal installer result.
GitHub cancelled the AMD64 job at 19:45:43 UTC after 55 minutes and recorded
"The job has exceeded the maximum execution time of 45m0s". WaitFinish still
showed in progress, and the CollectFinish and payload steps were pending. The
only AMD64 observer artifacts were the pre-Finish checkpoint, launch binding
and readiness receipt. The blocking operation and original installer outcome
remain unobserved.
[AMD64 execution](https://github.com/p4kx-org/p4kx/actions/runs/45872703215/job/140113609956) ·
[AMD64 readiness](https://github.com/p4kx-org/p4kx/actions/runs/45872703215/artifacts/15147036542)

### Native AMD64 diagnostic replay with live progress

The diagnostic job in run `45883443481`, attempt 1, used harness revision
`d809586729c72533289ae62e8d3bb01c836ceace` and the sealed native MSI packages
from production run `45872703215`, attempt 1, at revision
`bb2461a7754a042e331042a4ac78196b0041e5cc`. The replay verified each MSI and
helper hash and recorded the hash of the Registry fixture rebuilt from the
candidate revision.

The read-only checkpoint at 19:26:40 UTC recorded 129 events. The worker
had completed the previous install, rename upgrade, historical uninstall,
fresh install, feature no-op, and rollback-disabled phases. It was waiting
for `msiexec` in the repair phase, 35.43 seconds after worker start. The
request-bound supervisor, monitor, and waiter were all running 25.95 seconds
after readiness. The snapshot observer exited zero.

The diagnostic job succeeded in 3 minutes 28 seconds. Its Wait step took
86 seconds and accepted the readiness-bound zero-exit receipt before the
original cutoff. The job log retained the Wait invocation, and GitHub
records the successful step outcome.
The uploaded archives preserve readiness and the earlier live snapshot.
Production CollectFinish, final cleanup evidence, and package execution
remain separate checks on the original consumer.

The readiness archive SHA-256 is
`34bef927708b28925ff8c9809da809723cec143e67cc3c57a6395096514a14e5`.
The live archive SHA-256 is
`5e62891d43b87afe7686e29232e08f45ef547701389f0f8760b2c06cf9e73f77`.
[AMD64 diagnostic job](https://github.com/p4kx-org/p4kx/actions/runs/45883443481/job/140128182133) ·
[Readiness archive](https://github.com/p4kx-org/p4kx/actions/runs/45883443481/artifacts/15148063150) ·
[Live archive](https://github.com/p4kx-org/p4kx/actions/runs/45883443481/artifacts/15148846894)

### Native Windows lifecycle with a production live checkpoint

Windows-only run `45886906022`, attempt 1, exercised the native Windows
packages at `c5b1c79091a60d305b90f0018b2fd6c406fcb6e6`. The ARM64 consumer
on Windows 11 build 26200 completed in 4 minutes 47 seconds. Its production
live checkpoint captured 76 events at 20:32:41 UTC, 2.15 seconds after bound
readiness. The worker was in `runtime/repaired`, and the original parent,
supervisor, monitor and waiter were running.

The original Wait step completed in 68 seconds. CollectFinish verified the
same request, launch and readiness chain. The original journal closed all
174 events with one completed final cleanup. Both retained packages matched
their producer hashes and passed native CLI, MCP and GUI frame checks. The
software adapter, install, repair, rollback and profile preservation passed.
Physical audio, clipboard, file dialogs and hardware GPU remain unobserved.

The ARM64 live checkpoint archive SHA-256 is
`31b292129377e633b5c26f3fa8fafc268c016c0fa126352ee036a711a4112602`.
The terminal evidence archive SHA-256 is
`d5e6506477ab7bc3207a737347cc385fc3488b4b7e4d39477c293337533fffc5`.
The executed package archive SHA-256 is
`2c63a68a04e9076126829f93543cd178e417289d907ac2ccd3e7e64d3c2b3ba4`.
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45886906022/job/140154656751) ·
[ARM64 live checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45886906022/artifacts/15152405389) ·
[ARM64 terminal evidence](https://github.com/p4kx-org/p4kx/actions/runs/45886906022/artifacts/15151841991) ·
[ARM64 executed packages](https://github.com/p4kx-org/p4kx/actions/runs/45886906022/artifacts/15152181505)

The native AMD64 consumer retained its live checkpoint at 20:32:26 UTC, 3.28
seconds after verified readiness. At sequence 85, 22.09 seconds into the
worker, `msiexec/wait` was active in `lifecycle/historical-uninstall`. The
legacy installer and rename upgrade had completed. The active operation was
uninstalling the current product after that upgrade. The original parent,
supervisor, monitor and waiter were all running at the sample.

Wait began at 20:32:27 UTC. GitHub still showed the step in progress after
the 20:44:58 UTC collection cutoff and cancelled the job at 21:24:21 UTC,
55 minutes after its start. The sole annotation reported a 45-minute job
maximum. The job log was unavailable, and the run retained no AMD64 terminal
installer evidence or executed package. The sample establishes the last
observed phase. The blocking operation and MSI outcome remain unobserved.

The AMD64 live checkpoint archive SHA-256 is
`216c17b3f6f811dfe9dff33f1f1431bb221bb073878839942df039a08feca514`.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45886906022/job/140154656694) ·
[AMD64 live checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45886906022/artifacts/15151148864)

### Native Windows execution on e0a9bdd

Both native Windows package producers succeeded. GitHub cancelled both execution
jobs at 23:31:11 UTC. The recorded durations were 55 minutes on AMD64 and
55 minutes 1 second on ARM64. Each job's sole annotation cited its 45-minute
maximum, and the final job logs returned HTTP 404. The workflow retained no
terminal installer receipt or executed-package evidence for either architecture.
[Release workflow](https://github.com/p4kx-org/p4kx/actions/runs/45900458773)

The AMD64 job preserved its launch binding, waiter readiness and live checkpoint.
That checkpoint recorded completion of the historical uninstall at 17.745 seconds
into the worker and showed the worker in restore. GitHub's last step status was
the wait for the original Finish receipt. The checkpoint establishes progress
through the uninstall. The subsequent blocking operation and terminal MSI result
remain unobserved.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45900458773/job/140202197327) ·
[AMD64 live checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45900458773/artifacts/15157054910)

The ARM64 job preserved its before-Finish diagnostics and original launch binding.
The worker had reached the installed runtime phase. GitHub's last step status was
the bound waiter start, and the job preserved neither waiter readiness nor a live
checkpoint. The terminal MSI result and the reason the hosted step did not return
remain unobserved.
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45900458773/job/140202197290) ·
[ARM64 before-Finish evidence](https://github.com/p4kx-org/p4kx/actions/runs/45900458773/artifacts/15158350863) ·
[ARM64 launch evidence](https://github.com/p4kx-org/p4kx/actions/runs/45900458773/artifacts/15158053307)

### Native Windows execution on 2d26aa221

Windows-only run 45917796679, attempt 1, passed on both architectures at
revision 2d26aa221ef3b57d323b2ea4ebf72eab4fdf4206. Both jobs passed the
pre-installation mock checks, then completed the genuine installer lifecycle,
published its terminal evidence and recorded the executed packages. The
direct Wait step returned against the original launch binding, and
CollectFinish verified the bound receipt and original cleanup journal.
[Windows-only validation run](https://github.com/p4kx-org/p4kx/actions/runs/45917796679)

AMD64 completed in 4 minutes 54 seconds. Wait took 89 seconds and CollectFinish
took 1 second. The terminal evidence archive SHA-256 is
`563b5275b352d141ecab260ec553b689cc28aab29d6f0ba6b5917a6f7409448b`.
The executed package archive SHA-256 is
`1a7c86dd8e911fdc217fee7bfb26ef052c0b4aaa631256fe6cc022f60ddfec23`.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45917796679/job/140252990919) ·
[AMD64 terminal evidence](https://github.com/p4kx-org/p4kx/actions/runs/45917796679/artifacts/15164454873) ·
[AMD64 executed packages](https://github.com/p4kx-org/p4kx/actions/runs/45917796679/artifacts/15163714881)

ARM64 completed in 5 minutes 20 seconds. Wait took 85 seconds and CollectFinish
took 1 second. The terminal evidence archive SHA-256 is
`06659f451aa72e03879d256f2adbd43e988c6a91f610caf73e5382692a240d26`.
The executed package archive SHA-256 is
`e49acf0a79f4c022b9db2f42eba0f7e6b2de8fed6885629a609cb3ef12636df7`.
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45917796679/job/140252990942) ·
[ARM64 terminal evidence](https://github.com/p4kx-org/p4kx/actions/runs/45917796679/artifacts/15165045583) ·
[ARM64 executed packages](https://github.com/p4kx-org/p4kx/actions/runs/45917796679/artifacts/15165405065)

Physical audio, clipboard, file dialogs and hardware GPU require separate
desktop device evidence.

### Native Windows execution on 3b2c90ab

Release run `45963862816` built both Windows packages and passed the two
detached control jobs. Both original native consumers executed the portable
runtime checks and uploaded byte-verified lifecycle checkpoints. AMD64
completed the legacy install and uninstall, rename upgrade, installed
runtime and historical uninstall. The checkpoint ended during baseline
restore. ARM64 completed the fresh install, installed runtime and feature
no-op. Its checkpoint ended at msiexec/pre-capture in the rollback-disabled phase.

Both jobs started at 09:04:02 UTC and GitHub cancelled them at 09:59:03 UTC.
The only job annotation on each cited the 45-minute job limit. GitHub still
listed the original Finish wait as in progress, and the per-job logs returned
HTTP 404 after cancellation. The terminal MSI receipts and Windows execution
payloads were absent. The outcome of the original installer remains
unverified.

The successful Windows-only run above used the same Windows executor and
Finish observer code. Its AMD64 image, `20261116.315.1`, also matches this
run's AMD64 consumer. This run's ARM64 consumer used `20261123.225.2`, the
same image as its successful ARM64 detached control. The available evidence
does not locate the blocking operation.

[Release run](https://github.com/p4kx-org/p4kx/actions/runs/45963862816) ·
[AMD64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45963862816/artifacts/15184917035) ·
[ARM64 checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45963862816/artifacts/15184922365)

### Native Windows execution on db093cc92

Release run 45978787079 built both Windows packages from PR head
db093cc92f1b643e5c738b9e70a930900b5a9f7e and the tested merge
ba3805738b40bb9e9a63ee08aea2793004ea25ab.

AMD64 passed the complete native lifecycle. The original Finish wait took
96 seconds, and CollectFinish verified the original request and terminal
receipt. The job uploaded the terminal evidence and the executed package
with SHA-256 digests
`1f9ddc62d6e01abb07f41be2df5e4a88e79576c0d1b040dec3d119482f296261`
and `a264e62834f351dc583b63f04e1ed92d060c05eb8e6e18fdd65c22b903ca2b76`.
[AMD64 job](https://github.com/p4kx-org/p4kx/actions/runs/45978787079/job/140456588445) ·
[Terminal evidence](https://github.com/p4kx-org/p4kx/actions/runs/45978787079/artifacts/15191830708) ·
[Executed package](https://github.com/p4kx-org/p4kx/actions/runs/45978787079/artifacts/15191122308)

On ARM64, the original worker reached the installed runtime phase at
10:47:01 UTC. GitHub marked the passive observation cancelled at 10:47:07
UTC after two seconds and skipped Wait and CollectFinish. GitHub ended the
job at 11:38:40 UTC, 55 minutes after it started. Its sole annotation
reported the 45-minute job limit. The final job log returned HTTP 404,
and the run retained no terminal receipt or executed ARM64 package. The
cause of the early observation cancellation and the original installer
outcome remain unknown.
[ARM64 job](https://github.com/p4kx-org/p4kx/actions/runs/45978787079/job/140456588362) ·
[Before-Finish evidence](https://github.com/p4kx-org/p4kx/actions/runs/45978787079/artifacts/15190712078) ·
[Launch evidence](https://github.com/p4kx-org/p4kx/actions/runs/45978787079/artifacts/15191555821)

### Native Windows checkpoints on 8a58c8d2f

Release run 45993359530 built both packages from PR head
`8a58c8d2ff2cabe73c8e5daf6e07f47a165a2956` and tested merge
`127366f76ca889b4d90b380a613d37a90c872cd5`. The two post-launch
checkpoints bind the original requests to native processes on each CPU.

- On AMD64, the journal closed the historical uninstall at sequence 89 and
  reached baseline restore at sequence 90, 22.259 seconds into the worker.
  The live archive SHA-256 is
  `9f0a8471481500864dce9d7e21c7ebc5ef20248b1351550eb5447f180b2f8888`.
  [Job](https://github.com/p4kx-org/p4kx/actions/runs/45993359530/job/140509389808) ·
  [Checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45993359530/artifacts/15197752889)
- On ARM64, the journal closed one native `msiexec/wait` and entered
  `msiexec/post-capture` at sequence 53, 11.955 seconds into the worker.
  The live archive SHA-256 is
  `ee4d4680f1a523b5b237fcce568814993df8abada527eb66dccfafa742a1db42`.
  [Job](https://github.com/p4kx-org/p4kx/actions/runs/45993359530/job/140509389802) ·
  [Checkpoint](https://github.com/p4kx-org/p4kx/actions/runs/45993359530/artifacts/15198485377)

At 12:53:44 UTC, both jobs still showed their original active step beyond
the collection cutoff. The ARM64 artifact was already available at 12:40:37
UTC while GitHub still reported the checkpoint upload in progress. AMD64
still reported the original Finish wait. Both checkpoints record lifecycle
progress.

Both jobs started at 12:37:10 UTC and GitHub cancelled both at 13:32:10 UTC.
The sole annotation for each job cited its 45-minute execution limit.
GitHub still listed the steps above as in progress. The per-job log
endpoints returned HTTP 404, and the run retained no original Finish
receipt or executed Windows release payload. The installer outcome and
the cause of the hosted stall remain unverified.

Local Windows AMD64 synthetic Finish checks passed the original launch
and receipt binding, timeout and negative-receipt cases, and the bounded
watchdog cleanup. They used harmless workers and a synthetic runner
identity.
