### Changed

- All-Ship SDK queries release a parsed vehicle result after its sole possible use when the
  original root and cache key establish that lifetime. Reusable keys keep the original reads,
  parse outcomes and source positions.
