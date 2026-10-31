# Contract templates are a catalog kind, selected by their struct and stated as facts

Decided for issue #812. Evidence: LIVE 4.10 (2026-10-31), the `ContractTemplate` Records. Builds on
ADR-0054 (additive catalog), ADR-0237 (mission data are facts), ADR-0263 (contract layers) and
ADR-0266 (ship spawns and location queries). The owner confirmed on 2026-10-31 that the Basetool
needs this export.

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
- **Facts only.** An entry has `guid`, `className`, `recordPath`, the `notForRelease` and
  `workInProgress` flags, the `title` (game text, the first display string; most templates have
  only a placeholder and so no text), the mission type, the display flags `illegal`,
  `showLifeTimeInMobiGlas` and `preShowObjectives`, the struct of the contract class with its
  `ActiveContractSettings` and `ContractAutoFinishSettings` (flags and the deadline values as
  stored; the Archive does not state their unit), the mission properties as variable, token and
  value kind, the template-level `shipSpawns` and `locationQueries` links into the kinds of
  ADR-0266, the hauling orders (ADR-0265) and the objective tokens (`id`, `parentId`, `debugName`,
  `startsActive`, handler struct, phase tag) in depth-first order.
- **Not exported:** the objective handler and display texts, comms notifications, mission flow
  triggers, partial reward payouts, reward contributions, modifiers and the owner team. They are
  free-form, hold texts, or have no consumer.
- **Reuse.** The Record reading loop moves into one helper (`read_kind`) shared with the mission
  location templates; the links come from the same level reader as the contract entries, so a
  template's `level` is always `template`. Mission broker entries (#813) and NPC spawns (#815)
  can use the same helper and the same property list.

## Consequences

- A consumer joins `contracts[].templateGuid` to `contractTemplates[].guid` and reads what the
  template states under the overrides of handler, contract and sub contract.
- A consumer must not read `completionTime` as a duration without knowing its unit.
- The text verification of ADR-0270 lists the kind with the baselines measured on LIVE: 24 keyed
  titles, 3 missing in English (0.13) and all in German (1.0).
- On LIVE the kind holds 496 entries, 335 of them named by a contract, with 711 objective tokens,
  58 ship spawn links, 1,128 location query links and 418 hauling orders; none is left out.
