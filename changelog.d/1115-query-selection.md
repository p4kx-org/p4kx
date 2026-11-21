### Fixed

- Query: evaluate wildcard conditions over complete source arrays without losing matches at a
  projected-value limit. Required value-reading failures now report the original Record and
  comparison instead of a successful empty selection; cancellation drops partial selections.
  CLI, GUI and MCP preserve prior exports and cache only completed selections.
