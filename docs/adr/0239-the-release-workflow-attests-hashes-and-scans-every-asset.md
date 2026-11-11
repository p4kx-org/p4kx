# The Release workflow attests, hashes and scans every asset

Decided for issue #822.

## Decision

- The Release workflow runs in separate jobs with the least permission each: `package` and `sbom`
  build, `scan` uploads to VirusTotal, `attest` holds `id-token: write` and `attestations: write`,
  `assemble` computes hashes and notes, and only `publish` holds `contents: write`.
- Every action is pinned to the full commit SHA of its newest stable release, with the version in a
  trailing comment. Checkout does not persist credentials. The tag must match
  `^v\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$`. actionlint and zizmor run in `workflow-lint.yml`.
- The CycloneDX SBOMs are generated per shipped binary with `cargo-cyclonedx` 0.5.9, because the
  tool writes one document per binary and no merge step is trusted. The workflow runs the
  project's own Windows release binary, checked against a pinned SHA-256, instead of compiling it:
  the build is faster, and the tool's own lock file pins a yanked `xml-rs` that `cargo install
  --locked` warns about. The Blender add-on ZIP has no SBOM: it bundles no third-party packages.
- `actions/attest` attests the ZIPs, the MSI and the SBOMs in one statement; `actions/attest-sbom`
  attests each SBOM against the portable ZIP and the MSI. The Sigstore bundles are Release assets,
  named `P4KX-<version>-provenance.sigstore.json` and `<sbom>.cdx.json.sigstore.json`, so a
  download verifies offline with `gh attestation verify <file> --bundle <bundle>`.
- `SHA256SUMS.txt` and `SHA3-512SUMS.txt` list every other asset, bundles and SBOMs included. The
  `.txt` extension lets users open them with a double click. SHA3-512 is
  computed with .NET on an Ubuntu runner, whose OpenSSL supports it.
- The VirusTotal scan covers the MSI and the portable ZIP, never runs on a pull request, and never
  blocks the original Release workflow; the notes state when it did not run, also when the script
  crashes, times out or the job fails. The v0.5.0 tag contract below supersedes that policy. Its worst case (two files, 5 minutes upload and 10 minutes polling each) stays
  below the 55-minute step timeout, and messages that reach the public notes are sanitised and
  capped.
- The `rust-cache` step of the package job carries `zizmor: ignore[cache-poisoning]`. This is acceptable because
  `lookup-only` is true on tags: a Release build never restores a cache and never saves one, so
  a cache poisoned from a pull request cannot reach a published asset.
- Release notes: the `CHANGELOG.md` section, the scan result, the verification commands, then
  GitHub's generated "What's Changed" list.

## Consequences

- A pull request that touches the workflow or its scripts builds and assembles the package matrix.
  Validation notes record that the run skipped Sigstore attestations. Tag notes include attestation
  verification commands after all attestation jobs succeed.

## v0.5.0 tag publication

The v0.5.0 acceptance criteria in #1116, #1114 and #1022 require a completed VirusTotal
report for the exact SHA-256 of each native package in the four-target matrix. Tag assembly
requires successful scanning and checks that every required package matches the structured
report receipt from the same version, revision and workflow run. An incomplete scan prevents
publication. Pull requests and operating-system-specific dry runs skip the external scan.
Full manual branch runs retain best-effort diagnostics. The architecture-neutral add-on
keeps its separate provenance attestation.
