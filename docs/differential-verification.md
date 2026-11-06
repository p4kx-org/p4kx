# Differential verification against unp4k, unforge and StarBreaker

The real-Archive tests check plausibility. The tests in `crates/p4kx-extraction/tests/reference/`
check P4KX against three independent tools, so that an error both of P4KX and of its own tests
cannot hide ([ADR-0234](adr/0234-differential-verification-reads-reference-output-from-folders.md)).
Each test is `#[ignore]`d, reads a folder of reference output the owner produced beforehand and
returns early with a note when its variable is unset. No test runs a foreign tool, CI downloads
nothing, and no reference output or game data is committed.

| Variable | Folder | Test | Comparison |
|----------|--------|------|------------|
| `P4KX_REF_UNP4K` | extracted by unp4k, paths as in the Archive | `unp4k::every_entry_unp4k_extracted_is_byte_identical` | bytes of every file against the decoded Entry, streamed in 64 KiB blocks; counts per compression and encryption kind |
| `P4KX_REF_UNFORGE` | `CryXml` converted by unforge, `<stem>.xml` next to its source | `unforge::every_cryxml_tree_unforge_converted_is_identical` | element trees: tag, attributes in order, trimmed text, children |
| `P4KX_REF_STARBREAKER` | `starbreaker dcb extract --format json` | `starbreaker::every_record_starbreaker_wrote_has_the_same_values` | per Record: GUID, name, tag, then every field by name, kind and value |

The real Archive and its key come as for every real-Archive test (`P4KX_DATA_P4K`, the stored
key). `tools\real-archive.ps1` sets the three reference variables when the folders exist. Run with
`cargo test --release -p p4kx-extraction --test reference --locked -- --ignored --nocapture
--test-threads 1`.

## Producing the reference folders

Use the official releases of the tools (unp4k and unforge 4.0.87 from dolkensp/unp4k, StarBreaker
CLI 0.3.2 from diogotr7/StarBreaker) and the same `Data.p4k` as the test. Extract only what the
comparison needs; the whole Archive is not required.

```text
unp4k.exe <Data.p4k> *.dcb              (in the folder for P4KX_REF_UNP4K)
unp4k.exe <Data.p4k> *.chrparams
unp4k.exe <Data.p4k> *.cdf
unforge.cli.exe <copy of the .chrparams and .cdf files>
starbreaker.exe dcb extract --dcb Data\Game2.dcb --format json --output <folder>
```

unp4k must write the raw bytes; the folder must keep the Archive paths below its root. The folder
of unforge keeps the sources next to the converted `.xml` files; only the `.xml` files are
compared, each against the Entry of its source. StarBreaker exports the main Record of each file
unless `--all` is given; Records without a file are counted as missing in the reference and do not
fail the test.

## Output and failing

Each test prints one summary line: compared, equal, different, allowed (items with at least one
allowed value), missing in the reference and missing in the Archive, then one line per allowed
reason with the number of allowed values, and at most 25 examples of differences (400 characters
each). The StarBreaker test also prints the number of embedded Records compared as a Reference.
The test fails on any difference that is not on the allow-list and on any reference file with no
Entry or Record in the Archive.

The allow-list holds the known behaviour of the reference tools. Each entry names one case and
applies to it only; each allowed value is counted under its reason, so one Record can count
several:

- unp4k: a nonempty all-zero suffix of encrypted Stored or effective method-100 pass-through
  data, only with validated source identity and the exact unmodified producer revision.
  The retained reference prefix must have one byte (possibly zero), or at least three bytes
  ending in nonzero. Empty/two-byte prefixes, inconsistent retained zeros, actual Deflate/Zstandard
  logical-byte loss and unknown/patched/mismatched contexts fail. Allowed values are counted under
  their specific producer reason, never exact equality
  ([ADR-0341](adr/0341-reference-zero-trimming-requires-effective-decoder-and-producer-evidence.md)).
- unforge: an attribute value or text `BUGGED` against a non-empty P4KX value, and text that is
  P4KX's non-ASCII UTF-8 read byte by byte.
