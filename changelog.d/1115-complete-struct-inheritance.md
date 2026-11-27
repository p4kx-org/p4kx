### Fixed

- Retain valid DataForge inheritance chains beyond 64 Structs in Record layouts, Queries,
  completion, schema drift counts and coverage; report actual repeated Struct indices as cycles.
  Remove the obsolete public `MAX_INHERITANCE_DEPTH` constant and unreachable
  `DataForgeError::InheritanceTooDeep` variant. Owned value nesting remains a separate policy.