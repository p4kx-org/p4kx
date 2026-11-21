# Read and index each language in full by default

## Context

A language's `global.ini` is an encoded text Entry. Its syntax places no fixed limit on the Entry
length, the decoded UTF-8 length or the number of distinct lookup keys. UTF-16 decoding and
replacement of invalid sequences can change the byte count. Key indexing merges ASCII case variants
and the plain and suffixed forms of the same key.

## Decision

- Catalog, CLI and GUI ingestion read and index each selected language in full. Archive Entry
  readers still reject disagreement between declared and actual Entry length.
- Library callers may supply `LocalizationLimits` for source bytes, decoded UTF-8 bytes and distinct
  keys. Each limit is optional and applies per file. The source limit includes any byte order mark.
  The decoded limit counts UTF-8 bytes after removal of the leading byte order mark. The key limit
  counts distinct lookup keys after case folding and suffix handling. A limit violation returns an
  error, so no decoded file or key index is returned as complete after a budget cutoff.
- The GUI uses 32-bit row identifiers and key offsets to keep its shared language table compact.
  Values that cannot fit those coordinates and failed allocation requests produce distinct errors.
