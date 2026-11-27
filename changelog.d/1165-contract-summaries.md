### Added

- The SDK can return canonical contract summaries with blueprint and reputation rewards, giver,
  offer locations, sharing, authored location-query links and localized titles without building
  the original Generator arenas or publishing the property and NPC spawn text tables.
- Contract summaries also carry the canonical debug name, fixed payouts, cooldowns, buy-in,
  reacceptance, standing bounds, authored scoped reputation requirements and protected descriptions.
  The `notForRelease` and `workInProgress` flags use the same parent values as full contracts.
- Both summary queries have direct SDK entry points for an Archive or its decoded DataForge Entry.
