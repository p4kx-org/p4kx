# A contract lists the payout it states, and the calculated reward is not in the Archive

Decided for issue #888. Builds on ADR-0054, ADR-0065 and ADR-0068.

## Context

Community tools show a UEC reward for every contract. The catalog's `contracts` kind had none
(ADR-0068: `ContractResult_CalculatedReward` is not exported). The question was whether the Archive
states the reward after all.

## Findings (LIVE 4.10.2-live.12913470)

- `ContractResult_CalculatedReward` (2,389 results) has only `missionResults`. The figure a
  community tool shows for `HH_Pyro_LastLandings_Local_1box_Single_Courier` (48,250 UEC) is not a
  number of any Record, and no layer (sub contract, contract, handler, template,
  `propertyOverrides`, difficulty profile, broker entry) holds it.
- It follows from no rule of the data either: 140 of 626 groups of contracts with the same
  template, the same four difficulty ratings and the same reputation reward are shown with
  different figures. The game computes it from inputs outside the Archive.
- `ContractResult_Reward` carries a `MissionReward` (`reward`, `max`, `plusBonuses`, `currencyType`)
  and is a stored fact. It occurs in eight results, and where a community tool
  lists these contracts its figure equals the stored `reward`.

## Decision

- `contracts[].fixedRewards` lists the `ContractResult_Reward` results that fire on success (first
  `missionResults` entry set), as stored, in stored order: `currency`, `amount`, `max`,
  `plusBonuses`, each `null` when absent. Additive within version 2 (ADR-0054).
- In catalog v3, `fixedRewards.value` retains the same parent-contract selection and order.
  The list and each payout member have their own source evidence. `amount` and `max` are exact
  signed decimal strings. See [Fixed payouts](../catalog-contract-generators.md#fixed-payouts).
- A contract with only calculated rewards has an empty payout list (ADR-0065).
- Third-party data stays out of the repository and CI: the community figures were only a hint.

## Consequences

- A consumer that needs the UEC reward of a calculated contract has to take it from elsewhere; the
  catalog documents that the Archive does not state it.
- A Build in which CIG moves a stored amount into the data changes the field's content only, never
  its shape.
