# Typed mission property values

Generator properties use the tagged `PropertyValue` in
`missionPropertySets[].properties[].value`. Template and Broker properties retain their original
fields and ordered child values in `definitions.contractTemplates` and
`definitions.missionBrokerEntries`. Their `configurations` rows carry property bindings and
`valueMeanings`, while `configurations.missionProperties` links to the physical source declarations.
See [mission source and context](catalog-template-source.md) for the original value coordinates.

These fields describe authored configuration. Unsupported concrete variants, additional
unrepresented fields and malformed present payloads remain explicit source failures.

`DefinitionReference` retains `guid`, `targetRecordIndex`, `recordPath`, `targetType` and
`status` (`present`, `dangling` or `unexpectedTargetType`). A dangling target retains its original
GUID. The reader does not follow a Reference into its target Record. An authored tag list is
`{"tags": [...]}` with these nullable References in source order. Resolved tag-path lists remain
available separately where the established catalog payload already exposes them.

## Automatic spawns

`MissionPropertyValue_AutoSpawns` has discriminator `autoSpawns`, an `autoSpawns` definition
Reference and optional `searchOptions`:

| Search discriminator | Source type | Fields |
| --- | --- | --- |
| `base` | `SpawnClosetSearchOptionsBase` | None |
| `entityDeclaration` | `SpawnClosetSearchOptions_EntityDeclaration` | `entityDeclaration`: definition Reference |
| `tags` | `SpawnClosetSearchOptions_Tags` | `closetPositiveTags`, `closetNegativeTags`: authored tag lists |

An absent pointer is null; the fieldless base is an explicit concrete object. Empty tag lists
remain empty, and unresolved target identities remain present. This value references the
independent AutoSpawns definitions without flattening them into legacy NPC-spawn descriptions.

The ignored AutoSpawns source comparison requires a physical Template with a readable target.
Set P4KX_EXPECT_AUTO_SPAWNS_CLOSET_TAGS=1 to also require authored closet tags. Both modes
compare every selected occurrence and the exact source state of its search options.

## Investigation

`MissionPropertyValue_Investigation` has discriminator `investigation` and ordered
`startingJournalEntries`, `suspects` and `locations`. Journal entries are definition References.
Each suspect contains `name` (the established AI-name configuration), `initialSuspect` and
`isGuilty`. Each location contains ordered `clueItems`.

Suspect and corpse-scan names retain the established AI-name fields and add a required `source`
object. Its nullable `characterGivenName`, `characterGivenLastName` and `characterGivenNickName`
keep the original Locale tokens, including empty strings, placeholders and non-`@` text. Its
nullable `characterNameData` is the complete definition Reference, retaining a dangling GUID or
resolved physical target metadata. The presentation fields keep their existing key normalisation;
`nameDataGuid` stays null for an unresolved source link. The existing `aiName` property variant and
embedded NPC-name API retain their shape and meaning. Present malformed fields and nonfinite
nickname chances fail the investigation owner with the exact indexed name-field location.

Every clue item retains `entityClass`, `spawnTags`, ordered `clues` and `details`:

| Details discriminator | Source type | Additional fields |
| --- | --- | --- |
| `base` | `Investigation_ClueItem` | None |
| `corpse` | `Investigation_ClueItem_Corpse` | `suspectRepresented`: owner-local weak Reference |
| `datapad` | `Investigation_ClueItem_Datapad` | `canvas`: definition Reference |

Every clue retains localised `objectiveLongDescription`, `objectiveShortDescription` and
concrete `details`:

| Details discriminator | Source type | Additional fields |
| --- | --- | --- |
| `base` | `Investigation_Clue` | None |
| `corpseScanInfo` | `Investigation_Clue_CorpseScanInfo` | `name`: AI-name configuration; localised `causeOfDeath` |
| `journalEntry` | `Investigation_Clue_JournalEntry` | `journalEntry`: definition Reference |

## Options, dependencies and race splits

The `stringHash`, `integer`, `float` and `tags` variants retain ordered `options`. Every option
contains raw nullable `textId`, localised `displayName`, source `Single` `weighting`, `debugForceChooseThisOption` and
ordered `dependentProperties`. `textId` keeps empty and placeholder tokens exactly; `displayName`
retains its established key normalisation, excluding placeholder and non-`@` tokens. Fractional
weights and explicit false flags remain unchanged.
String values retain empty strings. Integer `value` and `variation` are exact signed `Int32`
values, including values that a 32-bit float cannot represent exactly. Float values and variations
retain finite source numbers. Tag options retain resolved `tags`/`negativeTags` and authored
`sourceTags`/`sourceNegativeTags`; the top-level `tags` value exposes the same raw tag lists.

A `DependentMissionProperty` is a structure with `property` and ordered `values`, rather than
a bare weak pointer. `property` uses the documented [mission-property link](scoped-locations.md)
with its variable when available. `values` retains raw owner-local links to the selected option
nodes. An option or investigation suspect need not contain a mission-variable field to resolve.

Raw owner-local links contain `targetRecordIndex`, `recordGuid`, `recordPath`, `targetType`,
`location`, `status`, `sourcePointer` and `sourceError`. `resolved` requires the original physical
Record and canonical node; `missingTarget` and `outsideOwner` retain available identities or raw
`structIndex`/`instanceIndex` coordinates. Matching GUIDs and paths never authorize another
physical owner. These Build-local indices are provenance, not new stable domain identifiers.
The same ownership check also bounds derived property-variable and objective links.

`timeTrialRace.targetSplits` retains ordered `TimeTrialSplit` structures with source `Single`
`targetSplit`, without changing its unit. Nullable pointers remain null, absent arrays remain
null and declared empty arrays remain empty. Class arrays retain their authored structures;
nullable Reference and pointer arrays retain null positions and duplicates.

Location/data-set conditions support `factionIdentity` with its original definition Reference.
Nearby and distant exclusion `rangeKm` values retain source `Single` precision. Ship-spawn
options retain the authored `crewInheritsLocationFaction` flag independently of their weight,
other tag settings or runtime faction selection.

Synthetic checks run with `cargo test -p p4kx-export --test it --locked mission_values` and
the spawn/owner regression suites. Their fixtures use the declared source integer, float,
Class, strong-pointer and weak-pointer types; schema-only alternatives are exercised explicitly.
Declaration-only LIVE/PTU metadata establishes type and field coverage, independently of any
claim that every declared variant is populated in a particular Build.
