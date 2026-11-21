### Fixed

- Archive opening preserves the outer directory when binary ZIP comments contain end-record
  signatures, refuses conflicting physical frames, and reports missing outer end records instead
  of certifying a nested empty ZIP as a complete empty Archive.
