### Changed

- Catalog exports default to the versioned marker document. Catalog SDK queries also default to
  `MarkersOnly`. Serialized source values carry `state`, `value`, and
  `containsEncryptedSource`. Consumers should accept missing `sourceUsesRefs` and
  `sourceSetComplete`. Full v3 source proofs remain available through explicit provenance
  selection.
