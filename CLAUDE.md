# CLAUDE.md

Working notes for Claude Code (and human contributors) in this repo. Keep this file current when
the build, architecture, or the rules below change.

## What this is

**P4KX** — a Rust desktop tool with a native Rust GUI that opens, browses and extracts the
`Data.p4k` archive of Star Citizen. It reimplements:

- the functionality of [dolkensp/unp4k](https://github.com/dolkensp/unp4k) (reading the p4k
  archive, decompressing and decrypting its entries, and converting the CryEngine formats it
  contains, e.g. binary XML / DataForge), and
- the functionality of the owner's earlier reference tool, whose source code was lost. It is still
  installed locally and serves as the behavioural reference; its behaviour is reconstructed by
  observing the installed program, not from source.

## Real test data

Real p4k files live under `C:\Program Files\Roberts Space Industries\StarCitizen\<channel>\Data.p4k`,
one folder per channel (`LIVE`, `PTU`, `EPTU`, `HOTFIX`, `TECH-PREVIEW`). They are many gigabytes,
read-only and game assets: never copy them or anything extracted from them into the repository, and
never write into the game folders. Extraction output during development goes under
`D:\Coding\claude-worktrees\p4k-extractor\`.

Never read `loginData.json` or other launcher credential files in the game folders. Tests against a
real Archive are `#[ignore]`d and need `P4KX_DATA_P4K`; CI uses synthetic
fixtures. Run them with `TMP` and `TEMP` under `D:\Coding\claude-worktrees\p4k-extractor\` so the
extracted subset lands there; the suite also uses the stored key (`P4KX_ARCHIVE_KEY`, else the
file named by `P4KX_KEY_FILE`, else `p4kx key set`'s file), so none is set by hand:

```powershell
$env:TMP = 'D:\Coding\claude-worktrees\p4k-extractor\tmp'; $env:TEMP = $env:TMP
$env:P4KX_DATA_P4K = 'C:\Program Files\Roberts Space Industries\StarCitizen\LIVE\Data.p4k'
$env:P4KX_DATA_P4K_B = 'C:\Program Files\Roberts Space Industries\StarCitizen\PTU\Data.p4k'
cargo test --release --workspace --locked -- --ignored
```

That runs every real-Archive test in the workspace, whatever test file it lives in. In the crates
that group their tests (see "Test layout"), the real-Archive tests are the modules of the
`real_archive` test binary, so one test runs as
`cargo test --release -p <crate> --test real_archive -- <test_name> --ignored`. The CI suite
runs faster with cargo-nextest: `cargo nextest run --workspace --locked`, then
`cargo test --workspace --locked --doc` for the doctests nextest skips; `cargo test --workspace
--locked` stays valid. Prefer nextest for `p4kx-render` locally: its GPU tests run one process each
with a time limit (`.config/nextest.toml`), while one shared process can hang in a graphics driver.
In a plain `cargo test` run, `p4kx_testkit::watchdog` ends a hung process with exit code 3 and
names the running tests and their phases; `P4KX_TEST_DEADLINE_SECS` overrides its 600 s limit. The two concurrency stress tests need optimised code and run with
`cargo nextest run --cargo-profile concurrency -p p4kx-dataforge --test lazy_indexes` and
`-p p4kx-assembly --test audio_lazy_index`. A new
real-Archive test gets `#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]`
(a test that reads encrypted Entries also needs the Archive key; its failure message says so,
the reason stays as written) and takes the path from `p4kx_testkit::real_archive_path`, and from
`p4kx_testkit::real_archive_b_path` for a second Archive, read from `P4KX_DATA_P4K_B`, and the key
from `p4kx_testkit::real_archive_key`, which opens it with `Archive::open_with_key`. A test
needing the second Archive returns early when it is not set (`real_archive_b_path` gives `None`),
so it skips cleanly. A test over both Archives takes them from `p4kx_testkit::real_archive_pair`
and returns early when it gives `None`. A timing test that must not share the cores with other tests
gets its own test file.

