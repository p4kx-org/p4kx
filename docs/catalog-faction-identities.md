# Faction identity definitions

`definitions.factionIdentities` contains one row per physical `FactionIdentity` Record selected by the ordinary Record filter. GUID, normalized source path and original Record index identify the owner. Separate identities can refer to the same Faction.

| Source declaration | Catalog field |
| --- | --- |
| `name`, `description` Locale | `name`, `description` protected `GameTextRef` values with the original keys and observed translations in the shared game text table |
| `factionTag` Reference<Tag> | `factionTag` original Tag link |
| `factionRef` Reference<Faction> | `factionRef` original Faction link |
| `securityNetworkRef` Reference<SecurityNetworkManifest> | `securityNetworkRef` original [security manifest](catalog-security-network-manifests.md) link |
| `encounterPresetWeights` Class<EncounterPresetWeight>[] | `encounterPresetWeights` ordered entries with the authored `EncounterPreset` enum token and exact `Single` weight |
| `lootPoolThemeTags` Reference<Tag>[] | `lootPoolThemeTags` ordered original Tag links |

The source carries the encounter preset as an enum. An EncounterIndex can group spawn sets by the same authored enum token. NPC archetype branches and AutoSpawnData carry separate physical FactionIdentity references. A Faction supplies faction configuration, and a FactionReputation supplies reputation configuration. Consumers can join those definitions through their respective original references.

Every sourced field and list element carries its immediate provenance. Empty arrays, null, invalid and unsupported data retain separate states. A resolved original target retains its identity when excluded from the catalog, and a dangling Reference retains its declared GUID. Extra authored declarations remain in `authoredFields`.

The scoped SDK route is `catalog_queries::faction_identities::with_faction_identities`.
Use `CatalogRecordSelection::All`, `Guid` or `RecordIndex` within the supplied Build.
A GUID selection retains every physical match. An index selection selects that one original
Record and still applies the family admission rule. The result provides the canonical rows,
their protected text definitions for the requested languages, original provenance and coverage.
Use `CatalogQuerySession` to share a parsed DataForge across scoped queries.

## Faction rules and references

Use `catalog_queries::factions::with_factions` and its decoded-Entry or parsed-session
forms to read `definitions.factions` by GUID, physical Record index or all original Faction
Records. Rows keep the order of the Catalog. They expose original legal flags, ordered ally,
enemy and friendly-fire slots, a separate reputation link and the remaining authored fields.
Referenced targets retain their physical identity when resolved and their authored GUID when
dangling.

The query resolves `name` through the public-name domain and `description` through the
protected game-text domain. The original value can also be a literal. A caller chooses
which locale payloads to read, or an empty list to retrieve keys without translations.
The snapshot returns these text rows, original per-value provenance and physical read coverage.
An unreadable owned field leaves the other fields of its Faction available.
