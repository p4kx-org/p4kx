# ContractGenerator catalog contract

`definitions.contractGenerators` holds the original `ContractGenerator` Records selected by type,
path and Filter. `projections.contracts` holds the logical contracts and sub contracts.
`projections.missionTitles` holds their derived title and description.

## Original records and contract identities

Each definition has the [eight sourced Record header fields](catalog-record-headers.md), an
`inputRef` and the complete original `source` arena. `source.roots` stores the ordered field
declarations and `source.values` stores their original value occurrences. Raw slots include
empty arrays, duplicates, nulls and unreadable values where the Record tree could be retained.

A contract projection's `source` identifies the same physical definition by `inputRef` and
`recordIndex`. Its `handler`, `contract` and nullable `subContract` select actual values in that
definition. Each `valueIndex` is a decimal string in the definition's `source.values`. The
Generator Record GUID and path are `guid.value` and `recordPath.value` of that definition.

Logical `guid` and `subContractGuid` have immediate origin metadata at the original `id` fields.
The parent row has an absent `subContractGuid`. The projections follow the established order of
contract ID, sub-contract ID and Generator Record GUID. The first logical ID pair wins. All
selected physical Generator rows and their original slots remain available in the definition table.

## Authored scenario requirements

`declarations.contractScenarioRequirements` contains each original
`required_active_scenarios` Reference array on a selected Generator, Handler or Contract.
Join `source.inputRef` and `source.recordIndex` to `definitions.contractGenerators`.
`source.owner` selects the original Handler or Contract occurrence, with null for the Generator
root. `source.scenariosValue` selects the corresponding raw array occurrence in `source.values`.

`scenarios` preserves the authored order, repeated references, empty arrays, null slots and
unresolved targets. Its target type is `MissionScenario`. Each stored slot has immediate source
evidence. The declaration stays with its original owner when the logical contract projection
excludes or deduplicates a row. The table describes authored requirements independently of any runtime scenario
availability.

## Run terms

`timeToComplete` and `buyInAmount` are sourced values on `projections.contracts.rows`. Both
use the parent contract's first results struct. Sub-contract rows retain those same original
values and source uses. `timeToComplete` preserves the finite floating-point projection, with
conversion evidence for a narrowed double or integer input. `buyInAmount` retains the stored
integer when it fits in the projected signed integer. The Archive states no unit.

Readable absence is `absent`. An unreadable input without a usable value is `unreadable` with
an incomplete source set. Both states have `value: null`. A missing declaration retains
the actual declaration survey. Failed reads remain attempts, and the original results block
remains in `definitions.contractGenerators`.

## Cooldown values and levels

The five cooldown fields on `projections.contracts.rows` carry sourced values. Each member of
`cooldownLayers` carries its own sourced level. `cooldownLayer` carries the nearest level selected
across those five decisions. Each cell includes `state`, `value`, `containsEncryptedSource`,
`sourceUsesRefs` and `sourceSetComplete`. Boolean `false` and numeric zero are present values.

Each field uses the first usable typed override in sub-contract, contract and handler order.
Parameter names are matched case-insensitively. The handler's `defaultAvailability` supplies
the final fallback. An override retains the contributing choice inputs and its original datum.
Integer overrides retain a conversion use. Defaults and boolean overrides retain payload uses.
Each layer result retains the inputs that decided the level. The summary joins the five sets of
level inputs.

Readable absence has state `absent`. An unreadable input without a usable fallback yields state
`unreadable` and an incomplete source set. Both states have `value: null`. Failed reads remain
visible as attempts. A successful fallback can have a complete set of contributors. The
summary can still name a level when another field is unreadable. Its source set is incomplete
in that case. The original declarations remain in `definitions.contractGenerators`.

## Layered contract facts

Each of `canBeShared`, `illegal`, `onceOnly`, `maxPlayersPerInstance`, `availableInPrison`,
`canReacceptAfterAbandoning`, `canReacceptAfterFailing` and `hideInMobiGlas` contains a sourced
`value` and a sourced `layer`. The value records the selected original boolean or integer.
The layer records the choice of `subContract`, `contract`, `handler` or `template`.
Each cell carries its own state, encryption marker, contribution references and completeness.

