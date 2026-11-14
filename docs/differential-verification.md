# Differential verification against unp4k, unforge and StarBreaker

Differential checks compare P4KX with independently produced output. The ignored tests in
`crates/p4kx-extraction/tests/reference/` never launch or download foreign tools. Hosted CI runs
their synthetic mutation and integration tests; game files, binaries, keys and real manifests stay
outside the repository.

A configured reference root requires a version-1 `.p4kx-reference.json`. An unset optional variable
skips explicitly. An existing empty folder, partial producer run or zero meaningful comparisons
fails verification. A genuinely declared empty selection reports `no independent comparisons`;
it does not become a successful oracle run.

| Variable | Producer output | Comparison |
| --- | --- | --- |
| `P4KX_REF_UNP4K` | Raw extracted Entries | Streamed exact bytes, with a narrowly evidenced zero-trim allowance |
| `P4KX_REF_UNFORGE` | Converted CryXml with declared original Entry identities | Element trees: names, ordered attributes, trimmed text and children |
| `P4KX_REF_STARBREAKER` | Record JSON with declared GUID/DCB identities and optional JSON pointers | Typed Record values and explicitly selected embedded content |

Run configured comparisons with the existing Archive/key helpers:

```powershell
cargo test --release -p p4kx-extraction --test reference --locked -- --ignored --nocapture --test-threads 1
```

The manual preparation and schema-writing tests skip unless their own request variables are set.
No reference test reads a launcher credential file.

## Trust and source identity

The checked-in schemas describe the versioned contract:

- [Reference manifest](../schemas/p4kx-reference.schema.json)
- [Pre-conversion plan](../schemas/p4kx-reference-plan.schema.json)
- [Derived producer error report](../schemas/p4kx-reference-errors.schema.json)

The manifest records producer name, version, immutable revision, binary SHA-256 and declared
unmodified/local-patch scope separately. Preparation measures the actual binary before conversion;
finalization checks it again. That identifies the local binary file. Its relationship to the
declared source revision and the owner's success receipt are **claims**, not build attestations or
proof that a foreign process executed.

The verifier locally recomputes SHA-256 of the current Archive, its canonical Entry inventory and,
for Record references, the decoded preferred DCB. Channel and all Build identifiers must also match.
A digest stored in JSON alone is not considered verified. The Archive watch rejects observed
source changes. Hashing a large Archive is deliberate and can dominate a bounded comparison run.

Expected logical identities and output locations are frozen **before conversion**. They are Entry
paths or canonical Record GUIDs plus DCB identity, not surviving filenames. All selected identities
must exist in the measured source dataset. Duplicate identities cannot replace missing members.
Every declared artifact is present, uniquely named, fingerprinted and accounted for; extra JSON,
logs and nested manifests fail. Only the exact root `.p4kx-reference.json` is excluded.

Artifact paths are bounded relative paths. Absolute/drive/UNC paths, parent traversal, ambiguous
components, symlinks, junctions, resolved escapes, case aliases and hard-linked artifact aliases
are rejected before payload comparison. Multiple Records may share one file only through distinct,
declared JSON locations that contain their actual identities. One Record may have several declared
artifacts; it still counts as one logical source.

## Preparing a new reference run

Keep the plan, producer binary and owner completion receipt **outside** the output root. Use a
fresh output root; `begin` refuses pre-existing payloads, preventing post-hoc certification of
whatever happened to survive. Explicit source-copy artifacts may exist before conversion and must
match their original Archive Entries byte for byte.

A plan supplies `schemaVersion: 1`, producer metadata, the declared selection, expected logical
identities/output locations and artifact paths/roles. Its artifacts have `fingerprint: null`:
finalization supplies measured fingerprints. The roles include `payload`, optional `sourceCopy`,
exactly one `producerStdout`, `producerStderr` and `producerErrorLog`.

An illustrative pair of XML mappings is:

```json
{
  "expected": [
    {
      "identity": {"kind": "entry", "path": "Data/example/shared.cdf"},
      "targets": [{"path": "cdf/shared.xml", "subresource": null}]
    },
    {
      "identity": {"kind": "entry", "path": "Data/example/shared.chrparams"},
      "targets": [{"path": "chrparams/shared.xml", "subresource": null}]
    }
  ]
}
```

These are synthetic paths, not a runnable inventory. Build the actual expected list from the
selected source Archive/DCB before running the producer. Isolate CDF and CHRPARAMS families so
unforge's same-stem `.xml` names cannot overwrite one another. The manifest maps each result to
its original extension; the verifier never guesses from neighboring files. Original input files
retained inside the root require explicit `sourceCopy` roles.

With existing `P4KX_DATA_P4K` and key configuration, prepare the source snapshot and frozen plan:

```powershell
$env:P4KX_REF_PREPARE_ROOT = '<private fresh reference root>'
$env:P4KX_REF_PREPARE_PLAN = '<private pre-conversion plan.json outside that root>'
$env:P4KX_REF_PRODUCER_BINARY = '<actual producer executable>'
$env:P4KX_REF_PREPARE_STAGE = 'begin'
cargo test --release -p p4kx-extraction --test reference --locked reference_prepare::prepare_reference_manifest -- --ignored --exact --nocapture
```

