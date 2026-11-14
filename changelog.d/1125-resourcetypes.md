### Changed

- Move ResourceType catalog data to schema-v3 `definitions.resourceTypes.rows`, with explicit value
  states and per-value encryption origin through shared physical sources and actual source uses.
  Preserve exact numeric widths and IEEE bits, and resolve all available supported languages through
  separate public-name and protected-description tables. Other catalog and tool-output families
  remain pending migration; global key status alone does not provide per-field origin.
