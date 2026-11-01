# NPC and entity spawns are catalog kinds of content-hashed descriptions, and an unknown property value fails the Record

Decided for issue #815. Evidence: LIVE 4.10 (2026-10-31), the `MissionPropertyValue_NPCSpawnDescriptions`
and `_EntitySpawnDescriptions` structs of the contract generators, templates and broker entries.
Builds on ADR-0054 (additive catalog), ADR-0266 (ship spawns and location queries, content-hash
identity), ADR-0270 (texts per key), ADR-0313 (contract templates) and ADR-0320 (mission broker
entries). The owner confirmed on 2026-10-31 that the Basetool needs this export and, on #812, that
the Records state everything and a free-form part has a stable, documented, tagged shape; a struct
the catalog does not know fails the Record instead of hiding behind an escape hatch.

## Decision

- The kinds `contractNpcSpawns` and `contractEntitySpawns` and the fields `npcSpawns` and
  `entitySpawns` of a contract, a contract template and a mission broker entry are added additively
  (ADR-0054, schema version 2 keeps its number).
- **Two kinds, not one.** The Archive has no Record for either description. The entity spawn
  descriptions are the same kind of free-form part as the NPC ones and the `other` variant of the
  property value named both, so the kind `contractEntitySpawns` comes with `contractNpcSpawns`.
- **Identity as in ADR-0266.** Equal content shares one entry; its `guid` is the first 16 bytes of
  the SHA-256 of the canonical JSON of the groups, written as a UUID. The embedded name of an NPC
  option has game texts; the hash is made while those hold their keys only, and the texts are
  resolved when the pools are stored (`SpawnPools::resolve_texts`), so the identifier depends on
  the Archive and not on a language or on the translation of a key.
- **Links as in ADR-0266.** One struct, `SpawnPoolLink` (`variable`, `token`, `level`, `poolGuid`),
  serves the ship, NPC and entity links; it replaces `ShipSpawnLink`, whose shape it keeps (the
  schema definition has a new name, no field changed). The level reader resolves the nearest level
  that states a variable; a description without a group hides the variable at a lower level and is
  itself left out. `SpawnPools::register` is the one place that keeps a description and says what
  names it; the contract levels and the objective properties both use it.
- **What an NPC entry states.** Everything the structs state: per group the `name` and the options
  (`priority`, `includeLocationAISpawnTags`, `weight`, the `autoSpawnSettings`, the
  `identifierTags`, the variable of the `nameProperty` and the `embeddedName`). The settings state
  every field, with the five positive and negative tag pairs written as `{positiveTags,
  negativeTags}` objects. The embedded name has three game texts (`x-gameText`: `givenName`,
  `givenLastName`, `givenNickName`) and the Record of the random name parameters. A stored enum or
  free string (`excludeSpawnGender`, `initialActivity`) is kept as stored; `excludeSpawnGender`
  lists the values of the enum `EExcludeSpawnGender` in `x-values`.
- **What an entity entry states.** Per group the `name` and the slots; per slot the options with
  `amount`, `weight` and the four tag sets.
- **The property value union loses `other`.** The value of an objective property is now one of
  `object`, `deliveryOrder`, `boolean`, `stringHash`, `integer`, `float`, `tags`,
  `shipSpawnDescriptions`, `location`, `npcSpawnDescriptions`, `entitySpawnDescriptions` and
  `organization`; a value of any other struct fails the Record (`TemplateError::UnknownStruct` with
  the field `objective property value`). The `organization` variant is new because five
  objective properties on LIVE hold one: it carries the `matchConditions` as for a location query
  and the `debugForceChosenOrganization`; a condition of an unknown struct fails the Record as well.
  The types that only the contract-level properties hold on LIVE (`MissionItem`, `AIName`,
  `CombinedDataSetEntries`, `HaulingOrders`, `Reward`, `TimeTrialRace`) are left to #1003: its
  variants join the same union in `PropertyReader::value`, and the `AiName` struct of this ADR is
  what its `aiName` variant reuses. As in ADR-0320, the kind `contractTemplates` was merged
  with #990 on 2026-10-31 and no release contains it (v0.4.2 is unreleased), so no consumer has
  seen the `other` variant: ADR-0054's additive rule is not touched.
  Note: #1003 added those variants (`missionItem`, `aiName`, `combinedDataSetEntries`,
  `haulingOrders`, `reward`, `timeTrialRace`, and `starMapLocation`) and reads the values of the
  contract-level properties with the same reader; the field of the error is now `property value`
  and covers both levels, and a mission item's condition of an unknown struct fails with
  `item condition`.
- **Location queries keep `other` for now.** `LocationCondition::Other` of `contractLocationQueries`
  (ADR-0266) still lets a condition of an unknown struct pass; none occurs on LIVE. Only the
  organisation value fails the Record on one.
- **Texts.** The names are game texts and go through the per-kind verification of ADR-0270. On LIVE
  all 22 embedded names hold placeholder keys, so the kind has no keyed text and needs no baseline;
  a future key would be held to a baseline of zero and name itself.

## Consequences

- A consumer joins `contracts[].npcSpawns[].poolGuid` (and the same field of a template and a
  broker entry) to `contractNpcSpawns[].guid`, and the entity fields to `contractEntitySpawns`.
  Every link resolves on LIVE. The `guid` must not be stored across a change of the description.
- On LIVE the kinds hold 411 NPC spawn descriptions with 2,074 options and 43 entity spawn
  descriptions with 556 options. The links are 544 NPC and 182 entity links on contracts, 7 and 10
  on templates and 793 and 146 on broker entries.
- A Build with a property value of an unknown struct in an objective removes the whole template or
  broker entry (see ADR-0320). None fails on LIVE.
- The new parts are 2.1 MB for the two kinds and 0.5 MB for the link fields. The catalog is
  208,451,074 bytes decompressed and 3,820,279 bytes as `.json.zst` on LIVE (2026-10-31), 2.58 MB
  and 40,411 bytes more than the 205,869,419 and 3,779,868 bytes of ADR-0320. Against the limits that
  is 6.0 % of the upload limit of 64,000,000 bytes (headroom 60.2 MB) and 20.8 % of the decompressed
  limit of 1,000,000,000 bytes (headroom 791.5 MB).
