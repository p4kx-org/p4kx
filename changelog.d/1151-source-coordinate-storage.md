### Changed

- Catalog preparation retains common DataForge source coordinates in compact form. It preserves
  the source-use IDs, coordinate spelling and JSON Schema. The Rust
  `SourceUseDescription::location()` getter still returns a borrowed `SourceLocation` and is now
  evaluated at runtime.
