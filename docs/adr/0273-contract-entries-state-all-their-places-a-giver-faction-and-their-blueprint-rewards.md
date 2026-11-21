# Contract entries state all their places, a giver faction and their blueprint rewards

For catalog schema 3, the pickup-place length cap is superseded by
[ADR-0359](0359-contract-pickup-places-retain-every-eligible-source-location.md). The original decision and recorded Build observations below remain historical evidence.

Decided for issue #915 (VerseKit, an external consumer). Evidence: LIVE 4.10 (2026-10-31), the 109
`ContractGenerator` Records, 5,175 contract entries. Builds on ADR-0054 (additive growth), ADR-0065
(raw facts), ADR-0068 (contracts), ADR-0263 (layers) and ADR-0266 (locations).

VerseKit lists where a blueprint comes from and found four gaps in the `contracts` kind.

## Decision

- **`canBeShared` needs no change.** Each entry has had `canBeShared` as `{value, layer}` since
  ADR-0263 (parameter `CanBeShared` of sub contract, contract and handler, else the contract class
  of the template).
- **`offeredAt` lists every place.** `pickupPlaces` keeps its meaning (at most five starmap
  objects, ordered by rank; 3,936 of the 5,175 entries reach the cap). The new `offeredAt` has
  `localityGuids`, the `MissionLocality` Record of each `ContractPrerequisite_Locality`, and
  `locations`, the object of each `ContractPrerequisite_Location`, each with `starmapObjectGuid`,
  `objectType` and `place`. It reads the same prerequisites as the pickup places (a sub contract's
  own replace those of the contract and its handler), in stored order and none twice, without the
  cap, without dropping stars and without dropping an object that is missing from
  `starmapObjects` (its `objectType` and `place` are `null`). A locality is linked, not expanded:
  the consumer joins `missionLocalities[].starmapObjectGuids` to `starmapObjects`, which keeps
  the 5,175 entries from repeating the same list. A contract whose prerequisites state no place
  takes the location of its broker entry and `brokerFallback` is `true`, so a consumer tells it
  from a stated location. This differs from `pickupPlaces`, which falls back whenever no place
  resolves (for example only stars are named); `offeredAt` stays "as stated". A Reference to a
  missing Record keeps its GUID (ADR-0269). On LIVE 4,801 entries state a place and every link
  resolves. The three new fields add about 2.0 MB to the uncompressed catalog JSON.
- **`giverFaction` states the faction and where it came from.** `faction` stays the handler's
  `factionReputation` as stored; changing it would change its meaning, which ADR-0054 forbids.
  `giverFaction` is `{source, faction}`, `faction` having the shape of `faction`, and `source` one
  of `handler` (the same faction as `faction`), `reputationResult` or `reputationPrerequisite`:
  the handler's faction first; else the faction of the first success amount of the contract's
  `reputationGains` whose `amount` is positive; else the faction of the first reputation
  prerequisite (the order of `standingRequirements`) that is not an exclusion, named from the
  Reference's path like a gain, so a faction Record the faction index lacks stays. A negative
  amount names a rival that loses standing (the `CFP_OOS` and `HH_OOS` handlers, ADR-0068) and
  never counts. The gains are those of the entry's own resolved results, resolved like the
  blueprint rewards. `null` when none names a faction. On LIVE: 4,010 entries `handler`, 232
  `reputationResult`, 57 `reputationPrerequisite`, 876 `null`. None of the 232 has more than one
  distinct positive faction, so the choice of the first is never a choice between factions; the
  picked gains sit in the scope of their faction (130 `factionreputationscope`, the rest the
  faction's own scope such as `reputationscope_wikelo`, 7 `reputationscope_affinity`), and since
  the factions never differ, preferring a scope would change nothing.
- **Why this does not break ADR-0065.** ADR-0065 forbids values computed with the game's rules
  (damage per second, acceleration). `giverFaction` computes nothing: it names one of the factions
  the Archive states and the field it came from, using a fixed order the documentation states.
  The raw facts stay in `faction`, `reputationGains` and `standingRequirements`, so a consumer
  with another preference reads them. The catalog does not weigh several stated factions against
  each other: the first wins.
- **`blueprintRewards` lists the pools of the entry.** Each element is `{poolGuid, source,
  chance}`, `poolGuid` the `guid` of an entry of `blueprintPools`. `source` `result`: a
  `BlueprintRewards` result whose first `missionResults` flag (success) is set, `chance` as stored;
  the results are the entry's own resolved results (the sub contract's when it states results,
  else the contract's). `source` `temporaryModifier`: an enabled
  `MissionModifier_TemporaryBlueprint` of `modifierOverrides` of the nearest level (sub contract,
  contract, handler) that states one, `chance` `null`. `blueprintPools[].rewardSources` stays, and
  names only the `id` that a contract and its sub contracts share.

## Census (LIVE 4.10)

- `BlueprintRewards` results: 1,037, all in the `contractResults` of a contract (1,032 of
  `contracts`, 5 of `introContracts`). No sub contract and no handler has a `contractResults`
  block, so every sub contract takes the results of its contract; the reading of the sub
  contract's own results is the rule for a build that adds some.
- Outcome flags: 1,034 fire on success, 3 on the second outcome. Those three are left out of
  `blueprintRewards`, as a failure amount is left out of `reputationGains` (ADR-0068). All 1,037
  have a `chance`.
- The scan that fills `blueprintPools[].rewardSources` finds every struct that references a pool.
  Two structs of the contracts do: `BlueprintRewards` (1,037) and
  `MissionModifier_TemporaryBlueprint` (2, enabled, in the `paramOverrides` of two contracts of
  the `thebackpocket` generator, named `tempBP` and `testIlya`; they have no chance). The
  modifiers are read, as `temporaryModifier`.
- No contract `id` is stated twice with a reward (no duplicate `guid` and `subContractGuid` is
  dropped by the entry rule), so no source is lost to that rule.
- With the modifiers, the entries list 2,290 rewards on 785 contracts. Every reward of an entry is
  a source of its pool, every sub contract lists the rewards of its contract, and exactly three
  sources of exported contracts are in no entry: the three `BlueprintRewards` results that fire on
  the second outcome (`TheCollector_Vehicle_L22_Wolf`, `TheCollector_Vehicle_DrakeClipper`,
  `ORS_Cr_Large`). The real-Archive test asserts that count.

## Consequences

- The three fields are additive within schema version 2 (ADR-0054). A reader that does not know
  them ignores them.
- `offeredAt.locations[].place` is a game text like `pickupPlaces[].place`.
- A consumer that wants names of every place resolves `localityGuids` through `missionLocalities`
  and `starmapObjects`, and reads `locations` directly.
