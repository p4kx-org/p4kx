### Added

- Media exports commit shared source/use inventories and occurrence-scoped field origins as
  separate hashed streams, preserving existing payload bytes and budgets. Archive and Shader
  fields retain their actual metadata, payload and containing-Entry encryption domains;
  unmigrated profiles and historical missing origins remain explicit.
  Generation reuse validates the original byte domains and every addressed payload occurrence,
  keeping absent, malformed and unreadable source facts distinct.