The differential tests of `crates/p4kx-extraction/tests/reference/` compare P4KX with the output of
unp4k, unforge and StarBreaker, which the owner produces beforehand (never a test, never CI); they
read it from `P4KX_REF_UNP4K`, `P4KX_REF_UNFORGE` and `P4KX_REF_STARBREAKER`
(`p4kx_testkit::reference_unp4k_path` and its two siblings) and skip when a variable is unset.
`docs/differential-verification.md` says how to produce the folders and holds the results of the
first run; a difference not on its allow-list fails the test.

The ship matrix check of `p4kx-export` (ADR-0245) reads the JSON the owner saved by hand as
`local-data/ship-matrix/ship-matrix.json` (excluded from git locally; nothing fetches it) from
`P4KX_REF_SHIP_MATRIX` (`p4kx_testkit::reference_ship_matrix_path`) and skips when it is unset.

The SCMDB cross-check of `p4kx-extraction` (ADR-0267) reads the SCMDB dumps the owner saved by
hand to `local-data/scmdb/` (`merged-<v>.json`, `crafting_blueprints-<v>.json`,
`mining_data-<v>.json`; excluded from git locally; nothing fetches them) from `P4KX_REF_SCMDB`
(`p4kx_testkit::reference_scmdb_path`). It skips when the variable is unset or no `<v>` matches the
Archive's build manifest.

The same run includes the RFC 8251 Opus decoder vectors test of `p4kx-audio`. It reads the folder
of the official `testvectorNN.bit` and `.dec` files from `P4KX_OPUS_VECTORS` and skips with a note
when the variable is unset.

## Test layout

Linking one binary per test file dominates the build, so a crate's integration tests are modules of
one binary: `crates/<crate>/tests/it/main.rs` (binary `it`) only lists `mod <name>;`, every test
file is `tests/it/<name>.rs`, and shared helpers are modules beside them (`tests/it/support/mod.rs`,
used as `use crate::support;`). The real-Archive tests are the modules of `tests/real_archive/main.rs`
(binary `real_archive`). A test gets its own binary under `tests/` only when it needs a process of
its own: a timing test that must not share the cores, a stress test CI runs in `--release`
(`lazy_indexes`), or a test CI names by binary (`report_schema`, `catalog_schema`, `record_json_schema`). One test runs as
`cargo test -p <crate> --test it -- <test_name>`. Insta snapshots live in `tests/it/snapshots` and are named
`it__<module>__<test>.snap`.

Crates not yet converted still have one binary per file, and a crate that is converted stays so:
a new test file goes into `tests/it/` and `main.rs` gets its `mod` line.

## Plan and decisions

- [docs/ROADMAP.md](docs/ROADMAP.md): product decisions, the milestones M1–M14 (all done) and the
  releases up to 0.4.2, with 0.5.0 next.
- [README.md](README.md): what P4KX does and how to install it; [docs/commands.md](docs/commands.md):
  every command and the MCP tools; [docs/user-help.md](docs/user-help.md): the window. Keep them
  in step with `p4kx --help` when a command or option changes.
- [CONTEXT.md](CONTEXT.md): the glossary; use its terms in code, issues and commits.
- [docs/adr/](docs/adr/): architecture decisions.

## Architecture

- `p4kx-*` crates in three layers (ADR-0003, ADR-0019): format crates depend only on
  `p4kx-foundation` and the format crates they declare in `builds-on` (never `p4kx-archive`),
  service crates on format crates, front-ends (`p4kx`, `p4kx-gui`, `p4kx-mcp`) only on services.
  Never add a dependency against this direction.
- Libraries are synchronous and parallelise with rayon; tokio only in `p4kx-mcp`. The one
  exception is Sound Conversion during Extraction, which runs on plain worker threads (ADR-0022).
- `thiserror` in libraries, `anyhow` only in binaries; no `unwrap` / `expect` outside tests.
- `unsafe_code = "forbid"` everywhere except the `cfg(windows)` DWM module in `p4kx-gui`, whose
  manifest repeats the workspace lints with `unsafe_code = "deny"`; `cargo xtask lints` checks
  that (ADR-0006).
- The look follows the P4KX theme strictly: dark only, Lato only (ADR-0006). See "GUI design
  (hard rule)" below.
