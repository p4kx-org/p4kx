# Factions, reputation and mission data are facts, and lawful is a field

Decided for issue #319. Evidence: LIVE 4.10, ScDataDumper and NovaSCDataExtractor (facts only).

## Decision

- The catalog exports the Records that contracts refer to as seven kinds of schema version 2
  (`factionReputations`, `reputationScopes`, `reputationStandings`, `reputationContexts`,
  `missionTypes`, `missionLocalities`, `contractScenarios`). Version 2 is not released, so no bump.
  Each kind is the Records of one type under one folder, ordered by GUID.
- Facts only: GUIDs, Record names, numbers and links. The one kind of text is the name a Record
  shows in the game (faction, scope, standing, mission type), flagged `x-gameText` as the contract
  titles are. Descriptions, perk and journal texts, logos and icons stay out.
- A faction is lawful by the boolean `entityLawful` property of its `propertiesBB`, not by the
  `_Lawful_` in the class name that the other extractor uses. The field agrees with the
  `lawful_` or `unlawful_` prefix of all 24 factions of LIVE that have one and also classifies 14 of
  the 15 without (`Wikelo` lawful, `Ruto` not); a faction without the property (`IASI`) has `null`.
- A contract names its standing range by the `minStanding` and `maxStanding` of the contract
  (3,976 of 5,175 entries) and its reputation prerequisites per level (`handler`, `contract`,
  `subContract`) as stored. The catalog does not decide how the levels combine or what `exclude`
  inverts, because the Archive does not say.
- A locality has no localised name; its Record name is its name. A scenario keeps the tiers
  (points, badge identifier, multiplier, blueprint pools) and not the progression texts.
- Contract templates, mission broker entries, hauling orders, NPC and ship spawns and
  `MissionLocationTemplate` are not exported: the Basetool has no use for them yet and they are
  large, free-form structures. They can follow as further kinds.

## Faction definitions in schema v3

The catalog exports each Record whose owning type is `Faction` in
`definitions.factions.rows`, independent of its path spelling or development-like name. The
unfiltered family keeps every physical owner. The shared Filter selects rows by Record path, and
availability reports the source declaration independently of that selection.

A row carries its original Record identity, name and description, reaction and faction labels,
allied and enemy References in source order, the ordered friendly-fire overrides, and the
arrest, trespass, criminality and legal-rights flags. Each sourced value distinguishes a missing
field, explicit null, readable false and unreadable or invalid storage. A remaining owned member
occurs once in the row's source arena with its original declaration and value. Unknown Enum
labels remain authored strings. These fields describe static configuration.

Locale fields refer to canonical public names or protected game texts. Legacy String fields retain
their literal text in the same domains. A `FactionReputation` GUID and an incoming `Faction` GUID
are independent links. A stored GUID survives a dangling or filtered target. The reputation
dataset retains its own identity and standing semantics.

## Consequences

- A consumer joins `contracts[].factionReputation`, `reputationGains[].scopeGuid`,
  `minStandingGuid` and `standingRequirements` to the new kinds by GUID, and
  `contracts[].missionTypeGuid` to `missionTypes`; on LIVE every link resolves.
- Two factions have enemies and none has allies on LIVE; the lists are as stored.

## Catalog v3 standing definitions

The standing rows belong to `definitions.reputationStandings`. Their physical Record
identity and field provenance follow ADR-0356 and ADR-0357. The authored description,
perk text and icon paths are part of the definition. Rank display names and perk
descriptions use the protected text registry. Stored drift intervals retain their
Single bits and original hours, including fractional intervals and signed zero.

## Catalog v3 scope definitions

`definitions.reputationScopes` holds one sourced row per physical scope Record. The
owned standing map retains the authored numeric widths and each standing reference
in stored order, including unresolved references. Scope labels and descriptions use
the protected text registry. The icon remains a path, and the source arena retains
additional declarations.

## Catalog v3 context definitions

`definitions.reputationContexts` retains physical contexts, their authored sort order, and
each primary and listed scope occurrence. Scope references preserve their original identity and
resolution. Each occurrence retains its original blackboard properties, including protected
Locale references and exact numeric values. Unclassified dynamic bodies remain in the source arena.

## Catalog v3 sandbox triggers

Each selected physical faction reputation Record owns its ordered triggers in
`configurations.reputationTriggers`. Each configuration refers to its owner by sourced
GUID and original Record index. Manual parameters and outcomes preserve authored order,
while References join the reusable trigger and reward amount definitions by original GUID.
Unknown concrete types retain their original fields and report their coverage state.
