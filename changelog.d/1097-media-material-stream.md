### Fixed

- Stream complete material source trees, surfaces and dependency lookups in budgeted JSONL batches,
  preserving original field origins and compatibility with earlier material records.
- Keep shader links bound to the original selected submaterial list when authored XML is only partially retained.
- Reuse the validated source context for material fields inside a batch.
