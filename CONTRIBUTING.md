# Contributing to P4KX

Thanks for taking the time to help improve P4KX, the tool that opens, browses, compares and
extracts the `Data.p4k` Archives of Star Citizen. This document explains how to ask questions,
report bugs, propose features and send pull requests, and the rules every change is held to.

The short version:

1. **Security issues never go into public issues**; use the confidential channel in
   [`.github/SECURITY.md`](.github/SECURITY.md).
2. **Bugs, features and tasks go through the
   [issue templates](https://github.com/p4kx-org/p4kx/issues/new/choose).**
3. **Never commit or attach game files, extracted game assets or the Archive key.**
4. **Pull requests branch from `main` and target `main`**, and pass `cargo fmt`, `cargo clippy`,
   `cargo xtask` and the tests locally before they are pushed.
5. **Every user-visible change adds a changelog fragment** under
   [`changelog.d/`](changelog.d/README.md) instead of editing [`CHANGELOG.md`](CHANGELOG.md).
6. **Sign the [Contributor License Agreement](CLA.md) once** before your first pull request, and
   **sign off every commit** ([DCO](#developer-certificate-of-origin-dco)): `git commit -s`.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Project documentation map](#project-documentation-map)
- [Asking questions](#asking-questions)
- [Reporting a security vulnerability](#reporting-a-security-vulnerability)
- [Reporting bugs](#reporting-bugs)
- [Suggesting features](#suggesting-features)
- [Game data and the Archive key](#game-data-and-the-archive-key)
- [Local development setup](#local-development-setup)
- [Pull requests](#pull-requests)
- [Contributor License Agreement (CLA)](#contributor-license-agreement-cla)
- [Developer Certificate of Origin (DCO)](#developer-certificate-of-origin-dco)
- [Commit messages](#commit-messages)
- [Style guides](#style-guides)
- [Before-you-push checklist](#before-you-push-checklist)
- [Releasing](#releasing)
- [Licence](#licence)

## Code of Conduct

This project and everyone who takes part in it are governed by the
[Contributor Covenant 3.0 Code of Conduct](CODE_OF_CONDUCT.md). By taking part, you agree to uphold
it. Report unacceptable behaviour confidentially to
[lucas.greuloch@gmail.com](mailto:lucas.greuloch@gmail.com).

## Project documentation map

| Topic                                                              | Where                                                                  |
|:-------------------------------------------------------------------|:-----------------------------------------------------------------------|
| What P4KX does, installing, building it                            | [`README.md`](README.md)                                               |
| The commands and the MCP server                                    | [`docs/commands.md`](docs/commands.md), [`docs/cli-json.md`](docs/cli-json.md) |
| Using the window                                                   | [`docs/user-help.md`](docs/user-help.md)                               |
| Product decisions and milestones                                   | [`docs/ROADMAP.md`](docs/ROADMAP.md)                                   |
| The glossary: the terms to use in code, issues and commits         | [`CONTEXT.md`](CONTEXT.md)                                             |
| Architecture decisions                                             | [`docs/adr/`](docs/adr/)                                               |
| Architecture rules, test layout, real-Archive suite, working rules | [`CLAUDE.md`](CLAUDE.md)                                               |
| The look of the window: theme, desktop spec, logo                  | [`docs/design/`](docs/design/) and `p4kx_gui::theme`                   |
| The formats P4KX reads and writes                                  | the documents under [`docs/`](docs/)                                   |
| Release notes                                                      | [`CHANGELOG.md`](CHANGELOG.md), unreleased ones in [`changelog.d/`](changelog.d/README.md) |
| Security reporting, supported versions, scope                      | [`.github/SECURITY.md`](.github/SECURITY.md)                           |

`CLAUDE.md` holds the working rules for human and AI contributors alike; read it before your
first pull request.

## Asking questions

Start with the [README](README.md) and the [user help](docs/user-help.md). If they do not answer
your question, open an issue and say what you tried.

## Reporting a security vulnerability

Do not open a public issue. Follow [`.github/SECURITY.md`](.github/SECURITY.md).

## Reporting bugs

Use the [bug report template](https://github.com/p4kx-org/p4kx/issues/new?template=bug_report.yml).
Before you open one:

- Use the latest Release; the bug may already be fixed.
- Search open and closed issues for duplicates.
- Note the P4KX version, how it was installed, the Star Citizen channel and Build, and the exact
  steps. Name Entry paths instead of attaching files.

## Suggesting features

Use the
[feature request template](https://github.com/p4kx-org/p4kx/issues/new?template=feature_request.yml).
Describe the problem first, then the solution you have in mind. Check
[`docs/ROADMAP.md`](docs/ROADMAP.md) to see whether it is planned already.

## Game data and the Archive key

P4KX reads your own Star Citizen installation and ships no game data.

- Never commit, attach or upload game files or anything extracted from them. Tests use synthetic
  Archives from `p4kx-testkit`.
- Never write into the game folders; P4KX itself refuses to.
- The Archive key is never part of the repository, in any spelling (ADR-0027). `cargo xtask keys`
  checks this.
- Never read or share the launcher's credential files.

## Local development setup

You need the stable Rust toolchain (see the [README](README.md#build)) on Windows or Linux, and
for the Blender add-on Python and Blender 5.2 LTS.

On Linux the audio preview builds against the ALSA headers: `sudo apt-get install
libasound2-dev` (Debian and Ubuntu; CI installs the same package). PipeWire and PulseAudio need
nothing extra at build time; at run time they serve the ALSA `default` device through
`pipewire-alsa` or `pulseaudio-alsa`, which desktop distributions install.

```powershell
cargo build --release
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --locked --doc
cargo xtask
```

`cargo test --workspace --locked` works as well when cargo-nextest is not installed. `cargo xtask`
checks the layer direction of the crates, the 400-line limit, the SPDX headers, the changelog
fragments, the lint set and the Archive key. After a dependency change, run
`cargo xtask notices --write` (needs `cargo install cargo-about --version 0.9.2 --locked`).

The tests against a real Archive are ignored by default; see
[Real-Archive suite](#real-archive-suite).

### Real-Archive suite

A second suite checks a real `Data.p4k`: opening and listing it, its Build, reading Entries of
every compression method and encryption state present, extracting a small subset, comparing two
Archives on both levels, and the conversions of the formats P4KX reads. Its tests are
`#[ignore]`d, so `cargo test` and CI report them as ignored. Point `P4KX_DATA_P4K` to an Archive
and `P4KX_DATA_P4K_B` to a second one for the Comparison, and pass `--ignored`; without
`P4KX_DATA_P4K` the suite fails and says so, and the tests over two Archives return early without
`P4KX_DATA_P4K_B`. Tests that read encrypted Entries use the Archive key from `P4KX_ARCHIVE_KEY`,
else the file named by `P4KX_KEY_FILE`, else the file of `p4kx key set`.

```powershell
$env:P4KX_DATA_P4K = 'C:\Program Files\Roberts Space Industries\StarCitizen\LIVE\Data.p4k'
$env:P4KX_DATA_P4K_B = 'C:\Program Files\Roberts Space Industries\StarCitizen\PTU\Data.p4k'
cargo test --release --workspace --locked -- --ignored
```

The Archive is only read. The extracted subset goes to a temporary folder below `TMP`, which is
removed afterwards. `CLAUDE.md` describes the test layout, the differential tests against unp4k,
unforge and StarBreaker ([docs/differential-verification.md](docs/differential-verification.md))
and how to run a single real-Archive test.

## Pull requests

1. Open or pick an issue first, so the change is agreed before you spend time on it.
2. Branch from `main`.
3. Keep a pull request to one concern. Refactors that are not needed for the change go into a pull
   request of their own.
4. Fill in the [pull request template](.github/PULL_REQUEST_TEMPLATE.md) and link the issue with
   `Closes #<issue>`.
5. CI runs the lints, `cargo xtask`, the tests on Windows and Linux, the schema and concurrency
   tests, the Blender add-on tests, CodeQL and the DCO check. All must pass.
6. A maintainer checks your [CLA](#contributor-license-agreement-cla), reviews and merges with a
   merge commit.

## Contributor License Agreement (CLA)

Before your first contribution is merged, you sign the
[P4KX Individual Contributor License Agreement](CLA.md) once; it covers all your present and
future contributions. You keep the copyright in your contributions: the CLA grants the maintainers
a perpetual copyright and patent licence, in addition to the MIT and Apache-2.0 licences under
which your contribution is distributed.

To sign, open a pull request titled `CLA: sign — <your-github-handle>` that adds your row to
[`docs/cla-signatures.md`](docs/cla-signatures.md), with the acceptance sentence from
[§ 11 of the CLA](CLA.md#11-how-to-sign) in its description. If you contribute on behalf of an
employer, an Entity CLA is needed as well; ask
[lucas.greuloch@gmail.com](mailto:lucas.greuloch@gmail.com).

The CLA applies to every contribution that ends up in a commit on `main` under your authorship:
code, documentation, translations, workflows, typo fixes alike. A pull request from an author
without a signed CLA waits until it is signed.

## Developer Certificate of Origin (DCO)

In addition to the one-time CLA, every commit carries a `Signed-off-by` trailer that certifies the
[Developer Certificate of Origin, version 1.1](https://developercertificate.org/): that you wrote
the change or otherwise have the right to submit it under the project's licence. The CLA is the
lasting licence grant; the sign-off is the per-commit statement of origin.

```powershell
git commit -s
```

That appends `Signed-off-by: Your Name <you@example.org>`, taken from `git config user.name` and
`user.email`. To fix commits you already made:

- the last commit: `git commit --amend --signoff --no-edit`
- every commit since `main`: `git rebase --signoff main`

then update the pull request with `git push --force-with-lease`. Never force-push `main`.

The *DCO sign-off* job of the [Commit Gates](.github/workflows/commit-gates.yml) workflow checks
every commit of a pull request: its `Signed-off-by` trailer must match the commit's author name and
e-mail (the e-mail case-insensitively). Merge commits and Dependabot and GitHub Actions commits are
skipped.

## Commit messages

- The subject is one imperative sentence, starting with a capital letter, without a trailing full
  stop, at most about 72 characters. A prefix naming the area is welcome:
  `Assembly: choose the sub-geometry on replaced ports too`.
- The body explains *why*. The story of the change belongs here and in the pull request, not in
  code comments.
- Use the terms of [`CONTEXT.md`](CONTEXT.md) and British spelling.
- The footer carries the `Signed-off-by:` trailer.

## Style guides

### Rust

- `thiserror` in libraries, `anyhow` only in binaries; no `unwrap` / `expect` outside tests.
- `unsafe_code` is forbidden everywhere except the one module ADR-0006 allows.
- Libraries are synchronous and parallelise with rayon; tokio only in `p4kx-mcp`.
- Crates follow the three layers of ADR-0003: format crates, service crates, front-ends. Never add
  a dependency against this direction.
- Source files stay below about 400 lines; split by responsibility before that.
- Every `.rs` file starts with the two-line SPDX header.

### Comments

Doc comments (`///` and `//!`) and short technical comments (`//`) are welcome where they document
what the code cannot say itself: a non-obvious invariant, a format detail, a unit or coordinate
convention, the spec section an algorithm implements. Both stay short and describe only the
current state: no history, no dates or issue numbers, no comments that restate the code, no
commented-out code and no `TODO`. Lint suppressions use `#[expect(lint, reason = "...")]`.

### Window

The window follows P4KX's own theme strictly: dark only, Lato only. Every colour, size, spacing and
radius comes from `p4kx_gui::theme` and [`docs/design/`](docs/design/); a new value goes into both
together. Verify a visual change with a render against the desktop spec and attach screenshots.

### Strings

User-visible strings live in the Fluent files for `de-DE` and `en-GB` (British spelling), never
inline.

### Tests

A crate's integration tests are modules of one test binary, `tests/it/`; tests against a real
Archive are modules of `tests/real_archive/`. See "Test layout" in [`CLAUDE.md`](CLAUDE.md).

## Before-you-push checklist

- [ ] `cargo fmt --all` leaves nothing to change.
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
- [ ] `cargo xtask` passes.
- [ ] The tests pass.
- [ ] A user-visible change has its `changelog.d/` fragment.
- [ ] The CLA is signed.
- [ ] Every commit is signed off.
- [ ] No game data, extracted assets, Archive key or other secrets in the diff.

## Releasing

Maintainers only. The [Release workflow](.github/workflows/release.yml) builds and publishes
everything; nobody uploads assets by hand.

1. Fold the `changelog.d/` fragments into `CHANGELOG.md` (`cargo xtask changelog --release
   <version>`) and merge that pull request. The version of `p4kx-gui` is the Release version.
2. Push the tag `vMAJOR.MINOR.PATCH[-suffix]` on the merge commit. The workflow refuses any other
   tag name.
3. The workflow builds the portable ZIP, the MSI and the Blender add-on ZIP, tests install,
   upgrade and uninstall of the MSI, and generates a CycloneDX SBOM per shipped binary
   (`p4kx-gui`, `p4kx`, `p4kx-mcp`). The add-on is Python without bundled third-party packages
   and has no SBOM of its own.
4. The ZIP and the MSI are scanned on VirusTotal. A failed or missing scan never blocks the
   Release; the notes then say the scan did not run.
5. The build provenance of every asset and the SBOMs of the ZIP and the MSI are attested
   (Sigstore, via GitHub Artifact Attestations). The bundles become Release assets.
6. `SHA256SUMS.txt` and `SHA3-512SUMS.txt` list every other asset, as plain text files that open
   in any text editor. The Release notes are the
   `CHANGELOG.md` section, the VirusTotal result, the verification commands, and GitHub's generated
   "What's Changed" list.

A pull request that changes the workflow, its scripts, the installer or the add-on runs the same
jobs as a dry run: everything is built, but nothing is scanned, attested or published.
The `workflow_dispatch` trigger adds the VirusTotal scan to such a run.

Secret: the owner sets the repository secret `VIRUSTOTAL_API_KEY` (Settings, Secrets and
variables, Actions). Without it the scan is skipped.

### Verifying a download

Replace `<file>` with the name of the downloaded asset.

- Hashes, Windows PowerShell: `Get-FileHash <file> -Algorithm SHA256`; Linux:
  `sha256sum --check --ignore-missing SHA256SUMS.txt`. SHA3-512 on Linux: `openssl dgst -sha3-512
  <file>`. On Windows `openssl` is usually not installed and `Get-FileHash` has no SHA3-512; use
  PowerShell 7 on an operating system with SHA3 support (Windows 11 24H2 or newer):
  `[Convert]::ToHexString([Security.Cryptography.SHA3_512]::HashData([IO.File]::ReadAllBytes("<full
  path>")))`, which prints upper-case hex. Compare the output with the line of the file in
  `SHA256SUMS.txt` or `SHA3-512SUMS.txt`, ignoring case.
- Attestation: `gh attestation verify <file> --repo p4kx-org/p4kx`.
- Attestation offline, with the bundle from the Release:
  `gh attestation verify <file> --bundle P4KX-<version>-provenance.sigstore.json --repo
  p4kx-org/p4kx`. The bundle of an SBOM is named after it (`<sbom>.cdx.json.sigstore.json`) and
  needs `--predicate-type https://cyclonedx.org/bom`.
- SBOM: the `P4KX-<version>-<binary>.cdx.json` assets are CycloneDX 1.5 JSON. Their attestation
  ties them to the ZIP and the MSI, and the provenance bundle covers the SBOM files themselves.

## Licence

P4KX is licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT licence](LICENSE-MIT) at your option. Unless you state otherwise, any contribution you
intentionally submit for inclusion in P4KX is dual-licensed as above, without any additional terms
or conditions.
