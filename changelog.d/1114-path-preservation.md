### Fixed

- Windows MSI: preserve the user's PATH existence, Registry string type and exact bytes
  across installation, repair, replacement, rollback and removal using a private Rust
  helper. Keep foreign PATH edits and require native raw-value checks on both CPUs.
