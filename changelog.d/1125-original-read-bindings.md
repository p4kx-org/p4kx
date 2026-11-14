### Fixed

- Loadouts reject a supplied canonical tree from another DataForge even when its GUID and
  Record position match. Original field declarations and resolved target bindings remain
  available without changing value equality or canonical Record JSON/XML.
- Observed loadout reads retain original admission and actual cached read/parse failures;
  reuse does not invent additional operations, and source-evidence failure remains fatal.
