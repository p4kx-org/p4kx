# Blueprint catalog contract

`definitions.blueprints`, `configurations.blueprintTiers` and
`projections.blueprintPresentation` are the Blueprint datasets in schema v3. The shared table
availability describes whether the source family is declared. Selection uses the
`CraftingBlueprintRecord` type, the crafting Blueprint path, the Filter and the development
leftover rule.

## Physical identity and the source arena

A definition has the [eight sourced Record header fields](catalog-record-headers.md), the physical
`inputRef` and `source`. A `blueprint` link identifies this definition by `inputRef` and
`recordIndex`. GUIDs are nominal game identifiers and may repeat across physical Records. Definitions
and presentations are sorted by GUID and then physical Record index.

`source.roots` keeps every declared root field and `source.values` stores the original value
occurrences once. Each `valueIndex` is a decimal string indexing the same definition's
`source.values`. Source fields keep their original declaration, type and value position. Struct
fields and array elements preserve authored order, repeats, nulls and unreadable slots. Exact
numeric storage, protected authored text, reference targets and source errors remain in the
arena. A valid deeply nested recipe uses flat links throughout the output.

## Tier interpretation and ingredients

Each `configurations.blueprintTiers.rows[]` points to its Blueprint and original tier occurrence.
`craftTimeSeconds` rounds the readable stored time parts to whole seconds. `mandatory` and
`optional` navigate the selected interpreted cost occurrences. The `costs` array holds the
recognized costs in original depth-first order. Each cost has a `source` reference, `meaning`
and `context`.

- `select` retains count, original debug-name reference, protected display-name reference and
  ordered option references.
- `resource` retains ResourceType GUID, quantity in SCU and minimum quality.
- `item` retains entity GUID, quantity in units and minimum quality.

The context points to the original source and retains the effective quantity multiplier, the
original composition option, and ordered property modifiers. Recognized multiplier and additive
ranges keep their quality endpoints and factors or summands. Every typed normalized datum carries
its actual contributing source uses and encryption marker. The raw arena retains values outside
the supported interpretations.

`projections.blueprintPresentation.rows[]` carries the protected name reference, selected
category choice, resolved produced-item GUID, default availability and the applicable dismantle
link. Its `firstTier` and `ingredients` point back to the canonical tier and costs. Each ingredient
links the resource or item cost and the nearest enclosing named selection if one exists. The
ingredient order follows the first tier's depth-first cost order. Resource Commodity backfill
uses all tiers. Original selector occurrences remain distinct, including repeated references to
the same ResourceType.

## Research

`research.source` points to the actual value occurrence when the tier declares `research`.
`research.envelope` has a sourced state and contains `researchCosts` and `unlockRequirements`
for a readable `CraftingResearch`. Each member carries its original occurrence, actual declared
base and independent value state. The recognized strong-pointer bases are
`CraftingRecipeCosts_Base` and `CraftingResearchUnlock_Base`. A present envelope with both
pointers null records the present-empty case. The exact fieldless declared bases can also be
present. Unsupported populated shapes and unreadable members remain in the original arena.
The health report carries the actual field diagnosis. Normal publication requires complete
health, while diagnostic publication retains the raw owner and the diagnosis.

The historical LIVE 4.10 audit observed empty pointers on all 926 declared Research envelopes.
The recorded 5.0 PTU declaration audit established the two base types. Installed-Archive
acceptance remains a separate check for concrete production payloads.

## Standalone Rust reader

`read_crafting` shares Blueprint selection and typed cost interpretation. `Tier::costs()`
returns the flat cost sequence. Resolve the tier's `mandatory`, `optional` and each
`Selection.options` value through `Tier::cost`. `Tier::mandatory_cost` reads the mandatory
cost directly. `Tier::research` preserves the original envelope and member state.