Run the chosen foreign tool separately, from the intended private working folder, capturing its
raw UTF-8 stdout and stderr at the two predeclared paths. Do not run these commands from an
Installation or let the producer overwrite game files. Keep unp4k isolated from outbound exception
reporting: the inspected source can report extraction exceptions remotely. Network isolation does
not justify falsely declaring a modified binary unmodified.

The supported reference commands remain the producers' own commands, for example unp4k raw
Extraction, isolated unforge file/family Conversion and StarBreaker `dcb extract --format json`.
Official StarBreaker CLI 0.3.2 is main-only and does not accept `--all`; the separately pinned
current-source build supports it. A matching version string alone does not identify those builds.
Do not invent flags or infer all-Record coverage from a main-only release.

Create a completion receipt from the actual observed exit status and errors, after inspecting the
captured streams. Its shape is:

```json
{"ownerAssertsSuccess": false, "exitCode": null, "errors": []}
```

The example intentionally makes no success claim. For several producer invocations, the receipt
must cover all invocations and preserve any nonzero exit/error rather than just the final command.
Set `ownerAssertsSuccess` only after that review; it is retained as an owner assertion.

Finalize without launching any foreign process:

```powershell
$env:P4KX_REF_PREPARE_RECEIPT = '<private actual completion receipt.json outside the root>'
$env:P4KX_REF_PREPARE_STAGE = 'finalize'
cargo test --release -p p4kx-extraction --test reference --locked reference_prepare::prepare_reference_manifest -- --ignored --exact --nocapture
Remove-Item Env:P4KX_REF_PREPARE_STAGE
```

Finalization recomputes source and binary fingerprints, checks the frozen mapping, derives the
typed producer error report from the captured streams, fingerprints the complete artifact inventory
and verifies source copies. A failure leaves the manifest explicitly unfinalized. No preparation
step claims to run or observe the foreign process itself.

Legacy folders without preserved pre-conversion expectations must be reproduced in a fresh root.
Scanning old survivors and labeling them complete is not a migration path.

## Producer errors and review status

Captured stderr is not synonymous with failure: both inspected StarBreaker revisions write normal
loading/export/completion progress there. The separate error report records hashes of both captured
streams and is recomputed during verification; a forged empty error report cannot hide their errors.

The classifiers are pinned to these primary-source contracts:

