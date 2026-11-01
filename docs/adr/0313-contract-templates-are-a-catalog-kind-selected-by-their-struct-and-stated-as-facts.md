# Contract templates are a catalog kind, selected by their struct and stated as facts

Decided for issue #812. Evidence: LIVE 4.10 (2026-10-31), the `ContractTemplate` Records. Builds on
ADR-0054 (additive catalog), ADR-0237 (mission data are facts), ADR-0263 (contract layers),
ADR-0266 (ship spawns and location queries) and ADR-0270 (texts per key). The owner confirmed on
2026-10-31 that the Basetool needs this export and decided, in two further comments on #812 the
same day, that it carries everything the Record states: the payouts, contributions, modifiers,
owner, mission flow, comms, the handler and display texts and the properties of the objectives.

Amended by ADR-0320: a modifier, flow condition or flow action of a struct the catalog does not
know fails the Record instead of being `other`, the flow conditions `anyTokensState` and
`onMissionStart` and the modifiers `factionHostility`, `hostileMission`,
`ignoreMissionPlayerCriminality` and `lawLicense` are variants, and "none occurs on LIVE" below
held only because those two conditions were `other`.

## Decision

- The kind `contractTemplates` is added additively (ADR-0054, schema version 2 keeps its number).
  `contracts[].templateGuid` resolves to an entry; on LIVE every one does.
- **Selection by struct, not by folder.** A template is every Record of the struct
  `ContractTemplate`. 488 of the 496 on LIVE lie under `contracts/contracttemplates`; the other
  eight (service beacons and a few contracts) are named by contracts too, so a folder prefix
  would leave links dangling. The kind's rule is therefore the whole `libs/foundry/records/` tree
  with the struct as the only discriminator.
- **Leftovers.** The words of the development-leftover rule apply except `template`: live
  templates such as `handyman_template` carry it, and every Record of the kind is a template.
- **What an entry holds.** `guid`, `className`, `recordPath`, the `notForRelease` and
  `workInProgress` flags, the `title` (game text), the mission type, the display flags, the
  contract class with its settings and deadline, the mission properties as variable, token and
  value kind, the template-level `shipSpawns` and `locationQueries` links into the kinds of
  ADR-0266, the hauling orders (ADR-0265), the `partialPayout`, the `modifiers`, the `owner`, the
  `missionFlow`, the start and end comms and the objective tokens.
- **Objectives.** Each token states its handler (the struct and its fields; a field the struct
  lacks is `null`), its display texts and the overrides of the mission details (game texts), its
  reward contribution, its comms and its `objectiveProperties`.
- **Objective properties.** A property is `referenced`, `output`, `embedded` or `input`. Its value
  is a union tagged by `type`: `object`, `deliveryOrder`, `boolean`, `stringHash`, `integer`,
  `float`, `tags`, `shipSpawnDescriptions`, `location` and `other`. Ship spawns and location
  queries go through the reader of the contracts and join their kinds (ADR-0266); a struct the
  catalog does not read, such as the NPC and entity spawn descriptions that follow with #815, is
  `other` with its `structName`.
- **Shapes of the free-form parts.** The mission flow is a tree: a condition is tagged by `type`
  (`tokenState`, `allTokensState`, `and`, `or`, `not`, `propertyIsSet`, `other`), an action
  likewise (`stateChange`, `tokenStateChange`, `pickRandomMissionPhase`, `other`); a modifier is
  tagged by `modifier` and its shared fields are one header. A struct the catalog does not know is
  `other` with its `structName` and the fields its siblings share; none occurs on LIVE. A weak
  pointer to an objective is the `id` of that objective, a weak pointer to a property its
  `missionVariableName`, a pointer to nothing is `null`.
- **Depth.** Nothing is cut silently. A Record whose objective phases nest deeper than 32 levels
  or whose flow conditions nest deeper than 16 cannot be read: it is a failure of the build with
  that reason and no entry. The Records of LIVE nest five and three levels.
- **Closed sets and unknown names.** Stored enum names (a state, a category, a comms timing or
  reason, a kind of objective property) are closed value sets (ADR-0054): a string with
  `x-values`. A name the catalog does not know is `null` and counted in the warning
  `unknown_template_values` of the build, with each kind and value; the real-Archive test requires
  none on LIVE.
- **Reuse.** The rewards and the reputation of a contract are read for `contracts` and are not
  repeated: the payout links to `reputationScopes`. The Record reading loop is one helper
  (`read_kind`) shared with the mission location templates; the links come from the same level
  reader as the contract entries, so a template's `level` is always `template`. Mission broker
  entries (#813) and NPC spawns (#815) can use the same helper, the same property list and the
  same weak pointer readers. The test of a nil unique identifier is one helper, `nonnil_guid`.
- **Tag paths.** A Reference to no Record is a null value in the DataForge, so a nil tag never
  reaches the tag reader and no guard is needed. A test pins this for a list and for a single tag
  field. The content-hash identifiers of `contractShipSpawns` and `contractLocationQueries` and
  the entries of `missionLocationTemplates` are byte for byte the same on LIVE with and without a
  guard against nil tags (SHA-256 of each kind, checked 2026-10-31), so ADR-0266 stands unchanged.

## Consequences

- A consumer joins `contracts[].templateGuid` to `contractTemplates[].guid` and reads what the
  template states under the overrides of handler, contract and sub contract.
- A consumer must not read `completionTime` as a duration without knowing its unit.
- On LIVE the kind holds 496 entries, 335 of them named by a contract, with 711 objective tokens,
  6,140 objective properties (433 with a value), 58 ship spawn links, 1,128 location query links,
  418 hauling orders, 1,075 flow triggers (1,250 links to objectives, 2 that point to none), 709
  handlers, 672 payout ranges, 70 modifiers and 724 comms; none is left out and no stored name is
  unknown.
- The kind adds 6,765 keyed texts to the verification of ADR-0270; the baselines are 0.01 for
  English and 0.99 for German, the share measured on LIVE rounded up to the hundredth.
- The catalog is 187,179,857 bytes decompressed and 3,436,476 bytes as `.json.zst` on LIVE
  (2026-10-31); the upload limit is 64,000,000 bytes. The kind itself is 4.7 MB of JSON. Step by
  step: the identity, links, flow and objectives made it 183,926,363 and 3,401,291 bytes; the
  payouts, modifiers, owner, comms, handlers and texts 186,268,384 and 3,422,874; the objective
  properties added 0.9 MB and 13.6 KB to that.