The first usable typed override wins in sub-contract, contract and handler order. Parameter
names are matched case-insensitively. When no usable override is present, the handler's
`defaultAvailability` supplies six facts, and the Template supplies `canBeShared` and `illegal`. A Template fallback includes the actual Generator reference and the contributing
fields of the selected Template Record. The scalar value carries its own payload use. The
layer carries the selectors that established its choice. An unused fallback supplies no
contribution to either cell.

Readable absence has state `absent`. An unreadable input with no usable fallback produces
`unreadable` and incomplete evidence. Both states carry a null value. A readable `false`,
zero, or negative player count retains its original value.

## Intro prerequisites

`requiredIntros` carries the ordered list of authored completion tag requirements. Handler,
contract and sub-contract declarations are appended in that order. Repeated authored tag
references retain their separate occurrences. The sourced `level` describes where each
requirement occurs. The sourced `tagGuid` retains the original tag reference.

Each `introContracts` list and each grant ID retain the exact authored tag occurrence that
selected that lookup. A grant uses the intro contract's first mission result and the completion
tags of that result when it indicates success. Several physical intro occurrences can contribute
to the same grant ID. A whole-record weak pointer also retains the original target's GUID header
when that header supplies the tag. An empty grant list describes the admitted Generator scan.
Relevant read failures, including an unreadable Generator path that prevents selection, make
the list's source set incomplete.

## Fixed payouts

`fixedRewards` is a sourced list. The parent contract's first `contractResults` struct supplies
the results in stored order. A `ContractResult_Reward` contributes its first `contractReward`
struct when its first `missionResults` slot indicates success. Sub-contract rows share that
parent list.

Each payout carries sourced `currency` from `currencyType`, `amount` from `reward`, `max` and
`plusBonuses`. The two signed integers use exact decimal strings. Zero and false are present
values. An absent declaration, explicit null, unreadable input and invalid value have distinct
source states. The list retains the actual result selection. An unreadable selection input makes
the list's source set incomplete. An unreadable payout member makes that member's source set
incomplete.

## Reputation gains

`reputationGains` is a sourced list. A sub contract with a nonempty result list uses that list.
The parent supplies the results for other rows. A `ContractResult_LegacyReputation` contributes its
authored amount entries when its first `missionResults` slot indicates success. The entries retain
their stored order.

The faction and scope GUIDs retain the authored references. Each display name retains the actual
target path used to read its basename. The amount retains the authored Reward reference and the
original selected `reputationAmount`. When more than one Reward Record has the same GUID, the last
readable integer in physical Record order wins. The amount also retains evidence of later candidates
consulted during that selection. An unreadable candidate makes the amount's source set incomplete
even when another candidate supplies the amount. Amounts use exact signed decimal strings, including
zero and negative values.

## Authored keys and language values

Each title row's `contract` is the same physical occurrence reference as `source` on its contract
projection. `title` and `description` carry a sourced `key` and a `translations` map. A present key
names `texts.gameTexts[].id.value`. That row contains the original authored key and its available
language forms. Each translation in `projections.missionTitles` is a separate sourced display
result after mission-token expansion. Both keys and display text carry `x-gameText`.

A title uses the first usable override in sub-contract, contract and handler order, then the
first Template display slot and the Broker title. A description uses the same overrides followed
by the Broker description. Each override layer considers its first matching authored parameter.
A readable unusable value at that site falls through to the next layer.
If all choices end with an unreadable authored key, the key result is unreadable and
its contributor set is incomplete. Readable absence and a successful later fallback keep
their complete contribution sets. Failed read attempts remain separately observable.

Each language result follows its own preferred INI lookup and the nearest actually consulted
token bindings. String-hash options, eligible Organization variants, explicit Tag name matches
and nested lookups keep their original source contributions. Runtime and unresolved tokens
retain their bracket form. A readable missing translation has an absent result. A missing
language Entry has no map member. Input errors and failed attempts remain independently visible
in source and export-health metadata.

`containsEncryptedSource`, `sourceUsesRefs` and `sourceSetComplete` are immediate on the
authored key and every derived language value. Each value uses its own selected original key,
interpreted decision fields and actual language payloads. Shared canonical text definitions
may have several authored callers. The projection uses its own contributing occurrences.
Filter selection applies independently to Template and Broker definition rows. A consulted
dependency can still contribute physically observed source facts when its definition is filtered.
