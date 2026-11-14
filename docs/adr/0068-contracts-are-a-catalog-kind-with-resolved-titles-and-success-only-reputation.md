# Contracts are a catalog kind with resolved titles and success-only reputation

For catalog schema 3, the pickup-place length cap is superseded by
[ADR-0359](0359-contract-pickup-places-retain-every-eligible-source-location.md). The original decision and recorded Build observations below remain historical evidence.

Decided for issue #416 (VerseKit, an external consumer). Builds on ADR-0054 and ADR-0065.

VerseKit rebuilds the contract chain itself from the format crates and finds that every tool has to
repeat the same work: lay the overrides of a sub contract over its contract, handler and template,
resolve `~mission(Token)` in the titles, find the mission type and sum the reputation. The issue
asks whether that belongs in the Basetool catalog or in a command of its own.

## Decision

- **A kind of the one catalog.** `contracts` joins the catalog (ADR-0054: new kinds are additive,
  one upload, one `completeness`, one Build). It is not a separate export with its own schema: the
  legal rules, the `x-gameText` marks, the channel notice and the schema test already live here,
  and the existing reward sources (`blueprintPools`) name the same contracts by `id`.
- **One entry per contract and per sub contract.** A contract with sub contracts has its own entry
  and one per sub contract, as the reference tools list them. The overrides are laid over in this
  order, the first that states a value wins: sub contract, contract, handler, template.
- **Resolved title with placeholders.** The title is the one text the consumer needs, so the
  catalog resolves what the Archive states (string-hash and organisation properties, with the
  organisation's tagged string variant) and writes what the game picks at mission start as
  `[LOCATION]`, `[TARGET]` or `[SHIP]`. The placeholders are the catalog's convention, documented
  in `docs/basetool-catalog.md`; an unresolvable token becomes `[TOKEN]`. The title and the mission
  name are game text (`x-gameText`).
- **Reputation is listed, not summed.** `reputationGains` has one entry per amount of the results
  that fire on success (first `missionResults` entry set), with the faction and scope Records and the
  `reputationAmount`. A failure result is not listed. Adding faction and affinity scope together is
  the consumer's step (ADR-0065: raw facts, no derived values).
- **No computed reward.** `ContractResult_CalculatedReward` is not exported as a number: its
  formula is the game's, not a field of the Archive.

## Consequences

- The Rust types, the schema and `docs/basetool-catalog.md` change in one commit; version 1 stays.
- Pickup places are the starmap objects of the locality and location prerequisites, system first,
  then planets and moons, stars left out, at most five; they need the starmap kind in the same
  build. They are the one exception to "the first level that states a value wins": the
  prerequisites of the contract and of its handler are combined (a sub contract's own replace
  them), and a contract that is left with none takes the location of its broker entry.
- A contract without a template takes title, type and location from its `MissionBrokerEntry`.
- Token expansion has a budget (depth 3, 64 replacements, about 4 KiB per title and language): a
  replacement that holds its own token cannot grow a title without bound.
- An organisation token without a variant takes the organisation's only string variant; an
  organisation with several states no choice, because its first entry is tied to a tag (`_from`,
  `_title_001`, `_desc_001`) and is not the plain name. The token stays unresolved (checked against
  LIVE records, #472).
- `ContractTemplate` Records are found by their type, not by their folder (two on LIVE lie outside
  `contracts/contracttemplates`).
- A negative reputation amount is listed as stored: on LIVE all 48 are the success result of the
  `CFP_OOS` and `HH_OOS` handlers (and `_Intro`), deducting standing with the rival faction.
- The layer and property layout is read as the reference tools read it (`paramOverrides`,
  `contractParams`, `propertyOverrides`, `contractDisplayInfo`); a Build that moves a field leaves
  that part `null` or a placeholder, never a failed Record.
