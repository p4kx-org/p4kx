### Changed

- Export original physical crafting-global configuration rows under `configurations.craftingGlobal`
  with per-datum origins, default Blueprint selections, ordered nullable exclusion references and
  additional original fields; replace the merged root view.
- Query the same crafting globals directly through the Archive, decoded-Entry, session and parsed
  SDK without building a Catalog or reading Blueprint, resource or localization payloads.
