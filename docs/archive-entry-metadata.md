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

This reader step retains directory timestamps. Local-header timestamps, further
layout/signature/extra-range metadata and encoded CLI/MCP/media projections are
separate enrichment work; the full metadata-export issue remains open.

The packed field definition follows
[Microsoft's DOS date/time contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime).
