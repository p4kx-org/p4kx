# NPC spawn definitions in catalog v3

| Catalog table | DataForge Record | Typed fields |
| --- | --- | --- |
| `definitions.npcArchetypes` | `NPCArchetype` | `identity` is the ordered `NPCIdentityToArchetype[]`. Each branch has an `identity` Reference to `FactionIdentity` and an ordered `npcPools` Reference array to `NPCClassPool`. |
| `definitions.npcClassPools` | `NPCClassPool` | `npcClasses` is the ordered `NPCClassPoolEntry[]`. Each entry has the original Single `weight` with exact bits and a Reference `npcClass` to `EntityClassDefinition`. |

Both tables select by concrete Record type across all paths, subject to the catalog Filter.
Rows retain physical Record identity and order by GUID and Record index. The authored arrays
retain their original order, repeated links and null slots. Remaining original fields are retained
in `authoredFields`. Every sourced value has its own `containsEncryptedSource` marker.

Reference payloads keep the declared target identity and source resolution. Export health records
when a known target is excluded by the Filter. Readers can join a pool by its physical identity.
Weights are authored values. The catalog leaves class selection and effective spawn chances to the
runtime.
