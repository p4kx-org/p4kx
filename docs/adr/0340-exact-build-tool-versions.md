# ADR-0340: Exact build-tool versions

## Decision

The repository selects its exact stable compiler through `rust-toolchain.toml` and its exact
cargo-nextest release through `.config/nextest-version`. Rustup honours the compiler pin locally.
The minimal `install-rust` composite action reads these files, installs the selected releases and
compares the reported versions with the pins. Build jobs, nextest archive consumers, package jobs
and SBOM generation use that action; the build-setup action delegates installation to it.

The compiler pin satisfies the workspace minimum Rust version. The nextest installer selects the
native host asset rather than encoding an AMD64 download. Per-process test isolation is retained.

`cargo xtask toolchain`, also included in the default checks, rejects floating or malformed pins,
a compiler older than the minimum, direct workflow installer bypasses and missing version checks.
Exact tool releases do not imply immutable runner images or reproducible binary output.

## Updating

Update the compiler channel and nextest pin deliberately, retaining full patch versions. Run the
configuration checks, version verification, workspace tests, doctests, clippy and repository
checks on the supported native targets. Nextest archive producers and consumers use the same
repository revision and pins. Target routing, target-specific cache/archive isolation and native
runtime evidence belong to the shared platform contract, rather than another matrix here.

## Sources

- [Rustup toolchain files](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file)
- [Nextest release 0.9.151](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.151)
- [Nextest minimum-version settings](https://nexte.st/docs/configuration/minimum-versions/)
