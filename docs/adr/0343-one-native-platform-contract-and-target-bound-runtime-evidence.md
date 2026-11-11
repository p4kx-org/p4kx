# ADR-0343: One native platform contract and target-bound runtime evidence

The [v0.5.0 macOS specification](../macos-support.md) extends this decision with an Apple
Silicon target, Mach-O evidence and the corresponding package, SBOM and runtime requirements.

## Decision

`.config/platforms.json` declares exactly four native Windows/Linux AMD64/ARM64 targets and the
required package formats. CI routing, package names and update asset selection consume this
contract. OS, binary architecture and package kind have distinct typed representations in the
update library; installer and distribution-specific architecture spellings remain source data.

The compiler is selected with its full native host suffix while retaining the exact compiler
release pin. Build/test artifacts and cache identities include the Rust target. Consumers compare
the native archive's recorded target, operating system and architecture with the selected platform
row. They refuse missing or conflicting platform fields and source, compiler, nextest or digest
mismatches before running tests. PE/ELF machine inspection establishes binary identity. Native
CLI/MCP/GUI smoke steps establish executed behavior.

Runtime evidence records actual successful GitHub step outcomes. Missing or skipped required
smoke cases cannot produce a successful artifact. Its scope is native synthetic verification;
physical device capabilities remain explicit and the final integration gate owns their evidence.

Packages provide a bounded identity sidecar beside real executables. Update selection uses the
compiled binary target and package kind, with a Release-page fallback when identity or assets
are missing, malformed or inconsistent. Release pages and package links are separate fields.
No asset is fetched or installed by selection, and no architecture/profile/Archive data is sent.

Release assembly refuses an incomplete twelve-package, twelve-SBOM, four-runtime-proof matrix.
SBOM target/application identity is checked inside the document, not inferred solely from its
filename. Licence generation reads all four target triples from the same contract.

## Native tool audit

The pinned Rust action accepts full host toolchain specifications. The pinned nextest installer
passes `RUNNER_ARCH` to its installer and handles Windows ARM64 runner startup. The pinned mold
action selects the native `uname -m` download. Windows uses target-specific rust-lld variables
and the existing ReFS build optimization. Native setup/build/tests remain required on every
row; a helper incompatibility is a failed check, not permission to drop ARM64.

The architecture-neutral add-on test lane may use available bpy wheels without defining native
P4KX runtime support. Hosted synthetic evidence does not substitute for required native device
verification or locally traced real-Archive export verification.

## Sources

- [GitHub native hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Pinned Rust toolchain action](https://github.com/dtolnay/rust-toolchain/blob/b725eda525362e7cf34af9a7243cac1e3f01a7ec/action.yml)
- [Pinned native installer action](https://github.com/taiki-e/install-action/blob/5488ac5a951645ce95062214902ac4b6bc1a1037/action.yml)
- [Pinned mold action](https://github.com/rui314/setup-mold/blob/339e96b4ff1f3bd865856ba4cbbe8818a7acb038/action.yml)
- [cargo-about 0.9.2 multiple-target CLI](https://github.com/EmbarkStudios/cargo-about/blob/0.9.2/src/cargo-about/generate.rs)
