# Archive Entry source metadata

`p4kx-archive` exposes read-only directory facts through `Entry::metadata()`. The
`EntryMetadata` object has private fields and exposes the actual DOS words, source
record format, original directory ordinal, raw compression method and format-specific
fixed header words.
It is the common lower-layer seam for later metadata enrichment; no frontend or
export dependency is required by the reader.

The `DosTimestamp` always retains the complete unsigned 16-bit `raw_date()` and
`raw_time()` words. They come from the v1/ZIP central directory or the v2 fixed
directory record, rather than the local header or the host file's modification
time. Both known layouts contain these words even when all their bits are zero.
They remain present when their calendar encoding is invalid.

`calendar()` returns `Some(DosCalendar)` only for a valid calendar date and clock
time. Its year, month, day, hour, minute and second getters have no timezone, UTC
offset or epoch interpretation. The date uses years 1980–2107 and Gregorian leap
rules; 2000 has 29 February, 2100 does not. The time has two-second resolution, so its
valid seconds are 0, 2, …, 58. Encodings for 60/62 seconds, invalid days/months, hours
above 23 and minutes above 59 return `None`. All-zero words also return `None`;
time word 0 with a valid date instead means valid midnight.

Calendar validity is independent of Archive integrity and installation state.
Invalid source words are preserved without adding an Entry issue, concealing a
recovery diagnostic or changing CRC/SHA verification and installed-byte progress.
Opening a nested ZIP retains the metadata of each admitted physical Entry even
when folder records are removed and file indices change.

The independent synthetic fixtures write source words and expected calendar
components separately. They exercise v1, ordinary ZIP, recovery, v2 base/extension
and extension-only records, plus stored and compressed nested ZIPs. They do not use
the production decoder to generate expected values and contain no game data.

## Fixed directory fields

`record_format()` distinguishes a ZIP central-directory record (including P4K v1)
from a fixed P4K v2 record. `raw_compression_method()` retains the complete unsigned
16-bit method word independently of decoder selection.

`original_directory_ordinal()` is the zero-based source position before container
folder removal and admitted-Entry reindexing. v2 extension records follow all base
records. Recovery-scanned headers return `None`: their original table positions
cannot be established from the surviving records. Requesting `recover(true)` does
not erase known positions when the intact directory is read normally.

For ZIP records, `zip_header()` returns the original version-made-by and
version-needed words, all general-purpose flags, the 16-bit disk-start word and
all internal/external attribute bits. A ZIP64 marker remains the raw 16-bit word.
Host-system bytes and unknown bits are not reinterpreted, masked or replaced by
supported-feature values. v2 records instead provide `raw_v2_encryption_word()`,
including zero and unknown nonzero words; ZIP records return `None` for this field.
ZIP-only fields are absent on v2, while the actual DOS words remain present on both.

These facts do not change compression selection, the CIG encryption-block policy,
v2 nonzero-encryption interpretation, Entry issues, checksum verification,
installation flags or installed-byte progress. Local-header words, signatures,
ordered extra blocks and complete layout/range metadata are separate field groups.

## Shared export representation

`p4kx-extraction::EntryMetadataInfo::of(entry)` encodes the same directory facts
for frontends. CLI `list`, `search` and `socpak list` attach this object at
`metadata`; MCP `list_entries` and `search_entries` use the identical object.
The Archive media provider writes that same object at `data.metadata` on each
`archive-entry` record in `archive.jsonl`:

| Field | Meaning |
| --- | --- |
| `owner_entry_index` | The admitted physical Entry index in its immediately owning Archive |
| `dos_timestamp.raw_date` | Unmodified unsigned 16-bit directory date word |
| `dos_timestamp.raw_time` | Unmodified unsigned 16-bit directory time word |
| `dos_timestamp.calendar.state` | `valid` or `invalid`, separately from source-word presence |
| `dos_timestamp.calendar.year`, `month`, `day`, `hour`, `minute`, `second` | Present for `valid`, without a timezone or epoch interpretation |
| `record_format` | `zip_central_directory`, `p4k_v2`, or `unknown` for a reader format this projection does not interpret |
| `original_directory_ordinal` | Canonical unsigned decimal string for the source record position; `null` for recovery-scanned headers |
| `raw_compression_method` | Unmodified unsigned 16-bit compression method, independent of decoder selection |
| `zip_header` | Fixed ZIP directory words below; explicitly `null` on v2 |
| `raw_v2_encryption_word` | Unmodified unsigned 16-bit v2 word, including zero and unknown values; explicitly `null` on ZIP |

`zip_header` contains `raw_version_made_by`, `raw_version_needed`,
`general_purpose_flags`, `disk_start`, `internal_attributes` and
`external_attributes`. The first five retain unsigned 16-bit words; external
attributes retain all 32 bits. A ZIP64 disk marker is not replaced by a guessed
decoded disk number. These JSON integers are exact throughout their source width.

`DecimalU64` represents new unsigned 64-bit source facts as strings from `"0"`
through `"18446744073709551615"`. The schema rejects leading signs or zeroes,
whitespace, numeric JSON values and overflow. Existing owner indices, byte counts
and other numeric fields retain their types. Source ordinals belong to the
immediately owning Archive, including the combined v2 base/extension order.

Read the owner index together with the record's complete source/container identity;
it is not a global identifier or an original directory ordinal before folder removal.
A grouped texture row keeps its physical base Entry as metadata owner. Its old
sizes and `parts` remain group aggregates; metadata is not combined across parts.
With `--parts`, each physical part instead owns its own metadata object.

Raw words are exact JSON integers in the range 0–65535. All-zero words encode as
`calendar: {"state":"invalid"}` and remain present; they do not become a missing
source field or a fabricated 1980 calendar. Invalid calendar input does not mark
otherwise complete metadata or verified payload bytes as corrupt. The typed schema
bounds calendar fields and restricts seconds to even values from 0 through 58.

The generic media envelope remains available through `record_schema()`.
`builtin_record_schema()` also constrains this metadata object only for the built-in
Archive Entry namespace; other provider namespaces keep their general contract.

This projection covers directory timestamps and fixed directory words. Local-header
timestamps and further layout/signature/extra-range metadata remain separate parts
of the full metadata-export work. Invalid or unknown raw words do not suppress
the authoritative recovery, installation or integrity status.

The packed field definition follows
[Microsoft's DOS date/time contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime).
