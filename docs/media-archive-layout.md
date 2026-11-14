# Archive source layout in media exports

The Archive provider emits source-layout payload version `1.0` inside the existing media `1.0`
record envelope. Its capability version is `1.3.0`. The same writer, streaming artifact manifest,
generation publication, source watch and output protection apply to payload and layout records.

Each already opened root or nested Archive receives one directory callback. Nested sources use
the existing physical parent-container identity and their own source length. They do not acquire
an invented Entry zero, a new identity format or a retained-source manifest row. `source.raw`
remains null because retaining metadata sections does not establish retention of a complete file.

## Records and exact bytes

| Namespace | Payload |
| --- | --- |
| `archive-layout` | Actual reader route, declaration availability and every exposed ZIP/ZIP64/v2 layout word |
| `archive-section` | Every known section's absence/declaration/availability, readable prefix and proved raw reference |
| `archive-raw-chunk` | Source-local series, exact source range and an existing verified U8 chunk artifact |
| `archive-directory-record` | Original record position/ordinal, optional admitted physical index, shared `EntryMetadataInfo` and raw subranges |
| `archive-extra` | Original field order, duplicate/unknown IDs, declared total length, framing state and raw/body ranges |
| `archive-installation` | Original progress/reserved words, exact row bytes and independent validation |
| `archive-free-range` | Original free-list words, exact row bytes and separate extent validation |
| `archive-metadata-diagnostic` | Structural lazy-reader failure without inventing missing records |

Offsets, lengths, source counts and raw 64-bit words in the new payloads use the shared
`DecimalU64` canonical decimal strings. They are relative to the immediately owning source, not
the backing root file. Existing envelope/Entry fields and typed chunk numeric meanings stay
unchanged. Source word widths of 8, 16 and 32 bits remain bounded JSON integers.

A raw reference names an `archive-raw-chunk` series belonging to the same source identity and an
exact source range. Its chunks occur in original byte order, without gaps; each references actual
sidecar bytes with their output length and SHA-256. Record names, comments, extras, 128-byte v2
signatures and all 32-byte digest slots refer to already retained parent sections. Duplicate
records and all-zero slots remain distinct authored observations. No algorithm, authentication,
text encoding or semantic trie interpretation is inferred for opaque bytes.

The retention map contains at most the fixed 17 section extents. Chunk and directory-record
references stream instead of accumulating in memory. Recovered surviving records receive their
own source-local raw series; no original ordinal or global layout is fabricated. Unknown bytes
inside established metadata sections remain usable bytes. Unproved gaps elsewhere are outside
the raw-preservation claim.

## Coverage, work and health

Directory callbacks do not increment physical Entry or provider Entry-callback tallies. Their
actual records use the existing record tallies; sidecars enter the common artifact manifest and
output-byte budget. Retained metadata does not increment `converted_outputs`. Include Filters
select Entry payloads; metadata of an opened source describes its complete original directory,
including omitted folders. Excluding a container still prevents opening that subtree.

The export-wide `source_bytes` counter admits every actual metadata-read request in addition to
retention and container-opening work. `metadata_work` separately admits each reader-reported
cursor/read work item across all owning sources and providers. The normative default is
100,000,000, omitted canonically in index limits; nondefault values remain explicit. The actual
cancellation flag reaches every metadata access. Source watches run during admission, between
chunks/rows and before publication. Metadata input reads never exceed 64 KiB.

Header availability does not validate lazy rows. Invalid reserved words, excessive progress,
outside/overflow free extents, malformed extras, unreadable declarations and unparsed tails
make the relevant record partial with a warning. Recovery remains partial. Normal export refuses
publication; diagnostic export retains available bytes in a noncomplete generation. Budget,
cancellation and source I/O errors retain their typed causes and abort publication. Previous
complete indexes remain current after failure.

An authored zero-length free extent remains an explicit `zero-size` observation without an
inferred engine error. Its original offset/length and row bytes remain present.

The positively recognised CIG-v1 row profile checks its reserved word against zero, matching
the [pinned Svarog reader](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/archive.rs#L2847-L2870).
This is a named supported-reader invariant, not a claim of native engine validation. Null
signatures, sector-size zero and calendar-invalid DOS words acquire no inferred malformed status.

`builtin_record_schema()` binds payload types, presence rules and authoritative status only to
these namespaces under kind `archive`. Other kinds and custom provider namespaces retain the
generic data contract. The shared Entry metadata is the same DTO used by CLI and MCP. The
CLI exposes `--max-metadata-work`. CLI `info --json` and MCP `archive_info` include the same
cached `ArchiveLayoutInfo` as their additive `metadata` field, without reading lazy records.
Their `fieldOrigins` records the observed owning directory or Container for each layout field.
Original header facts identify the source-local header extent and logical field. The marker follows
the containing Entries when reading an already opened nested layout. The root directory does not
inherit encryption from its Entry payloads. Unknown recovery declarations remain unknown, and the
tool-authored metadata version has a complete empty contributor set. Human-readable summaries
retain their existing lines and counting meanings.

The reader's [source metadata contract](archive-source-metadata.md) defines record ownership,
section extents, recovery limits and lazy-access failures.