- unp4k/unforge revision
  [b492ab14](https://github.com/dolkensp/unp4k/tree/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b):
  per-Entry extraction exceptions and conversion failures are printed to stdout and can coexist
  with exit code zero. [unp4k entrypoint](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unp4k/Program.cs),
  [unforge entrypoint](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge.cli/Program.cs).
- StarBreaker
  [release 450fa284](https://github.com/diogotr7/StarBreaker/blob/450fa284d4b1cbed6832675297dfb02579c0abd3/cli/src/dcb.rs)
  and source revisions [7d3f01e7](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/cli/src/dcb.rs)
  and [305215bca](https://github.com/diogotr7/StarBreaker/blob/305215bca75350013c221bdfbcb2f8180fe09dbe/cli/src/dcb.rs)
  have separate producer identities. The 305215bca DataCore export entry point is byte-identical
  to 7d3f01e7. Individual export/write/directory errors can appear on stderr with a zero exit
  status. The verifier checks those errors, completion markers and exported-file counts.

Unknown revisions, local patches, unclassified log lines, invalid log encoding or missing completion
markers produce `review_required`/incomplete verification. They are not silently treated as
error-free. A reviewed new log contract requires primary-source evidence and regression tests;
the receipt alone cannot waive that gate. No invented empty error log bypasses actual streams,
additional error files or missing outputs.

## Scope and accounting

`all` requires the full logical universe: all Entries for raw comparison, the CDF/CHRPARAMS
families for unforge, or every Record of the preferred DCB for StarBreaker. A filtered, sampled or
main-only run compares its explicit predeclared subset; the declaration describes the selection,
not a predicate guessed from surviving output. `mainOnly` is an unverified producer claim:
StarBreaker selects the last Record per raw file-name offset, which the public P4KX Record API
does not expose. The verifier proves membership of the declared GUID/DCB subset and compares its
outputs; it does not attest that every selected GUID satisfies that foreign main-record rule.

Reports show selected logical sources versus the source universe, standalone and explicitly
embedded locations, and verified artifact counts. Multiple artifacts or embedded occurrences do
not compensate for a missing identity. Unselected Records are outside the declared scope, not
missing requested outputs. Embedded-content walk counts can repeat a Record at several locations
and are reported separately from unique selected-source coverage.

A comparison fails on any missing/extra/duplicate/unreadable selected artifact, mismatched hash,
invalid provenance, producer error, incomplete run or unallowed semantic difference. Every selected
logical source must contribute exactly one reconciled result. Building all P4KX Record trees is a
separate self-check and cannot substitute for an independent reference comparison.

| Bound | Limit |
| --- | --- |
| Manifest or preparation JSON | 64 MiB before parsing |
| Logical identities | 1,000,000 |
| Artifacts and mappings | 2,000,000 |
| Decoded metadata text, relative path or JSON pointer | 4,096 UTF-8 bytes before field allocation |
| Escaped JSON metadata token scratch | 24 KiB before parsing |
| Hashed artifact | 8 GiB, streamed |
| Structured JSON/XML or captured log | 64 MiB |
| Captured log line / recorded findings | 64 KiB / 1,000 |

## Narrow comparison allowances

An allowed difference is separately counted under its reason, never exact equality.

- unp4k: only a nonempty all-zero suffix of encrypted Stored or effective method-100 pass-through
  bytes with validated matching source and the inspected unmodified producer revision. Stored
  output retains one byte (possibly zero), or at least three bytes ending in a nonzero byte.
  Method-100 output retains at least four bytes ending in a nonzero byte because the producer
  rewinds its four-byte probe after trimming. Unknown context, inconsistent retained prefixes,
  nonzero loss and actual Deflate/Zstandard logical loss fail
  ([ADR-0341](adr/0341-reference-zero-trimming-requires-effective-decoder-and-producer-evidence.md)).
- unforge: `BUGGED` against a nonempty value, and its documented byte-wise interpretation of
  non-ASCII UTF-8. Other value changes fail.
- StarBreaker: the narrowly typed weak-pointer, dangling-reference, invalid-bool and non-finite
  number differences described in ADR-0234. An unrelated error/unmapped kind never becomes null.

`EntryReader::effective_encoding` observes the existing decoder without additional reads.
Method 100 stays `Undetermined` until its decrypted prefix is read; an empty logical Entry can
remain undetermined. The additive read-only enum is also re-exported by extraction. It does not
prove checksum validity, producer success or reference completeness.

## Synthetic verification and schema maintenance

The ordinary `reference` suite runs without game data or foreign binaries:

```powershell
cargo test -p p4kx-extraction --test reference --locked
```

Its mutation tests cover byte/XML/Record changes, source/producer/Build/DCB fingerprints, failed
receipts and logs, unknown capabilities, empty scopes, missing/extra/duplicate outputs, same-stem
XML, multiple artifacts, explicit embedded positions, path escapes/aliases and size bounds.
Preparation fixtures exercise the same before/after helpers as a real manual run.

After intentionally changing the versioned DTOs, regenerate and verify their schemas:

```powershell
$env:P4KX_WRITE_REFERENCE_SCHEMAS = '1'
cargo test -p p4kx-extraction --test reference --locked reference_schema::write_reference_schemas -- --ignored --exact
Remove-Item Env:P4KX_WRITE_REFERENCE_SCHEMAS
cargo test -p p4kx-extraction --test reference --locked
```

All examples and committed fixtures are synthetic. Publish only aggregate outcomes from real runs,
not manifests, source identities, private paths or game payloads.

## First run on LIVE 4.10, 2026-10-27

Reference production (one machine, tools 4.0.87 and 0.3.2):

| Step | Time | Result |
|------|------|--------|
| unp4k `*.dcb` | 4 s | `Data\Game2.dcb`, 336,772,966 bytes |
| unp4k `*.chrparams` and `*.cdf` | 10.6 s | 4,151 and 1,690 files |
| unforge | 23.9 s | 5,728 of 5,841 files converted |
| StarBreaker `dcb extract --format json` | 34.5 s | 62,736 files, 3,069 MB, main Records only |

P4KX against it (release build):

| Test | Compared | Equal | Different | Allowed |
|------|----------|-------|-----------|---------|
| unp4k bytes | 5,842 | 5,842 | 0 | 0 |
| unforge trees | 5,728 | 5,728 | 0 | 0 |
| StarBreaker values | 62,736 | 62,160 | 0 | 576 |

"Allowed" counts Records. The StarBreaker row is the run of 2026-10-28 that compares embedded
Records by content; the first run of 2026-10-27 compared them as a Reference by GUID and had 62,162
equal and 574 allowed.

- The 5,842 Entries are all Zstandard and unencrypted. The encrypted allowance was not exercised;
  it is covered by the mutation self-test only.
- The 576 allowed StarBreaker Records hold 929 allowed values: 880 `null` for a Reference to a
  Record that does not exist and 49 `null` for a weak pointer in a derived type, counting those
  inside embedded Records. No other kind of error, no invalid bool and no non-finite number
  occurs, so those entries were not exercised; they are covered by the mutation self-tests only.
- 55,023 embedded Records were compared by content, those nested in another embedded Record
  included; none differs. Names, GUIDs and Record paths match case-sensitively.
- 54,973 of the 117,709 Records have no file in the reference: StarBreaker writes the main Record
  of each file only.
- Opening `Game2.dcb` (read, decrypt and decompress the Entry, open the DataForge) takes 0.32 s.
  Building the Record tree of all 117,709 Records takes 0.32 s on all cores, with none failing;
  that is the tree, not the writing of JSON files.
- The first comparison run found one error of the test itself, not of P4KX: StarBreaker embeds
  the Record of the same file where it is referenced, which the comparison now reads and
  compares by content.
