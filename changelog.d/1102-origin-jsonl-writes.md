### Changed

- Media export batches provenance JSONL writes per line, using bounded blocks
  for longer lines.
- Media export releases mutable source indexes before writing the final provenance inventories.
- Media source locations reuse text already stored in their full source-use references.
