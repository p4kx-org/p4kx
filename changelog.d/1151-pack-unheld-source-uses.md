### Changed

- Catalog preparation stores unheld DataForge source-use rows in compact memory after each
  record window and at selected projection boundaries while preserving their identities,
  coordinates and per-value encryption. The publisher reuses the sorted packed index when
  sealing its provenance inventory. The packed map entry uses 40 bytes on 64-bit targets.
