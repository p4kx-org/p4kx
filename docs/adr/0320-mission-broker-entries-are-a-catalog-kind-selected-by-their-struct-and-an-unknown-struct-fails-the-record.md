# Mission broker entries are a catalog kind, selected by their struct, and an unknown struct fails the Record

Decided for issue #813. Evidence: LIVE 4.10 (2026-10-31), the `MissionBrokerEntry` Records. Builds on
ADR-0054 (additive catalog), ADR-0237 (mission data are facts), ADR-0266 (ship spawns and location
queries), ADR-0270 (texts per key) and ADR-0313 (contract templates). The owner confirmed on
2026-10-31 that the Basetool needs this export and, on #812, that an entry carries everything its
Record states and that a free-form part has a stable, documented shape: a tagged union, and a
struct the catalog does not know fails the Record instead of hiding behind an escape hatch.

## Decision

- The kind `missionBrokerEntries` is added additively (ADR-0054, schema version 2 keeps its number),
  together with the contract field `brokerEntryGuid`, the `missionBrokerEntry` the contract names.
  On LIVE every one of the 419 contracts that name an entry resolves.
- **Selection by struct, not by folder.** An entry is every Record of the struct
  `MissionBrokerEntry` below `libs/foundry/records/`. All 2,584 on LIVE lie under `missionbroker`;
  the rule does not depend on it. The usual leftover words apply (156 Records are left out), and the
  entries the catalog lists are 2,428.
- **What an entry holds.** Everything the Record states: the identity, the five game texts, the
  links (mission type, location, locality, giver, the entries it invites, links, requires or
  excludes), the tags, the availability flags and wanted level, the schedule, the cooldowns, the
  instance limits, the reward, the buy-in, the deadline, the reputation requirements and results,
  the mission properties, the ship spawn and location query links with the new level `broker`, the
  hauling orders of the objectives, the objectives, the partial payout, the modifiers and the
  mission flow. The shapes of the last five are those of ADR-0313 and come from the same readers
  (`read_kind`, the level reader, the property, objective, flow and modifier readers, `nonnil_guid`
  and the warning `unknown_template_values`).
- **Grouping without a change of shape.** The fields are grouped in small structs (names, links,
  tags, module, availability, cooldowns, instance limits, reward, finish) that are written flat or
  as one object; the reading modules stay below 400 lines and each has one responsibility.
- **Unknown structs fail the Record.** A modifier, a flow condition, a flow action and the
  reputation requirement block or one of its elements of a struct the catalog does not know make
  the Record a failure of the build with the struct name as reason. This replaces the `other`
  variants of ADR-0313 for modifiers, conditions and actions. The ADR stated that none occurs on
  LIVE; two did: `MissionFlowCondition_AnyTokensState` (three templates) and
  `MissionFlowCondition_OnMissionStart` (one), which would have been `other`. Both are variants
  now (`anyTokensState`, `onMissionStart`) and no Record of LIVE fails. The value of an objective
  property keeps `other` with its `structName` for the NPC and entity spawn descriptions that
  follow with #815; no objective of a broker entry holds one.
- **Union shapes.** A reputation requirement element is tagged by `type`: `requirement`, `and`,
  `leftParenthesis`, `rightParenthesis`; the expression is infix and the catalog does not evaluate
  it. The modifiers gain `factionHostility`, `hostileMission`, `ignoreMissionPlayerCriminality` and
  `lawLicense` next to those of ADR-0313.
- **Closed sets and unknown names.** The comparison, the month, the repeat type, the reaction of a
  faction and the licence type are closed value sets (ADR-0054): a string with `x-values`. A name
  the catalog does not know is `null` and counted in the warning `unknown_template_values`; the
  real-Archive test requires none on LIVE.
- **Reuse for #815.** The NPC spawn descriptions of the properties are still only a `valueType`
  here. The property list reader, the level reader with its new level and the pools in
  `contractShipSpawns` and `contractLocationQueries` are the seams #815 extends; no entry of this
  kind needs a change for it.

## Consequences

- A consumer joins `contracts[].brokerEntryGuid` to `missionBrokerEntries[].guid`. A link to another
  entry (`invitationMissionGuid`, `associatedMissionGuids`, `requiredMissionGuids`, a modifier's
  `brokerEntryGuid`) resolves on LIVE in every case; a GUID the Archive lacks would be kept.
- A consumer must not read a time of an entry (cooldown, life time, deadline) as a duration without
  knowing its unit; the Archive does not state one.
- On LIVE the kind holds 2,428 entries with 703 objective tokens, 26,741 properties, 949 modifiers,
  2,097 flow triggers and 1,666 requirement terms and none is left out for a failure.
- The kind adds 17,733 keyed texts to the verification of ADR-0270; the baselines are 0.04 for
  English and 1.0 for German, the share measured on LIVE rounded up to the hundredth.
- The kind itself is 16.9 MB of JSON. The catalog is 205,869,419 bytes decompressed and 3,779,868
  bytes as `.json.zst` on LIVE (2026-10-31), 18.7 MB and 0.34 MB more than the 187,179,857 and
  3,436,476 bytes of ADR-0313; the upload limit is 64,000,000 bytes.
