# NPC spawn definitions in catalog v3

| Catalog table | DataForge Record | Typed fields |
| --- | --- | --- |
| `definitions.npcArchetypes` | `NPCArchetype` | `identity` is the ordered `NPCIdentityToArchetype[]`. Each branch has an `identity` Reference to `FactionIdentity` and an ordered `npcPools` Reference array to `NPCClassPool`. |
| `definitions.npcClassPools` | `NPCClassPool` | `npcClasses` is the ordered `NPCClassPoolEntry[]`. Each entry has the original Single `weight` with exact bits and a Reference `npcClass` to `EntityClassDefinition`. |
| `definitions.autoSpawns` | `AutoSpawn` | `autoSpawnData` holds the class selector, faction identity, initial activity, assignment, limits, inventory and entity tags. |
| `definitions.autoSpawnSets` | `AutoSpawns` | `autoSpawnData` is an ordered StrongPointer array. `AutoSpawnsDataRef` links to `AutoSpawn` and `AutoSpawnsDataManual` owns `AutoSpawnData`. |
| `definitions.encounterIndices` | `EncounterIndex` | `encounters` is an ordered Class array. Entries preserve the `EncounterPreset` Enum and the `autoSpawns` Reference array. |

`AutoSpawnData.class` distinguishes `AutoSpawnTargetClass`, `AutoSpawnArchetype` and
`AutoSpawnClassPool`. The direct branch links the source EntityClassDefinition. The archetype
branch retains every ordered `NPCArchetypeWeight` and its original Single and Reference. The
class-pool branch retains the ordered Reference array. These targets keep their own definition rows.

`onSpawnAssignment` distinguishes the authored base, schedule-area and defend-area forms. Each
retains its ordered positive and negative Tag References. The inventory entries preserve
`entityClass`, `quantity`, `amountInventoriesToFill` and `markAsMissionItem`. Delays preserve
the Single bits and finite values. An additional concrete pointer alternative retains its type
and raw authored members and is reported in export health.

The tables select by concrete Record type across all paths, subject to the catalog Filter.
Rows retain physical Record identity and order by GUID and Record index. The authored arrays
retain their original order, repeated links and null slots. Remaining original fields are retained
in `authoredFields`. Every sourced value has its own `containsEncryptedSource` marker.

Reference payloads keep the declared target identity and source resolution. Export health records
when a known target is excluded by the Filter. Readers can join a pool by its physical identity.
Weights are authored values. The catalog leaves class selection and effective spawn chances to the
runtime.
