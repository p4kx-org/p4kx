### Changed

- Replace the legacy mining-global and refining root arrays with canonical v3 tables under
  `configurations.miningGlobal` and `definitions.refiningProcesses`, preserving complete original
  owner arenas, every existing numeric field, exact numeric widths/bits, text domains and per-datum
  provenance. Missing, null and unsupported values keep distinct states.
- Add direct typed SDK queries for selected mining globals and refining processes, including GUID
  aliases and physical Record selection, without building a Catalog or reading Resource/world
  bodies. Refining retains requested protected text translations and result-local source evidence.
