# Disposable MSI Registry fixture

The private `p4kx-msi-path` crate provides the opt-in Cargo example
`p4kx-msi-registry-fixture` for native Windows package verification. It captures the fixed
Registry values used by the PATH ownership protocol without decoding or normalising PATH.
It is verification infrastructure and is excluded from application and installer payloads.

## Build and execution boundary

The `native-fixtures` feature is disabled by default. Build the example separately on each
native Windows runner, after the common platform contract has checked the actual host and
target architecture. Set `P4KX_FIXTURE_SOURCE_REVISION` to the checked-out `GITHUB_SHA` before
compilation. Runtime revision must match that compiled value.

Execution requires `GITHUB_ACTIONS=true`, `RUNNER_ENVIRONMENT=github-hosted`, Windows,
matching process/runner architecture, canonical positive run and attempt identifiers, and
`P4KX_NATIVE_REGISTRY_FIXTURE=p4kx-native-registry-fixture:<run>:<attempt>`. Its dedicated
existing directory must be strictly beneath canonical `RUNNER_TEMP`, with no reparse-point
ancestors. Capture and mutation entry points recheck this boundary. These environment checks
prevent accidental developer-machine use; independent workflow and native-host evidence
establish provenance.

The example is never run on a developer machine. Pure tests can run there:

```powershell
cargo nextest run --locked -p p4kx-msi-path --lib --features native-fixtures
cargo test --locked -p p4kx-msi-path --example p4kx-msi-registry-fixture --features native-fixtures
```

Native CI checks the opt-in feature with Clippy on all four targets and runs these pure tests
on both Windows CPUs after verifying application machine types. These steps never call the
Registry fixture commands and do not establish native transaction or installer behavior.

## Private helper process status

The Windows helper preserves the operation's original success or failure and error output.
Success returns 0; a failure without an observed notification failure returns 1. Notification
diagnostics use the same callback result, without another send or a change to rollback policy.

| Status | Observed notification result |
| --- | --- |
| `0x20000000..=0x3fffffff` | Exact native error `status XOR 0x20000000`, including native zero |
| `0x60000001` | Invalid clock |
| `0x60000002` | Current-thread recipient |
| `0x60000003` | Unrecognised failure kind |
| `0x60000004` | Native error outside the exact payload range; no truncated value is reported |
| `0x60000101` | Complete enumeration with a timeout |
| `0x60000102` / `0x60000103` | Deadline without / with a timeout |
| `0x60000104` / `0x60000105` | Window limit without / with a timeout |
| `0x60000106` / `0x60000107` | Unrecognised enumeration stop without / with a timeout |

Decode only the helper's actual process status in the MSI log, such as
`CustomAction P4kxPathApply returned actual error code ...`. MSI's translated
`Action ended ... Return value 3` is a separate value. Native package evidence must establish
that the log retains the full status. A native error does not identify its originating window
or native call. The status does not encode delivery counts or out-of-range native values;
the original typed error remains intact.

## Commands and evidence

Arguments are separate command-line values. Every evidence argument is one canonical lowercase
`.json` filename beneath the guarded directory; paths, streams and device names are rejected.

| Command | Arguments after the command | Result |
| --- | --- | --- |
| `capture` | root, output | Exact current raw image and decoded ownership metadata. |
| `seed-path` | root, expected, seed, output | Compare the full expected image, then change PATH only. |
| `restore` | root, expected, original, output | Compare the full expected image, then restore the original fixed image. |
| `inspect` | root, input, output | Validate and decode retained evidence without reading the Registry. |

Seed schema 1 contains `path: null` for absence or
`path: { registry_type: 1, bytes_hex: "" }` for a present zero-byte REG_SZ value.
Type 2 is REG_EXPAND_SZ. Lowercase even-length hex retains every byte, including missing or
repeated terminators and invalid UTF16. No string expansion or managed string setter is used.

Capture schema 1 binds role, source revision, run, attempt, process architecture and root.
`image.keys` records named `environment`, `installer`, `ownership` and `invocation` existence.
`image.path`, `state`, `sequence` and `binding` are null or raw type/hex pairs. `core` separately
reports empty, rejected or valid metadata using the production WAL decoder; valid metadata
does not assert that malformed PATH bytes are usable. Mutation inputs must reproduce their
derived core view and match the current invocation headers.

The fixed scope is HKCU's 64-bit Environment\Path, PathOwnership\StateV1 and Sequence,
and PathOwnership\Invocation\Binding under Software\P4KX-Installer. Unrelated values,
ACLs, key classes and timestamps are outside the image contract. Restoration deletes an
originally absent key only when it is empty; foreign contents refuse the whole transaction.
Every mutation compares and reads back the exact fixed image in one safe winreg transaction.
Evidence uses exclusive file creation before mutation. A failed evidence write after commit
remains a failure and requires a fresh capture before restoration.

JSON is bounded to 40 MiB before parsing. PATH, WAL, Sequence and Binding slots retain their
own bounds. Native read metadata and post-read checks do not prove a hard peak-allocation
bound against concurrent Registry edits. Actual transaction conflicts, raw-byte preservation,
installer ownership cleanup and supported recovery behavior require disposable AMD64 and ARM64
evidence. The native MSI lifecycle must also execute the production notification path. Fixture
restoration is never proof of installer preservation; these observations do not establish delivery
to individual or hung windows. A separate message receiver is optional diagnostic infrastructure.

The existing native Windows package consumer compiles this example with its source revision and
requires its PE machine and executable hash before use. Each actual MSI operation captures raw
before/after images. Eight install/uninstall cases prove seeded, actively installed and restored
PATH values; a separate helper probe verifies foreign-edit retention across a bound rollback.
`installer.json` retains these observations as the mandatory source/MSI-bound `path_contract`.
Unconfirmed process completion latches the fixture against further helper, seed or restore
mutations. Read-only capture cannot clear that latch or authorize recovery.

An active WAL4 reservation also reports `core.reservation_phase` as `neutral` or `sealed`;
otherwise it is null. `reserved_binding` has the same canonical version-1 spelling in both phases.
Neutral has no receipt or inferred folder. The fixture uses the production WAL4 decoder and
rejects incompatible WAL3 rather than treating it as a neutral reservation. The opt-in fixture and
its consumers are source-bound; this additive field is not independent permission to mutate Registry state.
