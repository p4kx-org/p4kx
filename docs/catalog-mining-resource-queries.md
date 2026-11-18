# Selective mining and ResourceType definitions

`p4kx_export::catalog_queries` exposes `with_mineable_elements`,
`with_mineable_compositions` and `with_resource_types`, each with a `_from` wrapper accepting an
already decoded Entry of the same Archive. `CatalogQuerySession` and its parsed view expose the
same methods. Reuse a parsed view for several queries to retain the decoded DataForge and lazy
indexes without constructing a complete Catalog or world-placement graph.

All three endpoints accept `CatalogRecordSelection::All`, `Guid` or `RecordIndex`. `All` supplies
the ordinary Catalog-selected family. `Guid` returns every eligible physical declaration with that
GUID; `RecordIndex` selects one physical owner. Rows follow canonical textual GUID order, then
physical index for tied GUIDs. Header-only authored references preserve the existing canonical
resolution/status policy, including unresolved and unexpected-type targets.

| Query | Complete canonical dataset | Source payload work |
| --- | --- | --- |
| `with_mineable_elements` | `mineableElements` | Selected element owners; no locales or groups |
| `with_mineable_compositions` | `mineableCompositions` | Selected composition owners and requested locales |
| `with_resource_types` | `definitions.resourceTypes.rows` | Selected resource owners, requested locales and the required ResourceTypeGroup family |

A GUID/reference lookup can consult the shared Record-header index without opening the target body.
No endpoint reads world/container payloads, placements or a complete Catalog. Full ResourceType
membership requires all eligible ResourceTypeGroup bodies, even when querying one resource. The
canonical reader requests this survey at `groupGuids`, and one query caches its result for every
selected resource. Empty or failed selected resource owners do not request it. Element and
composition queries never read this group family.

The immutable snapshots expose `rows()`, family `availability()`, `coverage()` and
`dependency_coverage()`. Availability describes source declarations; an empty row list alone is not
proof that the family is absent. Owner coverage reports unreadable headers/bodies and blocking
field interpretations. Dependency coverage separately reports the demanded group-family survey.
An unknown or incomplete survey cannot prove negative membership. Its canonical `groupGuids` cell
retains incomplete source knowledge even if some memberships were found.

Before replacing a consumer cache, require outer query success, inspect both coverage results and
handle incomplete or nonpresent source cells. A callback may run with recoverable source failures;
its successful return is not a completeness certificate. Cancellation and Archive-change checks
still apply after the callback, so publish caller-owned changes only after the outer call succeeds.
No fixed source-count quota is introduced.

Every existing field remains typed and publicly traversable. Elements retain each Single's raw IEEE
bits and nullable finite value. Composition shares retain original integer width, wide decimal
integers and float bits; repeated slots preserve original order. Resource types expose all nested
density/unit, cargo slot, property and crafting-quality variants, including authored references and
additional original fields. Reference-valued quality definitions retain their authored target
header without opening its body. Resolving another definition's full body is a separate request.

Use `public_name()` and `game_text()` to resolve references within this snapshot. Resource names
remain public short texts; descriptions and composition deposit labels retain their protected text
domain. `requested_languages()` preserves the requested policy; `available_languages()` lists only
payloads actually present and read. Elements need no locales. An empty successful-owner selection
leaves requested locale payloads unopened.

Resolve each field's `origin().source_uses_refs()` through this snapshot's own
`source_inventory()`, then follow its public physical source, ancestors and proof views. The
per-datum encrypted-source marker is independent of text domain. Plain, encrypted and unknown
source states remain distinct. Source-use IDs and text IDs are local to their originating context;
do not substitute an inventory from another query.

Borrowed ResourceType and text rows stay inside the callback. Return consumer-owned values if they
must outlive the query. This additive SDK surface preserves Schema v3 wire fields and values.
Independent harvesting/world/placement, refining, item and blueprint domains remain separate work.

Decision: [ADR0403](adr/0403-direct-mining-resource-queries-retain-canonical-membership-dependencies.md).
