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
