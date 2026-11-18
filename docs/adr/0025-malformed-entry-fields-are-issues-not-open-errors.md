# Malformed Entry fields are Entry issues, not open errors

Every byte of an Archive is untrusted, and one malformed field in 1.38 million directory records
should not make the whole Archive unreadable. Fields that concern a single Entry are therefore
reported as Entry issues; the open fails only when the directory itself is broken (end records,
trailer, sizes of the directory and name table, an unreadable header, an empty or unterminated path).

`Archive::issues()` returns `(position, EntryIssue)` pairs next to the Entries, in Entry order;
`Entry` has no issue field. A flagged Entry stays listed and fails with a typed error only when it
is read and its data is really unreadable.

The list is bounded: an Entry has at most one issue of each kind, `issues()` lists the first
`MAX_ENTRY_ISSUES` (1024), and `issue_count()` still counts all of them. The stored bytes of paths
that are not UTF-8 are kept apart from the list, one per such Entry and found by Entry position
(`Archive::raw_path(entry.index())`), so the cap never loses them.

## Decisions

- **Encryption.** The v1 CIG block 0x5002 and the v2 record field hold 0 (plain) or 1 (encrypted).
  Any other value is `UnknownEncryption` and the Entry is read as encrypted. This is the owner's
  decision, taken because the other readings are worse for the data that matters: ciphertext read
  as plain would pass as content without anyone noticing. It is not free of risk: a stored Entry
  that is really plain and carries an unknown value is decrypted into garbage, and nothing in the
  reader detects that yet, since a read does not verify checksums by default. `p4kx verify` and
  `extract --verify` close that gap (ADR-0026). A 0x5002 block that is not 6 bytes long (4 header,
  2 value) is `EncryptionBlockSize`; the value is still read when the block holds one, and a
  block too short for a value counts as encrypted.
- **Hash block.** A 0x5003 block that is not 36 bytes long (4 header, 32 SHA-256) is
  `HashBlockSize` and states no digest (ADR-0148).
- **Data range (v2).** An Entry whose data ends behind the start of the central directory or of the
  name table, whichever comes first, or whose end overflows, is `DataOutsideFile`. Opening it fails
  with the existing `Corrupt` error; the other Entries read.
- **Non-UTF-8 paths.** `Entry::path` holds the path with invalid bytes replaced by U+FFFD, for
  display and lookup. Extraction names such an Entry from the stored bytes: valid text as it is,
  `%` as `%25`, each invalid byte as `%` and two upper-case hex digits, in every writer (plain,
  converted, texture, DataForge, sound, Soundbank). Different bytes never share a name, which
  U+FFFD does not guarantee, and the result still passes the same traversal checks as every Entry
  path. A real file whose name literally spells such an escape could still collide; the Conflict
  policy then decides as for any two Entries with one path.
- **ZIP64 locator.** It is looked for right before the end record. If it is not there, or does not
  point at a ZIP64 end record, and the end record carries the 0xFFFF / 0xFFFFFFFF markers that call
  for ZIP64 records, the end region already read is searched backwards. A candidate counts only
  when the ZIP64 end record signature stands at the offset it names, before the locator.

- **ZIP end-frame ownership.** A signature inside the selected original comment prefix is comment
  data when the containing end route has a plausible own directory. Its available prefix remains
  meaningful even when the complete comment is truncated. Competing directory and Entry frames
  cannot also own that comment boundary; their proven overlap is a corrupt ambiguous layout.
  Later generations outside the earlier comment remain eligible. A directory declaring zero
  Entries and zero bytes starts at its following end frame, including the ZIP64 end-record start.
  Offsets remain relative to the owning source; no implicit executable-prefix rebasing is done.

## Considered Options

- Reject the Archive on the first malformed field: simplest, and what the v1 reader did for paths,
  but one bad record would hide the rest of a multi-gigabyte Archive.
- Treat unknown encryption values as plain, or as unreadable: the first passes ciphertext as
  content, the second loses an Entry that may decrypt.
- Decode non-UTF-8 paths with a code page as SharpZipLib does: the Archive does not state one, so
  the choice would be a guess, and the decoded names could still collide.
- Keep the raw bytes on every `Entry`: 16 bytes for each of 1.38 million Entries for a defect that
  does not occur in real Archives.
- Keep every issue: one 4-byte extra block can cost far more memory as an issue than it takes in
  the input, so an Archive could be made to exhaust memory through its issue list.
