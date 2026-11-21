# unp4k and unforge reference audit

Research date: 2026-11-06. P4KX source baseline:
[1ee9c0edbfc34e616d2f140951e4eb5bc1c67558](https://github.com/p4kx-org/p4kx/tree/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558).
The official unp4k release was
[v4.0.87](https://github.com/dolkensp/unp4k/releases/tag/v4.0.87), published on 2026-05-23;
its target and the inspected main branch both resolve to
[b492ab14d26280c6ec91c4365ff0faf5f3e24a6b](https://github.com/dolkensp/unp4k/commit/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b).
The repository's root [licence](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/LICENSE.txt)
is MIT. This report describes facts and interfaces; no foreign implementation code was added to P4KX.

## Scope and execution

The audit inspected the pinned archive reader, CryXML converter, DataForge schema and XML writers,
command-line entry points, and read-only virtual filesystem. A private source build produced manual
reference output for the installed PTU Archive. Foreign tools were run outside the tests; the
existing P4KX tests read their output folders, as [ADR-0234](adr/0234-differential-verification-reads-reference-output-from-folders.md)
requires. All tools, caches, probes and extracted files stayed below the task-specific Codex scratch
folder. Nothing was written to a game Installation.

The original [unp4k command-line catch block](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unp4k/Program.cs#L58-L91)
can send an HTTP exception report. Before execution, the private build inserted an unconditional
exception immediately before its HTTP-client allocation, inside the existing reporting try/catch.
The compiler reported the remaining reporter as unreachable. That was the only source change:
archive parsing, filters, decryption, decompression and extraction were unchanged. unforge was built
without source changes. Both were built for net8.0 with .NET SDK 9.0.320 and run on the installed
.NET 8 runtime; the official Windows release workflow targets net10.0. Therefore these are runs of
the pinned source, rather than claims about the downloaded release executable.

The bounded extraction selected all DCB, chrparams and CDF Entries. Raw output remained separate
from copied XML-conversion inputs. The unp4k runs reported no per-Entry extraction errors;
unforge reported no explicit conversion errors.

| Operation | Result | Time |
| --- | --- | --- |
| unp4k DCB extraction | 1 file, 349,063,293 bytes | 5.70 s |
| unp4k chrparams extraction | 4,209 files | 9.58 s |
| unp4k CDF extraction | 1,711 files | 6.10 s |
| Initial mixed unforge conversion | 5,804 distinct XML output paths from 5,920 sources | 35.59 s |
| Independent CDF-only unforge conversion | 1,711 XML outputs from 1,711 sources | 9.21 s |
| Bounded media extraction | 34 files, 225,369 bytes from 27 selected paths and their prefix companions | No extraction errors |
| P4KX raw-byte comparison | 5,921 equal, 0 different, 0 allowances | Passed |
| P4KX media raw-byte comparison | 34 equal, 0 different, 0 allowances | Passed |
| Initial mixed P4KX CryXML comparison | 5,804 equal, 0 different, 0 allowances | Passed with the coverage limit below |
| Isolated chrparams P4KX CryXML comparison | 4,209 equal, 0 different, 0 allowances | Passed |
| Isolated CDF P4KX CryXML comparison | 1,711 equal, 0 different, 0 allowances | Passed |

The mixed XML count is a collision count, not a count of unsupported inputs. There are 116 pairs
of `.cdf` and `.chrparams` with the same directory and stem. All 232 members are CryXML, and none
of the 116 raw pairs is byte-identical. Each pair writes the same `<stem>.xml`. Separate source-type
folders are required to cover both inputs. No concrete PTU asset names or values are included here.

The media subset includes model/skin companions, DDS and alpha headers, soundbanks, WEM and socpak
containers, one CAF, one DBA, one AIM and two encrypted Entries. Metadata selection through the
unchanged upstream ZipFile reader independently counted 54,391 CAF, 546 DBA, 551 AIM and 4,344
encrypted Entries. This is a directory census, not an exhaustive payload check. The selected
animation Entries were at most 4 MiB and encrypted Entries at most 16 MiB; their actual combined
size was 6,722 bytes. All other media sampling remained bounded by count and declared size.
The final 34-file comparison comprised 6 Store/plain, 26 method-100/plain and 2 method-100/encrypted
Entries. Every file was byte-identical; no trimming allowance was exercised by these real samples.

unp4k's filter uses substring matching and also includes DCB files when the filter ends in `xml`.
A full-path XML filter therefore produced an additional DCB copy; that unintended copy was identified
and removed from the media-only tree before comparison. The raw DCB reference remained in its own
folder. Successful process exit and a filter string alone are insufficient provenance for a reference
artifact set. The upstream rule is visible in
[Program.Main](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unp4k/Program.cs#L27-L33).

## What the source comparison establishes

| Area | Pinned upstream source | P4KX conclusion |
| --- | --- | --- |
| Compression | [GetInputStream](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/ICSharpCode.SharpZipLib/Zip/ZipFile.cs#L738-L795) supports Store, raw Deflate and method 100; method 100 passes through when its magic is not a Zstd frame | Already implemented in [entry_reader.rs](../crates/p4kx-archive/src/entry_reader.rs) and [decode.rs](../crates/p4kx-archive/src/decode.rs); no new unsupported method was established |
| Archive encryption metadata | [IsAesCrypted](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/ICSharpCode.SharpZipLib/Zip/ZipEntry.cs#L244-L250) checks a fixed extra-data byte; the ZIP reader also reads ZIP64 metadata | P4KX reads explicit CIG blocks and reports malformed sizes/values in [central_directory.rs](../crates/p4kx-archive/src/central_directory.rs); completed #282 already owns this robustness work |
| Decryption length | [CreateAndInitAesDecryptionStream](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/ICSharpCode.SharpZipLib/Zip/ZipFile.cs#L3116-L3140) buffers and strips trailing zero bytes before decompression | P4KX streams decryption and respects the declared logical size; stripping genuine logical zeros is an upstream defect, not behavior to copy |
| Value kinds | [Enums.cs](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge/Enums.cs) declares primitive integers, floats, strings, locale, enum, GUID, class, strong/weak pointer and Reference kinds | These kinds are present in the canonical P4KX reader; no additional current PTU value kind was identified from unforge |
| Conversion flags | [DataForgePropertyDefinition](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge/ComplexTypes/DataForgePropertyDefinition.cs#L18-L25) masks conversion to its low byte and reads a final raw u16 | P4KX already separates conversion and high-byte flags, reporting the flags through DataForgeIssue; no meaning for the final raw word is established by unforge's unused VariantIndex name |
| Record metadata | [DataForgeRecordDefinition](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge/ComplexTypes/DataForgeRecordDefinition.cs#L16-L42) emits name, type, GUID, path and v8 team metadata | P4KX [RecordTree](../crates/p4kx-dataforge/src/tree.rs), [Record XML](record-xml.md) and [Record JSON](record-json.md) already retain the corresponding identity and tag; different attribute names are a dialect difference |
| Shared record paths | [DataForge constructor](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge/DataForge.cs#L139-L152) keeps only the last Record in PathToRecordMap | P4KX retains all Records using [ADR-0008](adr/0008-record-files-by-record-path.md); the upstream overwrite must not become a completeness rule |
| Bounds and errors | [DataForge](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge/DataForge.cs#L262-L311) and [array reading](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge/ComplexTypes/DataForgeStructDefinition.cs#L201-L216) silently stop at configured node/depth limits or omit empty/null structures | P4KX's bounded canonical-tree error states remain authoritative; upstream omission is not evidence of an empty source value |
| Legacy DCB | [DataForge constructor](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge/DataForge.cs#L92-L137) has a pre-v6 legacy heuristic | [ADR-0136](adr/0136-dataforge-version-5-and-older-are-not-supported.md) deliberately limits P4KX to v6/v8; no new legacy-support issue is justified by this audit |

The upstream [spec.md](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/spec.md)
is derived documentation. Its table labels and Record layout do not fully match the current C#
reader, so the owning reader, not those names alone, was used to compare physical layouts.
No semantic claim is made for unknown words merely because an upstream property has a name.

## Media and filesystem boundaries

[unp4k.fs](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/README.md#unp4kfs---virtual-filesystem)
mounts a P4K or DCB through Dokan. [CompressedFileSystem](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unp4k.fs/CompressedFileSystem.cs)
serves raw Entries, attempts CryXML-to-XML conversion, and projects DCB records through
[DataForgeFileSystem](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unp4k.fs/DataForgeFileSystem.cs).
It is another access interface to those formats, not an independent semantic media converter.
The [unp4k CLI](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unp4k/Program.cs#L23-L54)
copies Entry streams; the [unforge CLI](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/unforge.cli/Program.cs#L77-L113)
converts CryXML and DCB to XML. Neither inspected path implements model-to-GLB, DDS image decoding,
animation evaluation or WEM/BNK audio decoding. Reading those files byte-for-byte can validate
archive extraction, but cannot validate mesh coordinates, material appearance, animation conventions
or acoustic output. No new media feature parity claim or mount requirement follows from this source.
Existing [media export work](ptu-media-compatibility-audit.md) remains the relevant implementation scope.

## Confirmed verification defects

Follow-up overview: [#1103](https://github.com/p4kx-org/p4kx/issues/1103), milestone v0.5.0.

### Ambiguous source mapping and incomplete coverage

Tracking: [#1104](https://github.com/p4kx-org/p4kx/issues/1104).

[unforge::source_of](../crates/p4kx-extraction/tests/reference/unforge.rs) chooses a chrparams sibling
before a CDF sibling for every XML output. That is ambiguous when both exist. The initial passing
run consequently covers 4,209 chrparams and 1,595 distinct-stem CDF sources, while the other 116 CDF
sources have no independently identified output in that mixed run. The independent source-type runs
subsequently covered all 5,920 source trees with zero differences or allowances. A passing count of output files does not establish
complete source coverage.

A reference manifest must identify the tool revision, source Archive identity, exact selected Entry,
expected conversion, artifact path and conversion status. Same-stem conflicts must be isolated or
rejected rather than guessed from an extension priority. Missing, failed, empty and intentionally
unsupported outputs must remain distinguishable. Synthetic acceptance cases should contain two
nonidentical CryXML sources with one stem, duplicate/ambiguous manifest identities, a missing output,
an unexpected output, a wrong Archive fingerprint and a configured empty reference folder.
Foreign tool execution remains manual and outside tests/CI.

### Zero-tail allowance lacks decoder context

Tracking: [#1105](https://github.com/p4kx-org/p4kx/issues/1105).

[unp4k::judge](../crates/p4kx-extraction/tests/reference/unp4k.rs) allows any shorter reference when
all additional P4KX bytes are zero and the Entry is encrypted. Its input has no compression or effective
decoder context. Upstream trimming happens before its Deflate/Zstd decoder, so it does not justify
accepting a missing suffix of logical zeros after successful decompression.

A private probe imported the unchanged P4KX comparison modules by absolute path. It built four synthetic
encrypted Entries with the existing testkit helper: Store, Deflate, a real Zstd frame and method-100
pass-through. Each decoded to 64 nonzero bytes followed by 33 logical zero bytes. For a reference
mutation that deleted those 33 bytes, the unchanged judge reported `allowed=1, different=0` in every
case, including Deflate and real Zstd. This reproduced a comparator false allowance; it did not claim
that unp4k produced those mutated references or that P4KX's decoder lost bytes.

The fix must preserve the known allowance for encrypted Store and method-100 pass-through where the
exact upstream trimming behavior explains the difference. Actually decoded Deflate/Zstd zero-tail
mutations must fail. Do not cap the allowance at 15 bytes: upstream can wrongly strip arbitrarily
long runs of genuine logical zeros on pass-through data. Cover all-zero, one-byte and remaining-zero
boundaries as well as nonzero changes, reference-longer data, unencrypted data and exact equality.

## Limits

The executed byte comparison covers the selected files, not every Entry in the Archive. CryXML tree
comparison covers structural XML values, not proprietary engine behavior or an in-game rendering.
The DataForge source review does not establish equivalence for every Record against unforge's lossy
XML dialect. The virtual filesystem was inspected but not mounted. No P4KX implementation code was
changed, and no new archive-format parsing regression was demonstrated.