- StarBreaker:
  - `null` for a weak pointer in a Record whose weak pointers sit in derived types (#262). A weak
    pointer is compared by kind only: against `null` or another weak pointer, never against any
    other value.
  - `null` for a Reference to a Record that does not exist (`DanglingReference`). Every other
    value P4KX reports as an error, and every kind of value the comparison does not map, is a
    difference against `null`.
  - `true` for a bool byte that is neither 0 nor 1 (`InvalidBool`); against `false` it is a
    difference.
  - `null` for a non-finite single or double, or its own text: `NaN` for not a number, `INF` and
    `-INF` for the infinities. A finite number against `null` or those texts is a difference.

A difference beside an allowed one still fails. Numbers are compared at the precision of the
field P4KX read: a single as `f32`, a double exactly, an integer exactly, `-0.0` equal to `0.0`.
Fields are sorted by name, ignoring case, and then compared by name; names, GUIDs and Record
paths are compared case-sensitively, because StarBreaker keeps the case of all three (the LIVE
run below passes that way). A Record that StarBreaker embeds where it is referenced is compared
by content with the Record P4KX holds under that GUID: name, struct type and every field, with the
same allow-list, and again for the Records embedded inside it. The Reference itself is compared by
GUID first. The number of embedded Records compared is reported; for a Record that differs, it
is the number reached before its first difference, so a lower bound. `crates/p4kx-extraction/tests/reference/starbreaker_json.rs` is the one place that
encodes the JSON shape of StarBreaker.

## Effective decoder and producer evidence

The declared compression method is not the effective encoding: CIG method 100 may contain a
Zstandard frame or direct bytes. The additive public `EntryReader::effective_encoding` API reports
`Stored`, `Deflate`, `Zstandard`, `PassThrough` or `Undetermined` from the same decoder that produces
the comparison bytes. Calling it performs no I/O and consumes no prefix; method 100 remains
undetermined until ordinary reading selects its decoder. An empty logical Entry can remain
undetermined. The enum is also re-exported by `p4kx-extraction`; no CLI/output schema changes.

The zero-trim classifier requires already-validated source and producer context. The inspected
producer is unmodified unp4k revision
`b492ab14d26280c6ec91c4365ff0faf5f3e24a6b`; its
[trim loop](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/ICSharpCode.SharpZipLib/Zip/ZipFile.cs#L3116-L3140)
runs before decompression and retains at least one byte. The classifier does not parse manifests,
prove producer success or excuse partial runs: those checks precede it in the reference harness.
An unmanifested run supplies no allowance context, so exact bytes may compare equal but missing
bytes remain differences. A local patch scope is not assumed equivalent to the inspected source.
The current public API observation does not verify hashes or prove reference completeness.

## Mutation self-tests

Without any variable, `mutation_bytes`, `mutation_unp4k`, `mutation_xml`, `mutation_value` and
`mutation_embedded` change a copy of
synthetic reference data and require every change to be detected: a byte changed, deleted or
appended at any block; a tag, attribute, text or child renamed, changed, swapped, deleted or
invented; an integer plus one, a single and a double by the smallest step, a boolean flipped,
texts swapped, an enum option, GUID or Reference changed, a value of another kind, an array
element deleted or invented, a field deleted or invented, and the Record name, GUID and tag
changed. They also check that `-0.0`, the short text of a single and whitespace around text are no
difference, and that each allow-list entry applies only to its case (`mutation_allow`,
`mutation_xml`, `mutation_bytes`): a weak pointer, a missing Reference, an invalid bool and a
non-finite number against every other kind of value, an array error or an unmapped kind against
`null`, `false` against an invalid bool, a finite number against `null` or `NaN`, `BUGGED`
against an empty value, and a difference in the case of a name, a GUID or a path. The content of
an embedded Record is mutated like a Record of its own (a value, name, type or GUID changed, a
field deleted or invented), also when it is nested inside another embedded Record, and an
embedded Record with no Record of P4KX to compare with is a difference. Every allowed value is
counted, not one reason per Record or tree.

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
