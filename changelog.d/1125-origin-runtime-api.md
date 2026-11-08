### Changed

- Rust API: source-use handles share their original registered descriptions; `reference` and `role`
  borrow that row through ordinary methods and no longer support const evaluation. Existing source
  and use wire identities remain unchanged.
