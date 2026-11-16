### Changed

- Reduce catalog projection memory by sharing Archive Entry receipts and materializing only selected Loot rows.
- Include projection boundaries and blocking diagnostic counts in opt-in catalog phase reporting.
- Share the canonical Tag tree and original field inspections across catalog tag paths, mission dependencies and paint matching.
- Reuse the Archive file index during loadout resolution and release its worker and proof caches after collection.
- Merge mission dependency results in original order after each read window and reuse the pending result buffers.
- Fold contract generator results in original order per read window and report the active mission projection phase.
- Reuse original Struct and array positions for mission source lookups instead of storing a second index of every occurrence.
- Coalesce repeated whole-Record inspections during loadout resolution while retaining their source evidence.
- Release duplicate field inspections as Record scan batches finish and index source uses with their existing IDs.
- Collect selected Record rows directly into their catalog tables as each read window finishes.
- Share common Commodity fallback source evidence across ResourceType rows.
- Track mission template source membership by original Record ranges.
- Allocate Record failure details only for observations with a diagnosis.
- Share empty origin markers and store literal DataForge field paths in their compact source identifiers.
- Stage contract generator and broker definitions as their Record windows complete.
- Stage particle libraries and weapon animation candidates as they are projected.
