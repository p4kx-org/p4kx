### Changed

- Vehicle XML readers release temporary selector deduplication tables before caching the
  parsed result. Cached results retain the original selector order and file evidence.
