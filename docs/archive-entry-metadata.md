# Archive Entry source metadata

`p4kx-archive` exposes read-only directory facts through `Entry::metadata()`. The
`EntryMetadata` object has private fields and currently provides `dos_timestamp()`.
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

## Shared export representation

`p4kx-extraction::EntryMetadataInfo::of(entry)` encodes these facts for frontends.
The Archive media provider writes that same object at `data.metadata` on each
`archive-entry` record in `archive.jsonl`:

| Field | Meaning |
| --- | --- |
| `owner_entry_index` | The admitted physical Entry index in its immediately owning Archive |
| `dos_timestamp.raw_date` | Unmodified unsigned 16-bit directory date word |
| `dos_timestamp.raw_time` | Unmodified unsigned 16-bit directory time word |
| `dos_timestamp.calendar.state` | `valid` or `invalid`, separately from source-word presence |
| `dos_timestamp.calendar.year`, `month`, `day`, `hour`, `minute`, `second` | Present for `valid`, without a timezone or epoch interpretation |

Read the owner index together with the record's complete source/container identity;
it is not a global identifier or an original directory ordinal before folder removal.
A grouped texture row must keep the physical base Entry as metadata owner instead
of implying that one timestamp describes every part.

Raw words are exact JSON integers in the range 0–65535. All-zero words encode as
`calendar: {"state":"invalid"}` and remain present; they do not become a missing
source field or a fabricated 1980 calendar. Invalid calendar input does not mark
otherwise complete metadata or verified payload bytes as corrupt. The typed schema
bounds calendar fields and restricts seconds to even values from 0 through 58.

The generic media envelope remains available through `record_schema()`.
`builtin_record_schema()` also constrains this metadata object only for the built-in
Archive Entry namespace; other provider namespaces keep their general contract.

This projection covers directory timestamps. CLI/MCP Entry summaries, local-header
timestamps and further layout/signature/extra-range metadata remain separate parts
of the full metadata-export work.

The packed field definition follows
[Microsoft's DOS date/time contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime).
