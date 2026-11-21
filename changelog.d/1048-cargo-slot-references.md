### Fixed

- Preserve distinct ResourceType cargo references when original slot declarations share a name.
  Address each slot's field origins by its original local position, while keeping resolved
  target-header origins separate.
