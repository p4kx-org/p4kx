### Changed

- Export Blueprint pool definitions, original ordered reward slots, and exact authored weights
  with per-value origins. Separate weight projections and incoming reward references into their
  own tables while retaining physical pool identity.
- Read selected pool definitions and weight projections through the Archive, decoded-Entry,
  session and parsed SDK without building a complete Catalog.
