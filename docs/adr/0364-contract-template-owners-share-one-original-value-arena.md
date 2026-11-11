# Physical Template owners retain one source arena and separate meanings

Catalog v3 stores a physical Contract template and its selected original value occurrences once in
definitions.contractTemplates, with typed Flow, property and context rows pointing into that same
owner arena, because recursive wire values lose source facts and make ordinary JSON depth depend
on game structure. This replaces the recursive Template wire shape of
[ADR-0313](0313-contract-templates-are-a-catalog-kind-selected-by-their-struct-and-stated-as-facts.md)
within [ADR-0356](0356-catalog-v3-uses-canonical-source-typed-tables.md), preserving the established
selection and identity recipes while separating readable source retention from required interpretation.

## Consequences

- Original Field/Array ordinals, every retained null/error slot, additional fields and exact numbers
  remain separate occurrences in original DFS order; equal values never merge. Selected same-owner
  target fields join this one arena by their actual occurrence, including overlapping roots, and
  references never recursively expand target Records. A cut root retains its original parent ordinal
  and local field index even when that parent is outside the emitted arena.
- configurations.missionFlows and configurations.missionProperties contain typed links and explicit
  declaration/selection controls. configurations.contractTemplates contains the permanent computed
  context, including localization, Tag paths, physical objective ordering, weak-field interpretation,
  spawn/query identities and hauling choices; copied raw flags/settings/payout facts are SourceFieldRefs.
- Nullable condition/action/random-phase/trigger meanings share the original arena cells. Unknown
  required conditions/actions refuse Normal publication and retain readable Diagnostic source facts
  with Unsupported meaning and incomplete required interpretation, rather than absence or empty success.
- Every independent original datum keeps its direct origins under
  [ADR-0357](0357-source-encryption-provenance-is-explicit-per-data-value.md); structural links are
  controls and cannot authorize source facts under
  [ADR-0360](0360-original-read-bindings-stay-separate-from-value-equality.md).
  Stored GUIDs, weak-derived GUIDs and actual selected target fields keep their distinct real receipts.
- One iterative original-site kernel serves canonical inspection and the separate projection index;
  these are separate tree traversals without an added decode or numeric-inspection pass.
  There is no consumer depth/count cutoff or relaxed JSON recursion check. Actual decoder/read failures,
  cancellation and checked host-size/counter failures remain explicit.
- Physical Brokers keep every original root and owned occurrence in their one
  definitions.missionBrokerEntries arena, with separate configurations.missionBrokerEntries
  context. Both families share the original source codec, flat Flow algorithm and tagged physical
  owner links for missionFlows and missionProperties. Raw Broker flags, links and quantities
  remain in D; contextual vocabulary, text, Tag, selection and reward interpretations point to
  actual source. This supersedes the root Broker DTO and source-omission rules of
  [ADR-0320](0320-mission-broker-entries-are-a-catalog-kind-selected-by-their-struct-and-an-unknown-struct-fails-the-record.md)
  in Catalog v3.
- A readable Broker owner stays emitted if required context fails. The raw Payload uses stay
  in that D arena, while the same original required cause is reported as DerivedProjection
  and still refuses Normal publication. The failed Broker context does not emit a success row.

The [Template source contract](../catalog-template-source.md) specifies the carrier and oracle migration.
This decision establishes no runtime, schema-generation, installed-Build or release acceptance.
