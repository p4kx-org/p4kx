### Changed

- Catalog schema v3 stores authored Quality distributions and quantizations in canonical
  definition tables with per-value source origins and exact numeric storage. A separate
  inverse projection retains first-eligible ResourceType relationships and explicit
  eligibility while preserving filtered dependencies and incomplete source evidence.
- Tagged catalog numbers enforce exact Int32 and UInt32 bounds; quantization eligibility
  statuses and reasons remain closed.
