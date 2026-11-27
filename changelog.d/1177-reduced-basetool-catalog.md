### Added

- Added `p4kx catalog basetool <archive> --out <file.json.zst>` and the Rust SDK's
  `write_basetool_catalog_file` for the fixed Basetool selection. Format v1 keeps
  canonical v3 record identities, English and German text and per-value encryption
  markers. Publication verifies the selected dependencies and enforces separate
  compressed and decoded size limits.