- The window opens Direct3D 12 first on Windows and falls back to the default wgpu backends;
  `WGPU_BACKEND` overrides it, and the GUI's headless tests open only the software backend
  (ADR-0317).
- Source files stay below about 400 lines; split by responsibility before that.
- One canonical Record tree serialised to XML and JSON; References are links, never embedded
  (ADR-0004).
- The Archive key is never part of the repository (ADR-0027): no constant, fixture, document or
  commit holds it, in any spelling. Synthetic Archives use `p4kx_testkit::test_key`, real-Archive
  tests read it through `p4kx_testkit::real_archive_key`, and `cargo xtask keys` fails on a
  key-like literal named as a key, and on the real key itself while `P4KX_ARCHIVE_KEY` is set.
- UI strings live in Fluent files for `de-DE` and `en-GB` (British spelling), never inline.

## GUI design (hard rule)

- **P4KX's own theme is the reference, for the whole repository.** `p4kx_gui::theme` holds every
  colour, type size, weight, spacing, radius and glow, and `docs/design/` holds the desktop spec
  and its machine-readable form, and the logo ([logo.md](docs/design/logo.md)). No external design system is followed. This holds for every person and every agent,
  for every change to anything the tool shows (windows, dialogs, views, icons, the About view) and
  for every review of such a change.
- **Agents:** an agent that delegates work touching the GUI passes this rule on to the agent
  doing it.
- **Binding source:** read `p4kx_gui::theme` and `docs/design/` fresh before GUI work, never from
  memory. Build from the existing tokens and widgets, and verify a visual change with a render
  against the desktop spec.
- Never invent a colour, size, spacing or radius: a new value is added to the theme and to
  `docs/design/` together, with the reason in the commit message.
- Deviate from the desktop spec only where egui or the platform forces it, never in colour, form
  or type, and record the deviation in an ADR.

## Rules

- **Comments state the current state only.** Documentation comments (Rust: `///` and `//!`) and,
  where they help, short technical comments (`//`) that document what the code cannot say itself:
  a non-obvious invariant, a format detail, a unit or coordinate convention, the spec section an
  algorithm implements. Both stay short and precise. No history (dates, PR or issue numbers,
  "previously" / "now"), no comments that restate the code, no commented-out code and no `TODO`.
  Lint suppressions use `#[expect(lint, reason = "…")]`. The story of a change goes into the commit
  message and the PR body, decisions into an ADR.
- **Rust follows the [Google Rust Style Guide](https://google.github.io/styleguide/rustguide.html)**,
  read fresh from that page. Deviations only with the owner's explicit approval, recorded in an ADR.
  The rules marked `[chrome]` are Chromium-only and do not apply. The comment rule above applies.
  Apply it to what you write and touch; restyling existing code is its own change.
- **No code smells, no tech debt:** no duplication, dead code, oversized functions, magic numbers,
  needless clones or allocations, swallowed errors or "fix later" shortcuts. A clean fix out of
  scope becomes a follow-up issue, never a `TODO`.
- **Small and modular:** small modules with one responsibility each, behind narrow interfaces, so
  that a single module or part can be replaced without touching the rest. Keep changes and PRs
  small and focused.
- **Every `.rs` file starts with the SPDX header**, two lines and a blank line, before any `//!`
  docs: `// SPDX-License-Identifier: MIT OR Apache-2.0` and `// SPDX-FileCopyrightText: 2026 the
  P4KX contributors`. It is a licence header, not a code comment under the rule above;
  `cargo xtask spdx` (part of plain `cargo xtask`) fails on a tracked file without it.
- A user-visible change adds a fragment `changelog.d/<issue>.md` (form in
  `changelog.d/README.md`, ADR-0035) instead of editing `CHANGELOG.md`; `cargo xtask changelog
  --release <version>` assembles the fragments at the Release.
- Shell work uses PowerShell 7 (`pwsh`). Commit messages go through `git commit -s -F <file>`,
  PR and issue bodies through `--body-file <file>`.
- Worktrees and temporary files only under `D:\Coding\claude-worktrees\p4k-extractor\`.

## Agent skills

### Issue tracker

Issues live in GitHub Issues of `p4kx-org/p4kx`, accessed with the `gh` CLI. See
`docs/agents/issue-tracker.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
