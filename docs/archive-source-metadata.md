# Archive source metadata

`Archive::metadata()` exposes read-only declarations from the layout route actually used by
the reader. `Archive::metadata_view()` binds bounded metadata reads to that same source and
its admitted physical Entries. Neither operation copies complete metadata bodies into memory.
The shared CLI/MCP Entry projection still contains the fixed directory facts documented in
[Entry metadata](archive-entry-metadata.md); this reader API does not itself add raw artifacts
to an export.

## Coordinates and presence

Every `MetadataRange` uses offsets relative to its immediately owning source. For a stored
or decoded nested container, `source_len()` is that container's length; `Archive::file_len()`
continues to describe the root backing file. Original directory ordinals precede folder
admission and reindexing. A directory record omitted from a nested inventory keeps its raw
bytes and ordinal without acquiring an admitted Entry index.

`MetadataRegion` separates the original offset/length declaration from its checked available
prefix and status: complete, truncated, outside the source or overflowing. Empty declarations
remain present. `span()` requires a complete extent; `available_span()` exposes a prefix with
its explicit full-declaration status. A declared free range describes source words and grants
no access to unrelated payload bytes.

Actual recovery reports `RecoveredUnknown`, absent global layout descriptors and
`UnavailableLayout` for global sections. Surviving recovered record spans remain readable;
their original directory ordinals remain unknown. Requesting recovery for an intact Archive
does not erase its established layout.

## Layout facts

| Layout | Available declarations and metadata sections |
| --- | --- |
| ZIP | Original end-record disk/count/size/offset words, directory bytes, binary comment and bytes following a complete comment |
| ZIP64 | Selected locator and its actual position, original end-record words and declared extensible data |
| Recognised CIG v1 | Exact 16-byte ZIP64 comment with `CIG\0\x01\0` prefix, sector/free-count words, twelve-byte progress rows and sixteen-byte free rows |
| P4K v2 | Base/extension directory and name declarations, payload end, free count, trie offset/size, sector size, flags, version, magic and both distinct opaque 32-byte trailer slots |

ZIP end selection treats signatures inside a plausible earlier end record's declared available
comment prefix as comment bytes. A competing original directory or Entry frame owning the same
boundary is an explicit corrupt-layout error. Appended generations outside earlier comments and
opaque bytes following a selected comment keep their existing interpretation. Final empty routes
and possible comment-owner boundaries are also checked against the existing
ZIP tail. Only a canonically complete directory record with the exact original local-header name
and a checked stored extent before that record can establish a conflicting physical Entry frame.
The shared local-header matcher preserves actual source I/O failures; malformed or unbound raw
signature candidates do not become Entry authority. Ownership checks share one lazily loaded
lookback before that tail, bounded by the format's maximum central-record length
(`46 + 3 * u16::MAX`). The original EOCD search window is unchanged.
Both the original first end frame and classic END are checked against the same records. A
ZIP64 start outside the held range uses a separate format-bounded window around that original
position, retaining at most one such remote window and reusing overlapping held bytes. These
bounded metadata reads do not scan the gap between windows, the whole Archive or decoded
payloads. Empty directory positions use the actual first end-frame start, including the
ZIP64 end record, in owning-source coordinates; executable prefixes are not
implicitly subtracted from offsets. These rules follow the record and offset structure in
[PKWARE APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT), sections 4.3.1,
4.3.6, 4.3.16 and 4.4.24 through 4.4.26. Bounded frame validation does not establish authenticity
or a unique interpretation of every intentionally constructed ZIP polyglot.

Unknown ZIP comments remain raw comment bytes and do not imply installation tables. Invalid
recognised table arithmetic is explicit rather than successful absence. v2 entry records
expose all 128 signature bytes and all 32 stored digest-slot bytes, including zero slots.
These are stored facts: no signature authentication or hash algorithm is inferred for opaque
slots. The existing v2 installation refusal and payload verification policies remain separate.

## Streaming and errors

`MetadataRecords`, `MetadataExtras` and `MetadataTableRecords` advance one bounded record at a
time. ZIP record framing reuses the ordinary fixed-header parser. Ordered extras retain
unknown and duplicate IDs; the reader's CIG extra length includes its four-byte header.
Malformed framing retains available bytes and status, then refuses continued proven iteration.
Declared counts leaving uninterpreted directory bytes report `UnparsedDirectoryTail`.

Installation rows retain their raw progress, optional reserved word and separate comparison
against the corresponding admitted Entry's stored size. Omitted Entries have an unavailable
bound. Free rows retain original offset/size words and separate zero-size, overflow or
out-of-source status. Header `Available` status does not establish validity of every lazy row.

`MetadataAccess` borrows the caller's real cancellation flag and one caller-owned
`FnMut(MetadataCost) -> io::Result<()>` budget adapter. It creates no counters. Callers must
share the same export-wide accounting across root and nested views. Reads charge requested
bytes and work before I/O, check cancellation around admission and I/O, and accept at most
`MAX_METADATA_CHUNK` (64 KiB) of caller-owned storage per call.

A span privately borrows its exact source. Foreign spans, foreign or cloned Entry objects,
out-of-range reads and oversized chunks are refused. Subspans only narrow existing access.
Pointer values are never serialized or used as durable source identifiers. Parser, source I/O
and caller budget failures retain their original causes. Metadata reads do not establish a
source snapshot; an export must also retain its existing ArchiveWatch and publication checks.

The independent synthetic tests cover ZIP/ZIP64/v2, recovery, nested stored/decoded sources,
ordered and malformed extras, exact opaque bytes, shared budgets, cancellation and I/O errors.
No installed game data is included in those fixtures.
