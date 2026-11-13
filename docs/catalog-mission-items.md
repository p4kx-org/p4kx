# Mission item definitions

Catalog schema v3 lists each selected physical `MissionItem` Record in
`definitions.missionItems.rows` in GUID and Record-index order. The physical
Record header identifies the exact source, GUID, class name and record path.
A Record filter affects selection. It does not change the original stored
identity of a reference.

| Catalog field | Original field |
| --- | --- |
| `tags` | Ordered `MissionItem.tags[]` References to `Tag` |
| `entityClass` | `MissionItem.entityClass` Reference to `EntityClassDefinition` |
| `weighting` | Original `MissionItem.weighting` Single and its exact bits |
| `stringVariants` | `MissionItem.stringVariants`, an owned `MissionStringVariants` |
| `stringVariants.value.variants` | Original `MissionStringVariant` array in physical order |
| `variants.value[].value.tag` | Original optional variant Tag Reference |
| `variants.value[].value.string` | Original Locale key and protected GameText reference |

Each source-backed field or list element carries its immediate state, encryption
marker and exact use references. The original locale key and each resolved
translation remain in the protected text dataset. Definition fields outside
the typed projection remain in their owner's `authoredFields` arena.

These rows describe possible source definitions. The original
`MissionPropertyValue_MissionItem` retains its authored count bounds, match
conditions and debug-only forced item in the property source. A stored debug
reference supplies no runtime item selection.
