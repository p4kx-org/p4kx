### Fixed

- Use the Entity's authored Material node for its main geometry and preserve the winning
  field in source provenance. An empty Material node uses the geometry's material path.
- Resolve a Material node's tint palette before the same component's geometry palette.
  Keep the winning field and report unreadable references before falling back.
