### Fixed

- MCP audio: inspect every indexed WEM candidate until metadata or decoding succeeds, so a
  readable sound after eight prefetched or malformed locations is found. Preserve candidate
  order, explicit-path selection and the existing error and excerpt limits.
  Prefetch-only errors describe the searched index scope.
