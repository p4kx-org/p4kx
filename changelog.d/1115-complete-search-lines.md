### Fixed

- Search: inspect complete admitted text lines, including matches and invalid UTF-8 after the
  first mebibyte, while retaining bounded literal and excerpt state. Preserve original line
  offsets, Unicode matching and explicit size/result limits; keep a cancellation request
  authoritative even when a later callback returns false.
