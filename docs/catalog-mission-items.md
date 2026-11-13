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

## Title and description previews

The existing sub-contract, contract, handler and template property precedence
applies to item tokens. The preview resolves a token when its selected
`MissionPropertyValue_MissionItem` has one `SpecificItemsDef` condition, one
readable physical `MissionItem` target, both original count bounds equal to
one, and no active debug override. The target supplies the sole eligible
locale variant, or the single variant matching an explicitly named Tag.
This is a static presentation of the authored data. It establishes no
runtime selection.

Each projected title and description has an `itemTokens` object keyed by
language. A row retains the authored token name and nullable variant in
sourced cells. The outcome is either `staticPreview`, with the physical
MissionItem GUID and Record index, or `unresolved`, with the original token
left as a bracketed placeholder in the rendered text. Reasons distinguish
runtime queries, empty or ambiguous candidates, missing item targets,
missing or ambiguous variants, and missing translations. Token rows follow
the original text graph order and carry their own translation and selection
inputs. The original title and description keys remain in the protected
text dataset.

A scoped export can render a preview from a readable dependency outside
the requested Record filter. The filter still controls membership in
`definitions.missionItems.rows`. The row's physical identity and source
references remain sufficient to identify the exact dependency.

## Authored property inputs

The contextual `valueMeanings` of `configurations.contractTemplates` and
`configurations.missionBrokerEntries` bind `minItemsToFind`, `maxItemsToFind`,
`debugForceChosenItem` and `matchConditions` to the original owner's source
arena. Generator definitions expose the same links in
`definitions.contractGenerators.rows[].missionItemProperties`. Each Generator
entry identifies its handler, contract or sub-contract level and its original
property set, declaration and value within that Generator's existing arena.

For each `specificItems` condition, follow the `items` field binding and ordered
`candidates` to their original sourced cells. Repeated, null and unreadable
slots retain their own indices. The debug binding is independent of the
candidate list. The existing `missionPropertySets` rows retain their identifiers.
