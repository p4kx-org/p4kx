# Contract templates are a catalog kind, selected by their struct and stated as facts

Decided for issue #812. Evidence: LIVE 4.10 (2026-10-31), the `ContractTemplate` Records. Builds on
ADR-0054 (additive catalog), ADR-0237 (mission data are facts), ADR-0263 (contract layers),
ADR-0266 (ship spawns and location queries) and ADR-0270 (texts per key). The owner confirmed on
2026-10-31 that the Basetool needs this export and, in a second decision the same day (comment on
#812), that it carries everything the Record states: the payouts, contributions, modifiers, owner,
mission flow, comms and the handler and display texts.

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
  reward contribution and its comms. The `properties` of an objective (references, outputs and
  embedded values) are the one part left out: they are a second, embedded copy of the mission
  property model with value structs of their own and no consumer yet.
- **Shapes of the free-form parts.** The mission flow is a tree: a condition is tagged by `type`
  (`tokenState`, `allTokensState`, `and`, `or`, `not`, `propertyIsSet`, `other`), an action
  likewise (`stateChange`, `tokenStateChange`, `pickRandomMissionPhase`, `other`); a modifier is
  tagged by `modifier`. A struct the catalog does not know is `other` with its `structName`; none
  occurs on LIVE. Nesting is followed to 16 levels (phases to 32). A weak pointer to an objective
  is the `id` of that objective, a weak pointer to a property its `missionVariableName`. Stored
  enum names are closed value sets (ADR-0054): a string with `x-values`, `null` for a value the
  catalog does not know.
- **Reuse.** The rewards and the reputation of a contract are read for `contracts` and are not
  repeated: the payout links to `reputationScopes`. The Record reading loop is one helper
  (`read_kind`) shared with the mission location templates; the links come from the same level
  reader as the contract entries, so a template's `level` is always `template`. Mission broker
  entries (#813) and NPC spawns (#815) can use the same helper, the same property list and the
  same weak pointer readers.

## Consequences

- A consumer joins `contracts[].templateGuid` to `contractTemplates[].guid` and reads what the
  template states under the overrides of handler, contract and sub contract.
- A consumer must not read `completionTime` as a duration without knowing its unit.
- On LIVE the kind holds 496 entries, 335 of them named by a contract, with 711 objective tokens,
  58 ship spawn links, 1,128 location query links, 418 hauling orders, 1,075 flow triggers (1,250
  links to objectives, 2 that point to none), 709 handlers, 672 payout ranges, 70 modifiers and 724
  comms; none is left out.
- The kind adds 6,750 keyed texts to the verification of ADR-0270; the baselines are 0.01 for
  English and 0.99 for German.
- The kind makes the catalog 186,268,384 bytes decompressed and 3,422,874 bytes as `.json.zst` on
  LIVE (2026-10-31); the upload limit is 64,000,000 bytes. The objective texts, handlers, flow and
  comms add 2.3 MB decompressed and 21.6 KB compressed to the first version of the kind
  (183,926,363 and 3,401,291 bytes).
