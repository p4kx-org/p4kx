### Changed

- Catalog v3 and the resolved Loadout SDK carry per-value origin for the original owning GUID and
  the Ship or Item selection. Catalog consumers read their join keys from `entityGuid.value` and
  `entityKind.value`.
