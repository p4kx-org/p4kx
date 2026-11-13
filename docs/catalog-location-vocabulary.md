# Catalog mission location vocabulary

The catalog v3 keeps seven physical Record families as definition tables. Each row has the
[canonical Record header](catalog-record-headers.md) with the original GUID and physical
Record index. The exact Record type selects a family at any path. Rows sort by GUID and
physical index.

| Catalog table | Original Record type | Original typed field |
| --- | --- | --- |
| `definitions.contractScopes` | `ContractScopeGlobalParams` | Seven named `ContractScope_Item` StrongPointer slots with a Locale label and an icon String |
| `definitions.locationClasses` | `MissionLocationClass` | `scopesVisibleTo`, an ordered Enum array of `ContractScope` |
| `definitions.locationFunctions` | `MissionLocationFunction` | The Record identity |
| `definitions.locationSizes` | `MissionLocationSize` | The Record identity |
| `definitions.locationRegistries.classes` | `MissionLocationClasses` | `locationClasses`, an ordered Reference array to `MissionLocationClass` |
| `definitions.locationRegistries.functions` | `MissionLocationFunctions` | `locationFunctions`, an ordered Reference array to `MissionLocationFunction` |
| `definitions.locationRegistries.sizes` | `MissionLocationSizes` | `locationSizes`, an ordered Reference array to `MissionLocationSize` |

All three registry tables export the original array as `members`. The catalog retains
each registry Record even when its array is empty. An individual dictionary definition
can have zero own payload fields. Additional authored members stay in the owner-local
`authoredFields` arena.

Scope labels and registry members retain their source order and repetitions. The array
and each position have independent source evidence and a `containsEncryptedSource` marker.
A null position stays null. A dangling or incompatible target retains its original GUID
and link state. Export health identifies targets excluded by a Filter. Table availability
is based on the source declaration, independently of the count of emitted rows.

Each `missionLocationTemplates` row carries `classification` from its scalar
`MissionLocationTemplate.locationData` Class of `MissionLocationData`. The value contains
the three independent original References `locationClass`, `locationFunction` and
`locationSize`. Each link identifies the original GUID and the selected physical target
when available. Null and dangling links retain their own states and evidence. An absent
or incompatible Class has its own outer state. The existing general, produce, consume
and AI tag values still describe their original tag fields.

Mission location filters that reference these definitions join to their original GUID
and physical Record identity when available. The registry membership and visible scopes
describe configuration stored in the Archive.

## Global contract scopes

The physical global scope table retains `location`, `cluster`, `sector`, `planetaryBody`,
`planetarySystem`, `solarSystem` and `universe` as individually sourced pointer slots.
Each present item keeps its original concrete type, `displayName` as a public short name
reference and `icon` as the exact stored String. The public name table retains the original
Locale key and resolved translations. This vocabulary describes authored labels and icons.
The source `ContractScope` Enum on location and contract configuration retains its own
authored label. Availability is explicit for Builds where the global Record type is absent.

## Mission board providers

`configurations.missionBoards` contains each selected
`EntityComponentMissionBoardUIProvider` occurrence in an Entity Record's `Components` array.
The Record filter applies to the original owner path. The table remains available when
the source declares the component type and the filter selects no matching owners.
Each row carries the owner GUID, physical Record index and original component ordinal.

`boardType` is an original StrongPointer to `MissionBoardType_Base`. The `career`
variant holds `guildFactionReputation` and the ordered `factions` Reference array.
Both target `FactionReputation` definitions. Duplicate occurrences, null slots and
missing targets keep their source positions and states. The `notice` and `base`
variants retain their concrete identity. Additional variants retain their concrete type
and complete authored fields. A null pointer has its own sourced state.

The table describes authored Entity configuration. Contract availability belongs to the
contract source.

## Installed verification

Set the standard `P4KX_DATA_P4K` and Archive key inputs. Each command builds filtered
catalogs for independently selected physical Records rather than a full catalog.

    cargo test --locked --release -p p4kx-export --test real_archive mission_location_vocabulary_matches_original_records_and_ordered_slots -- --ignored
    cargo test --locked --release -p p4kx-export --test real_archive mission_board_providers_match_original_component_slots -- --ignored

The location test compares physical dictionaries and registries, ordered arrays and classification
links with per-datum provenance. The board test selects one original Entity owner per observed
board alternative and compares every original board component on each selected owner.
Set `P4KX_EXPECT_LOCATION_VOCABULARY=1` or `P4KX_EXPECT_MISSION_BOARD_PROVIDERS=1` for positive
source requirements. Optional categories without an original owner report source absence.
